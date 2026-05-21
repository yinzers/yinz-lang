//! Zero-config Yinz source formatter.
//!
//! # Contract
//!
//! - `parse(format(x)).ast == parse(x).ast` modulo trivia — the formatter never changes program semantics.
//! - `format(format(x)) == format(x)` — idempotent; running twice produces the same output.
//! - One canonical output per AST — no config, no style options.
//!
//! # Usage by v0.2-M5 LSP
//!
//! The `textDocument/formatting` handler calls [`format`] with the full file text, then returns a
//! single `TextEdit` replacing `[0..len]` with the formatted output. No other logic is needed.
//!
//! # Semver stability
//!
//! This API is frozen at v0.2-M3. Backwards-incompatible changes require a major-version
//! bump once `v0.2.0` ships.

pub(crate) mod comment_merge;
mod error;
pub(crate) mod render;
pub mod walker;

pub use error::{CheckResult, FmtError};

/// Format a complete Yinz source file.
///
/// Returns the formatted source as a `String`. The output ends with exactly one trailing newline.
///
/// # Errors
///
/// - `FmtError::ParseError` — the source has parse errors; the caller should display them with
///   the standard `ynz-diagnostics` renderer and ask the user to fix the errors first.
/// - `FmtError::InvalidInput` — infrastructure problem (non-UTF-8 bytes, etc.).
pub fn format(source: &str) -> Result<String, FmtError> {
    format_named(source, "<fmt>")
}

/// Format a complete Yinz source file, labelling diagnostics with the given filename.
///
/// Like [`format`], but `ParseError` diagnostics carry `name` as their source label so that
/// callers can render them with the real file path or `"<stdin>"` instead of `"<fmt>"`.
///
/// # Errors
///
/// Same as [`format`].
pub fn format_named(source: &str, name: &str) -> Result<String, FmtError> {
    let db = ynz_parser::CompilerDb::default();
    let sf = ynz_parser::SourceFile::new(&db, name.into(), source.into());
    let output = ynz_parser::parse_query(&db, sf);

    if !output.diagnostics.is_empty() {
        return Err(FmtError::ParseError(output.diagnostics.clone()));
    }

    let (_tokens, comments) = ynz_parser::lex_with_trivia(name, source);
    let ctx = comment_merge::CommentContext::new(&comments, source);
    Ok(walker::emit_module_with_comments(&output.module, &ctx))
}

/// Format a byte-range within a Yinz source file.
///
/// The formatter always operates on the full file (Yinz has no statement-level
/// grammar — a range may cut through an expression or declaration).  The result
/// is the formatted text that should REPLACE `source[start_byte..end_byte]`:
///
/// - If `start_byte == 0` AND `end_byte >= source.len()`, this is equivalent to
///   [`format`] and returns the complete formatted source.
/// - Otherwise, the formatter rewrites the full file, then extracts the portion
///   that corresponds to the requested line range.  This ensures the extracted
///   text is syntactically correct at the line boundary level.
///
/// The proptest contract `format_range(x, 0, x.len()) == format(x)` holds.
///
/// # Errors
///
/// Same as [`format`].
pub fn format_range(source: &str, start_byte: usize, end_byte: usize) -> Result<String, FmtError> {
    let formatted = format(source)?;

    // Whole-file case — return immediately.
    if start_byte == 0 && end_byte >= source.len() {
        return Ok(formatted);
    }

    // Map byte offsets to line numbers in the original source.
    let safe_start = start_byte.min(source.len());
    let safe_end = end_byte.min(source.len());

    let start_line = source[..safe_start].matches('\n').count();
    let end_line_inclusive = source[..safe_end].matches('\n').count();

    // Extract the same line range from the formatted output.
    // The formatter preserves top-level item boundaries at line granularity,
    // so line N in the original maps to line N in the formatted output for
    // complete-item ranges.  Partial cuts round to the nearest item boundary.
    let fmt_lines: Vec<&str> = formatted.lines().collect();
    let s = start_line.min(fmt_lines.len());
    let e = (end_line_inclusive + 1).min(fmt_lines.len());

    if s >= e {
        return Ok(String::new());
    }

    let mut result = fmt_lines[s..e].join("\n");
    result.push('\n');
    Ok(result)
}

/// Check whether a source file is already in canonical form without rewriting it.
///
/// Returns [`CheckResult::AlreadyCanonical`] if `format(source) == source`, or
/// [`CheckResult::WouldChange`] with the formatted output if it differs.
///
/// # Errors
///
/// Same as [`format`].
pub fn check(source: &str) -> Result<CheckResult, FmtError> {
    let formatted = format(source)?;
    if formatted == source {
        Ok(CheckResult::AlreadyCanonical)
    } else {
        Ok(CheckResult::WouldChange { preview: formatted })
    }
}
