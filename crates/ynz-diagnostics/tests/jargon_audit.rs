// WHY: Banned jargon in compiler diagnostics means a developer who doesn't know
// CS theory can't act on the error message. This test catches violations at CI
// time so they never reach a user. See design/compiler-errors.md for the full
// ban-list and their plain-English replacements.

use std::path::{Path, PathBuf};
use ynz_registry;

fn crates_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR points to crates/ynz-diagnostics/
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates")
}

fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_rs_files(&path));
            } else if path.extension().map_or(false, |e| e == "rs") {
                files.push(path);
            }
        }
    }
    files
}

fn is_exempt(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    // Skip the constant-definition file (it lists the banned words by design)
    // and this audit file itself.
    name == "banned_jargon.rs" || name == "jargon_audit.rs"
}

/// Extract string content found inside Diagnostic construction call sites.
///
/// Looks for lines containing `Diagnostic::{error,warning,suggestion,new}(`
/// and scans string literals in the surrounding block until the call's parens
/// balance. Returns (file, line_number, string_content) triples.
fn find_diagnostic_strings(source: &str, filename: &str) -> Vec<(String, usize, String)> {
    let mut results = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    let mut in_ctx = false;
    let mut depth: usize = 0;

    for (line_idx, &line) in lines.iter().enumerate() {
        if line.contains("Diagnostic::error(")
            || line.contains("Diagnostic::warning(")
            || line.contains("Diagnostic::suggestion(")
            || line.contains("Diagnostic::new(")
        {
            in_ctx = true;
            depth = 0;
        }

        if in_ctx {
            let mut chars = line.chars().peekable();
            let mut in_str = false;
            let mut escaped = false;
            let mut current = String::new();

            while let Some(c) = chars.next() {
                if escaped {
                    escaped = false;
                    if in_str {
                        current.push(c);
                    }
                    continue;
                }
                match c {
                    '\\' if in_str => {
                        escaped = true;
                    }
                    '"' if !in_str => {
                        in_str = true;
                        current.clear();
                    }
                    '"' if in_str => {
                        results.push((filename.to_string(), line_idx + 1, current.clone()));
                        in_str = false;
                        current.clear();
                    }
                    c if in_str => {
                        current.push(c);
                    }
                    '(' if !in_str => {
                        depth += 1;
                    }
                    ')' if !in_str => {
                        if depth > 0 {
                            depth -= 1;
                        }
                        if depth == 0 {
                            in_ctx = false;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    results
}

/// Returns true if `haystack` contains `needle` as a whole word (not as a
/// substring of a longer word). Non-letter characters count as word boundaries.
///
/// Multi-word phrases (e.g. "algebraic data type") are matched as substrings
/// regardless of word boundaries — they're distinctive enough not to false-positive.
fn contains_whole_word(haystack: &str, needle: &str) -> bool {
    let needle_is_phrase = needle.contains(' ');
    if needle_is_phrase {
        return haystack.contains(needle);
    }
    // Single-word check: verify word boundaries on both sides.
    let hay = haystack.as_bytes();
    let nee = needle.as_bytes();
    let nlen = nee.len();
    if nlen == 0 || nlen > hay.len() {
        return false;
    }
    for start in 0..=(hay.len() - nlen) {
        if &hay[start..start + nlen] == nee {
            let left_ok =
                start == 0 || !hay[start - 1].is_ascii_alphanumeric() && hay[start - 1] != b'_';
            let end = start + nlen;
            let right_ok =
                end >= hay.len() || !hay[end].is_ascii_alphanumeric() && hay[end] != b'_';
            if left_ok && right_ok {
                return true;
            }
        }
    }
    false
}

#[test]
fn no_banned_jargon_in_diagnostic_strings() {
    let mut violations: Vec<String> = Vec::new();

    for path in collect_rs_files(&crates_dir()) {
        if is_exempt(&path) {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let filename = path.display().to_string();

        for (file, line_num, string_content) in find_diagnostic_strings(&content, &filename) {
            let lower = string_content.to_lowercase();
            for entry in ynz_registry::banned_jargon() {
                let w = entry.name.to_lowercase();
                if contains_whole_word(&lower, &w) {
                    violations.push(format!(
                        "{file}:{line_num}: diagnostic string contains banned word {:?}: {string_content:?}", entry.name
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "Banned jargon found in {} diagnostic string(s):\n{}\n\n\
             See design/compiler-errors.md for the full ban-list and plain-English replacements.",
            violations.len(),
            violations.join("\n")
        );
    }
}

/// Build the LSP-rendered message string from three components — mirrors the format
/// used by `ynz-lsp::diagnostic_transform::to_lsp_diagnostic`.
fn lsp_message(what: &str, what_instead: &str, why: &str) -> String {
    format!("{}\n\nWHAT INSTEAD: {}\n\nWHY: {}", what, what_instead, why)
}

/// Walk every diagnostic site and check the LSP-rendered form for banned jargon.
#[test]
fn no_banned_jargon_in_lsp_rendered_messages() {
    let mut violations: Vec<String> = Vec::new();
    let banned: Vec<_> = ynz_registry::banned_jargon().collect();

    for path in collect_rs_files(&crates_dir()) {
        if is_exempt(&path) {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let filename = path.display().to_string();

        // Extract all three-argument diagnostic construction sites and build
        // the LSP message string (WHAT\n\nWHAT INSTEAD: X\n\nWHY: Y).
        // We extract strings greedily from each site; for a three-argument call,
        // the first 3 strings are (what, what_instead, why) in that order.
        let mut site_strings: Vec<String> = Vec::new();
        let mut in_ctx = false;
        let mut depth: usize = 0;
        let mut site_count = 0;

        for line in content.lines() {
            if line.contains("Diagnostic::error(")
                || line.contains("Diagnostic::warning(")
                || line.contains("Diagnostic::suggestion(")
            {
                in_ctx = true;
                depth = 0;
                site_count = 0;
            }

            if in_ctx {
                for (_, _, s) in find_diagnostic_strings(line, &filename) {
                    site_strings.push(s);
                    site_count += 1;
                }
                for c in line.chars() {
                    match c {
                        '(' => depth += 1,
                        ')' => {
                            if depth > 0 {
                                depth -= 1;
                            }
                            if depth == 0 {
                                in_ctx = false;
                                // Build LSP message if we collected at least 3 strings
                                if site_strings.len() >= 3 {
                                    let msg = lsp_message(
                                        &site_strings[0],
                                        &site_strings[1],
                                        &site_strings[2],
                                    );
                                    let lower = msg.to_lowercase();
                                    for entry in &banned {
                                        let w = entry.name.to_lowercase();
                                        if contains_whole_word(&lower, &w) {
                                            violations.push(format!(
                                                "LSP-rendered message contains banned word {:?}: {:?}",
                                                entry.name, msg
                                            ));
                                        }
                                    }
                                }
                                site_strings.clear();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "Banned jargon found in {} LSP-rendered diagnostic message(s):\n{}\n\n\
             See design/compiler-errors.md for replacements.",
            violations.len(),
            violations.join("\n")
        );
    }
}

/// Audit formatter-emitted CLI messages for banned jargon.
///
/// Scope: messages the formatter writes to the USER via stderr/stdout
/// (e.g. "ynz fmt: rewrote ...", "Would reformat: ...", parse error renders).
/// Explicitly EXCLUDED: Yinz source content that the formatter passes through
/// byte-exact — a user can legitimately write `// type` in a comment and the
/// formatter must not be flagged for that.
#[test]
fn fmt_cli_messages_contain_no_banned_jargon() {
    let banned: Vec<_> = ynz_registry::banned_jargon().collect();

    // These are the fixed-text portions of formatter CLI messages.
    // Dynamic file paths are excluded (they're user data, not our messages).
    let fmt_messages = [
        "ynz fmt: cannot read",
        "ynz fmt: cannot write temp file",
        "ynz fmt: cannot rename",
        "ynz fmt: rewrote",
        "ynz fmt: invalid input",
        "ynz fmt: no `yinz.toml` found above",
        "pass a path directly or create a yinz.toml project",
        "ynz fmt: provide a file path or use --all or --stdin",
        "ynz fmt: cannot read stdin",
        "Would reformat:",
        // FmtError display strings
        "source has parse errors; fix those first",
    ];

    let mut violations: Vec<String> = Vec::new();
    for msg in &fmt_messages {
        let lower = msg.to_lowercase();
        for entry in &banned {
            let w = entry.name.to_lowercase();
            if contains_whole_word(&lower, &w) {
                violations.push(format!(
                    "Formatter CLI message contains banned word {:?}: {:?}",
                    entry.name, msg
                ));
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "Banned jargon found in {} formatter CLI message(s):\n{}\n\n\
             See design/compiler-errors.md for replacements.",
            violations.len(),
            violations.join("\n")
        );
    }
}

// WHY: deferred_language_feature entries render in user-facing LSP hover (`**Substitute:** …`,
//      `**Why deferred:** …`, `**Ships in:** …`) and completion detail strings. If any of those
//      fields contain banned jargon, a user hovering a deferred feature sees the banned word.
//      deferred_tooling_feature entries currently have no LSP render path, but the check is
//      included so that adding a render path in the future automatically inherits jargon
//      enforcement — catching the class that slipped through in v0.3-M2 Phase 4.
#[test]
fn no_banned_jargon_in_deferred_feature_user_facing_fields() {
    let banned: Vec<_> = ynz_registry::banned_jargon().collect();
    let mut violations: Vec<String> = Vec::new();

    // deferred_language_feature: substitute + why + ships_in all render in hover and/or completion.
    for entry in ynz_registry::deferred_language_features() {
        let fields = [
            ("substitute", entry.substitute),
            ("why", entry.why),
            ("ships_in", entry.ships_in),
        ];
        for (field, text) in &fields {
            let lower = text.to_lowercase();
            for b in &banned {
                let w = b.name.to_lowercase();
                if contains_whole_word(&lower, &w) {
                    violations.push(format!(
                        "[[deferred_language_feature]] '{}' field '{}' contains banned word {:?}: {:?}",
                        entry.name, field, b.name, text
                    ));
                }
            }
        }
    }

    // deferred_tooling_feature: no current LSP render path, but audited proactively so
    // any future render path inherits jargon enforcement automatically.
    for entry in ynz_registry::deferred_tooling_features() {
        let fields = [
            ("substitute", entry.substitute),
            ("why", entry.why),
            ("ships_in", entry.ships_in),
        ];
        for (field, text) in &fields {
            let lower = text.to_lowercase();
            for b in &banned {
                let w = b.name.to_lowercase();
                if contains_whole_word(&lower, &w) {
                    violations.push(format!(
                        "[[deferred_tooling_feature]] '{}' field '{}' contains banned word {:?}: {:?}",
                        entry.name, field, b.name, text
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "Banned jargon found in {} deferred-feature field(s):\n{}\n\n\
             See design/compiler-errors.md for plain-English replacements.",
            violations.len(),
            violations.join("\n")
        );
    }
}

// WHY: `booleanean` is a typo for `boolean` that reached the user-facing `print` diagnostic;
//      `infers` is the inflected verb form of banned jargon `infer` — the whole-word check
//      for `infer` does not catch `infers` (the trailing `s` breaks the word boundary).
//      Both must not appear in any diagnostic string. This test provides a precise guard so
//      `cargo test -p ynz-diagnostics` catches a reintroduction immediately.
#[test]
fn no_typo_booleanean_or_verb_infers_in_diagnostic_strings() {
    let mut violations: Vec<String> = Vec::new();

    for path in collect_rs_files(&crates_dir()) {
        if is_exempt(&path) {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let filename = path.display().to_string();

        for (file, line_num, string_content) in find_diagnostic_strings(&content, &filename) {
            let lower = string_content.to_lowercase();
            if lower.contains("booleanean") {
                violations.push(format!(
                    "{file}:{line_num}: diagnostic string contains typo \"booleanean\" (fix to \"boolean\"): {string_content:?}"
                ));
            }
            if lower.contains("infers") {
                violations.push(format!(
                    "{file}:{line_num}: diagnostic string contains banned jargon verb \"infers\" (use \"figures out\"): {string_content:?}"
                ));
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "Typo/jargon found in {} diagnostic string(s):\n{}\n\n\
             Replace \"booleanean\" with \"boolean\"; replace \"infers\" with \
             \"figures out\" (per vocabulary.md — `infer`/`infers` is banned in user-facing text).",
            violations.len(),
            violations.join("\n")
        );
    }
}

// WHY: ynz watch emits status lines, error messages, and --help text to the user's terminal.
//      These strings bypass the `Diagnostic::*` constructor path (which the main jargon audit
//      scans), so they need a dedicated check. A banned word like "infer" or "null" in a
//      status line would be the same UX failure as in a compiler error.
#[test]
fn watch_cli_messages_contain_no_banned_jargon() {
    let banned: Vec<_> = ynz_registry::banned_jargon().collect();

    // Fixed-text portions of watch CLI messages (dynamic values like paths/sizes excluded).
    let watch_messages = [
        // ui.rs status lines
        "▶ Building",
        "✓ Built in",
        "✗ 1 error in",
        "✗ errors in",
        "✓ Watching",
        // event_loop.rs + watcher.rs log lines
        "file watcher channel closed unexpectedly; exiting",
        "file watcher error",
        "ynz watch: watching",
        "ynz watch: using filesystem polling on this mount; rebuild may lag",
        // memory.rs + lib.rs memory messages
        "memory warning",
        "this is the safety stop",
        "rebuild the compiler state more often",
        "This releases accumulated compiler cache more frequently",
        "memory polling unavailable on this platform",
        "hard-stop disabled for this session",
        // WatchError Display strings (error.rs)
        "ynz watch could not subscribe to",
        "Check that the path exists and is readable",
        "The file system watcher must be able to read the target path",
        "requires a `yinz.toml` at",
        "project boundary is undefined",
        "Pass a single `.ynz` file instead",
        "The compiled binary",
        "could not be executed",
        "Check that the temp directory",
        "The watch daemon reads source files",
        "Memory polling failed",
        "The memory hard-stop safety net",
        "An I/O operation failed in the watch daemon",
        "Check that the file system is accessible",
    ];

    let mut violations: Vec<String> = Vec::new();
    for msg in &watch_messages {
        let lower = msg.to_lowercase();
        for entry in &banned {
            let w = entry.name.to_lowercase();
            if contains_whole_word(&lower, &w) {
                violations.push(format!(
                    "Watch CLI message contains banned word {:?}: {:?}",
                    entry.name, msg
                ));
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "Banned jargon found in {} watch CLI message(s):\n{}\n\n\
             See design/compiler-errors.md for replacements.",
            violations.len(),
            violations.join("\n")
        );
    }
}

// WHY: `lsp_inlay_hint_hover_for` returns WHY strings that go directly into LSP
//      `InlayHint.tooltip` fields — user-facing text. The `_ =>` fallback arm of the
//      `why` match historically contained `"inferred"` (banned per vocabulary.md). The
//      `_` arm is not reachable through any real registry domain (all 3 placement_category
//      values have explicit arms), so the runtime-domain loop below can't reach it.
//      The source-scan test catches reintroduction to ANY arm, including the fallback.
//      The runtime test catches jargon in any output actually produced by real registry data.
#[test]
fn no_infer_jargon_in_lsp_inlay_hint_hover_why_source() {
    // Locate lib.rs relative to this test crate.
    // CARGO_MANIFEST_DIR = crates/ynz-diagnostics/; lib.rs is at crates/ynz-registry/src/lib.rs.
    let lib_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("ynz-registry")
        .join("src")
        .join("lib.rs");

    let source = std::fs::read_to_string(&lib_path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", lib_path.display()));

    // Locate the lsp_inlay_hint_hover_for function body.
    // We extract from the `fn lsp_inlay_hint_hover_for` line through the matching closing brace.
    let fn_start = source
        .find("pub fn lsp_inlay_hint_hover_for(")
        .unwrap_or_else(|| panic!("`lsp_inlay_hint_hover_for` not found in {}", lib_path.display()));

    let fn_source = &source[fn_start..];
    // Walk to the end of the function body by brace-counting.
    let mut depth: usize = 0;
    let mut fn_end = fn_source.len();
    for (i, ch) in fn_source.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth > 0 {
                    depth -= 1;
                }
                if depth == 0 {
                    fn_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let fn_body = &fn_source[..fn_end];

    // Collect all string literals inside the function body.
    let mut violations: Vec<String> = Vec::new();
    let mut in_str = false;
    let mut escaped = false;
    let mut current = String::new();

    for ch in fn_body.chars() {
        if escaped {
            escaped = false;
            if in_str {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '\\' if in_str => {
                escaped = true;
            }
            '"' if !in_str => {
                in_str = true;
                current.clear();
            }
            '"' if in_str => {
                let lower = current.to_lowercase();
                if lower.contains("inferred") || lower.contains("infers") {
                    violations.push(format!(
                        "WHY string in lsp_inlay_hint_hover_for contains banned jargon: {:?}",
                        current
                    ));
                }
                // Also check the stem `infer` as a whole word (not as part of `inferred`/`infers`).
                // Use a simple byte scan with word-boundary checks.
                let needle = b"infer";
                let bytes = lower.as_bytes();
                let nlen = needle.len();
                if bytes.len() >= nlen {
                    for start in 0..=(bytes.len() - nlen) {
                        if &bytes[start..start + nlen] == needle {
                            let end = start + nlen;
                            let left_ok = start == 0
                                || (!bytes[start - 1].is_ascii_alphanumeric()
                                    && bytes[start - 1] != b'_');
                            let right_ok = end >= bytes.len()
                                || (!bytes[end].is_ascii_alphanumeric() && bytes[end] != b'_');
                            if left_ok && right_ok {
                                violations.push(format!(
                                    "WHY string in lsp_inlay_hint_hover_for contains banned jargon stem \"infer\": {:?}",
                                    current
                                ));
                                break;
                            }
                        }
                    }
                }
                in_str = false;
                current.clear();
            }
            c if in_str => {
                current.push(c);
            }
            _ => {}
        }
    }

    if !violations.is_empty() {
        panic!(
            "Banned jargon in lsp_inlay_hint_hover_for WHY strings ({} violation(s)):\n{}\n\n\
             Replace `inferred`/`infers`/`infer` with plain English per vocabulary.md \
             (e.g. \"figured this out\" instead of \"inferred\").",
            violations.len(),
            violations.join("\n")
        );
    }
}

#[test]
fn no_banned_jargon_in_lsp_inlay_hint_hover_output() {
    // WHY: Complements the source-scan above. Exercises the actual return value of
    //      lsp_inlay_hint_hover_for for every real registry domain. Guards the WHAT
    //      field (sourced from description), WHAT-INSTEAD (example_hint_rendered), and
    //      WHY (the placement_category arm). If a description or example sneaks in
    //      banned jargon, this catches it at runtime, not just source-level.
    let banned: Vec<_> = ynz_registry::banned_jargon().collect();
    let mut violations: Vec<String> = Vec::new();

    for domain_entry in ynz_registry::muted_hint_domains() {
        let domain = domain_entry.domain;
        let tooltip = match ynz_registry::lsp_inlay_hint_hover_for(domain) {
            Some(t) => t,
            None => {
                violations.push(format!(
                    "lsp_inlay_hint_hover_for({domain:?}) returned None — domain is in registry but lookup failed"
                ));
                continue;
            }
        };
        let lower = tooltip.to_lowercase();
        for entry in &banned {
            let w = entry.name.to_lowercase();
            if contains_whole_word(&lower, &w) {
                violations.push(format!(
                    "lsp_inlay_hint_hover_for({domain:?}) tooltip contains banned word {:?}: {:?}",
                    entry.name, tooltip
                ));
            }
        }
        if lower.contains("inferred") {
            violations.push(format!(
                "lsp_inlay_hint_hover_for({domain:?}) tooltip contains banned jargon \"inferred\": {:?}",
                tooltip
            ));
        }
        if lower.contains("infers") {
            violations.push(format!(
                "lsp_inlay_hint_hover_for({domain:?}) tooltip contains banned jargon \"infers\": {:?}",
                tooltip
            ));
        }
    }

    if !violations.is_empty() {
        panic!(
            "Banned jargon in lsp_inlay_hint_hover_for output ({} violation(s)):\n{}\n\n\
             Edit the registry description/example fields or the placement_category WHY arms \
             per vocabulary.md.",
            violations.len(),
            violations.join("\n")
        );
    }
}

// WHY: `[[muted_hint_domain]]` description fields are surfaced to the user in the WHAT
//      section of LSP inlay-hint hover tooltips (via `lsp_inlay_hint_hover_for`).
//      A description containing `infer`/`inferred` or other banned jargon reaches the user
//      as readily as any `Diagnostic::error` string — same ban applies. This test catches
//      regressions when editing registry/features.toml directly, which the Diagnostic-site
//      scanner above doesn't reach.
#[test]
fn no_banned_jargon_in_muted_hint_domain_descriptions() {
    let banned: Vec<_> = ynz_registry::banned_jargon().collect();
    let mut violations: Vec<String> = Vec::new();

    for entry in ynz_registry::muted_hint_domains() {
        let lower = entry.description.to_lowercase();
        for jargon in &banned {
            let w = jargon.name.to_lowercase();
            if contains_whole_word(&lower, &w) {
                violations.push(format!(
                    "muted_hint_domain {:?} description contains banned word {:?}: {:?}",
                    entry.domain, jargon.name, entry.description
                ));
            }
        }
        // Also check the inflected verb forms and typo that slip past whole-word checks.
        if lower.contains("infers") {
            violations.push(format!(
                "muted_hint_domain {:?} description contains banned jargon verb \"infers\": {:?}",
                entry.domain, entry.description
            ));
        }
        if lower.contains("inferred") {
            violations.push(format!(
                "muted_hint_domain {:?} description contains banned jargon form \"inferred\": {:?}",
                entry.domain, entry.description
            ));
        }
        if lower.contains("booleanean") {
            violations.push(format!(
                "muted_hint_domain {:?} description contains typo \"booleanean\": {:?}",
                entry.domain, entry.description
            ));
        }
    }

    if !violations.is_empty() {
        panic!(
            "Banned jargon in {} muted_hint_domain description(s):\n{}\n\n\
             Edit registry/features.toml and replace with plain-English per vocabulary.md.",
            violations.len(),
            violations.join("\n")
        );
    }
}
