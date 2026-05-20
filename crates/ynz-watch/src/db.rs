use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use salsa::Setter as _;
use ynz_codegen::codegen_query;
use ynz_diagnostics::{DiagnosticBucket, render};
use ynz_parser::{CompilerDb, SourceFile};

use crate::project::{WatchSourceFile, WatchTarget};

/// The central state for the watch daemon.
///
/// Holds two state stores:
///   1. `compiler_db` — the salsa `CompilerDb`, which caches parse/typeck/codegen results.
///      Dropped and recreated periodically (Layer 2 periodic rebuild).
///   2. `sources` — shadow `HashMap<PathBuf, String>` that lives outside salsa and is the
///      source of truth. When `compiler_db` is rebuilt, `sources` repopulates the fresh DB.
///
/// The shadow map MUST be written FIRST in `update_source`. If the process panics between
/// the shadow write and the salsa write, the shadow remains consistent with last-known content.
///
/// Time: O(1) amortized per rebuild. Space: O(n) where n = number of source files.
pub struct WatchDb {
    /// The salsa DB — derived cache; may be dropped and recreated.
    compiler_db: CompilerDb,
    /// Shadow source state — the source of truth for all file contents.
    sources: HashMap<PathBuf, String>,
    /// Salsa SourceFile handles keyed by path.
    source_files: HashMap<PathBuf, SourceFile>,
    /// How many rebuilds have happened this DB lifetime.
    rebuild_count: u64,
    /// When the current DB was created (monotonic — immune to clock skew).
    db_created_at: Instant,
}

impl WatchDb {
    /// Create a `WatchDb` populated from the resolved watch target.
    pub fn from_target(target: &WatchTarget) -> Self {
        let mut db = WatchDb {
            compiler_db: CompilerDb::default(),
            sources: HashMap::new(),
            source_files: HashMap::new(),
            rebuild_count: 0,
            db_created_at: Instant::now(),
        };
        for sf in &target.sources {
            db.init_source(&sf.path, &sf.text);
        }
        db
    }

    /// Register a source file on first load (before any salsa operations).
    fn init_source(&mut self, path: &Path, text: &str) {
        let path_str = path.display().to_string();
        let sf = SourceFile::new(&self.compiler_db, path_str, text.to_string());
        self.compiler_db.register_source(sf);
        self.sources.insert(path.to_path_buf(), text.to_string());
        self.source_files.insert(path.to_path_buf(), sf);
    }

    /// Return true if `text` is identical to the last-known shadow content for `path`.
    ///
    /// Used to suppress rebuilds triggered by our own `fs::read_to_string` calls — notify
    /// 8.x watches `IN_OPEN` so every rebuild read fires a new inotify event. Skipping the
    /// rebuild when content is unchanged breaks the feedback loop without losing any real
    /// user-initiated changes (salsa would return the cached result anyway).
    pub fn source_unchanged(&self, path: &Path, text: &str) -> bool {
        self.sources.get(path).is_some_and(|existing| existing == text)
    }

    /// Update a source file in both the shadow store and the salsa DB.
    ///
    /// Shadow is written FIRST so a mid-update panic leaves the shadow consistent.
    pub fn update_source(&mut self, path: &Path, text: String) {
        // 1. Shadow update — source of truth.
        self.sources.insert(path.to_path_buf(), text.clone());

        // 2. Salsa input mutation — or creation if this is a new file.
        if let Some(&sf) = self.source_files.get(path) {
            sf.set_text(&mut self.compiler_db).to(text);
        } else {
            self.init_source(path, &text);
        }
    }

    /// Drop the current `CompilerDb` and recreate it from the shadow source state.
    ///
    /// Called by the periodic-rebuild trigger (Layer 2 memory defense). The shadow map
    /// ensures zero source-state loss across the rebuild — salsa is a derived cache.
    pub fn rebuild_db(&mut self) {
        // Drop and replace the salsa DB.
        self.compiler_db = CompilerDb::default();
        self.source_files.clear();

        // Repopulate from shadow.
        for (path, text) in &self.sources {
            let path_str = path.display().to_string();
            let sf = SourceFile::new(&self.compiler_db, path_str, text.clone());
            self.compiler_db.register_source(sf);
            self.source_files.insert(path.clone(), sf);
        }

        self.rebuild_count = 0;
        self.db_created_at = Instant::now();
    }

    /// Run the full compiler pipeline (codegen_query) against `entry_path`.
    ///
    /// Returns a `RebuildOutcome` with diagnostics + optional object bytes.
    ///
    /// Time: O(1) amortized (salsa caches all unchanged queries). Space: O(1) runtime.
    pub fn run_codegen(&mut self, entry_path: &Path) -> RebuildOutcome {
        self.rebuild_count += 1;

        let sf = match self.source_files.get(entry_path) {
            Some(&sf) => sf,
            None => {
                // File not in DB — possibly an out-of-project file.
                return RebuildOutcome::error(format!(
                    "WHAT: `{}` is not registered as a project source file.\n\
                     WHAT INSTEAD: Only files discovered at watch startup are compiled.\n\
                     WHY: The watch daemon populates its source map on boot; files added \
                     after startup are not automatically tracked.",
                    entry_path.display()
                ));
            }
        };

        let out = codegen_query(&self.compiler_db, sf);
        let elapsed_ms = 0u128; // rebuild.rs::rebuild_one patches this after timing the codegen pass

        if out.diagnostics.has_errors() {
            RebuildOutcome::Errors {
                diags: out.diagnostics.clone(),
                sources: self.source_snapshot(),
                elapsed_ms,
            }
        } else {
            RebuildOutcome::Success {
                object_bytes: out.artifact.object_bytes.clone(),
                elapsed_ms,
            }
        }
    }

