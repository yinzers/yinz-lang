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
