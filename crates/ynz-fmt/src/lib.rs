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

mod error;

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
/// - `FmtError::Unimplemented` — the formatter is not yet wired up.
pub fn format(_source: &str) -> Result<String, FmtError> {
    Err(FmtError::Unimplemented)
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