    /// Check whether the periodic-rebuild threshold has been reached.
    ///
    /// Returns `true` if either condition holds:
    ///   - `rebuild_count >= rebuild_after_n`
    ///   - `db_created_at.elapsed() >= rebuild_after_duration`
    ///
    /// Uses `Instant` (monotonic) for both checks — immune to NTP / clock skew.
    pub fn should_periodic_rebuild(&self, rebuild_after_n: u64, rebuild_after_duration: Duration) -> bool {
        self.rebuild_count >= rebuild_after_n
            || self.db_created_at.elapsed() >= rebuild_after_duration
    }

    /// Read the rebuild counter (for testing Layer 2 trigger).
    pub fn rebuild_count(&self) -> u64 {
        self.rebuild_count
    }

    /// Snapshot the current source text map (path → text, String keys for ariadne renderer).
    pub fn source_snapshot(&self) -> HashMap<String, String> {
        self.sources
            .iter()
            .map(|(p, t)| (p.display().to_string(), t.clone()))
            .collect()
    }
}

/// The result of one rebuild cycle.
pub enum RebuildOutcome {
    /// Compilation succeeded; caller may spawn the binary.
    Success {
        object_bytes: Vec<u8>,
        elapsed_ms: u128,
    },
    /// Compilation produced errors; diagnostics ready to render.
    Errors {
        diags: DiagnosticBucket,
        sources: HashMap<String, String>,
        elapsed_ms: u128,
    },
    /// Infrastructure failure (path not registered, etc.).
    Infra(String),
}

impl RebuildOutcome {
    fn error(msg: String) -> Self {
        RebuildOutcome::Infra(msg)
    }

    /// Render the human-readable diagnostic string (for text-mode output).
    pub fn render_diagnostics(&self) -> String {
        match self {
            RebuildOutcome::Errors { diags, sources, .. } => {
                render(diags, sources, false)
            }
            RebuildOutcome::Infra(msg) => format!("{msg}\n"),
            RebuildOutcome::Success { .. } => String::new(),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, RebuildOutcome::Success { .. })
    }

    pub fn error_count(&self) -> usize {
        match self {
            RebuildOutcome::Errors { diags, .. } => {
                diags.iter().filter(|d| d.severity == ynz_diagnostics::Severity::Error).count()
            }
            _ => 0,
        }
    }
}

/// Populate a fresh `WatchDb` from a `WatchTarget` (helper for project mode init).
pub fn init_db(sources: &[WatchSourceFile]) -> WatchDb {
    let mut db = WatchDb {
        compiler_db: CompilerDb::default(),
        sources: HashMap::new(),
        source_files: HashMap::new(),
        rebuild_count: 0,
        db_created_at: Instant::now(),
    };
    for sf in sources {
        db.init_source(&sf.path, &sf.text);
    }
    db
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::WatchSourceFile;

    // WHY (all db unit tests): WatchDb's shadow source state is the load-bearing invariant
    //      for Layer 2 periodic DB rebuild. If shadow ↔ salsa diverge, rebuild_db() silently
    //      empties salsa and watch reports 0 errors on broken code (silent-wrong-output class).

    fn make_db(sources: &[(&str, &str)]) -> WatchDb {
        let entries: Vec<WatchSourceFile> = sources
            .iter()
            .map(|(path, text)| WatchSourceFile {
                path: PathBuf::from(path),
                text: text.to_string(),
            })
            .collect();
        init_db(&entries)
    }

    #[test]
    fn rebuild_db_preserves_source_text() {
        let mut db = make_db(&[
            ("/tmp/a.ynz", "// file a\n"),
            ("/tmp/b.ynz", "// file b\n"),
        ]);

        db.rebuild_db();

        let snap = db.source_snapshot();
        assert_eq!(snap.get("/tmp/a.ynz").map(String::as_str), Some("// file a\n"));
        assert_eq!(snap.get("/tmp/b.ynz").map(String::as_str), Some("// file b\n"));
    }

    #[test]
    fn update_source_writes_shadow_first() {
        let mut db = make_db(&[("/tmp/x.ynz", "// v0\n")]);

        // Shadow should reflect the update immediately.
        db.update_source(Path::new("/tmp/x.ynz"), "// v1\n".to_string());
        assert_eq!(
            db.sources.get(Path::new("/tmp/x.ynz")).map(String::as_str),
            Some("// v1\n")
        );
    }

    #[test]
    fn periodic_rebuild_threshold_by_count() {
        let db = make_db(&[("/tmp/a.ynz", "// a\n")]);
        // With 0 rebuilds against threshold 500, should NOT trigger.
        assert!(!db.should_periodic_rebuild(500, Duration::from_secs(14400)));
    }

    #[test]
    fn rebuild_count_resets_after_rebuild_db() {
        let mut db = make_db(&[("/tmp/a.ynz", "// a\n")]);
        // Manually bump the counter to simulate rebuids.
        db.rebuild_count = 499;
        assert!(!db.should_periodic_rebuild(500, Duration::from_secs(14400)));
        db.rebuild_count = 500;
        assert!(db.should_periodic_rebuild(500, Duration::from_secs(14400)));

        db.rebuild_db();
        assert_eq!(db.rebuild_count(), 0);
        assert!(!db.should_periodic_rebuild(500, Duration::from_secs(14400)));
    }
}
