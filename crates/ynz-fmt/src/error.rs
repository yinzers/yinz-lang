use ynz_diagnostics::DiagnosticBucket;

/// The result of a `check()` call — either already canonical or would change.
#[derive(Debug)]
pub enum CheckResult {
    AlreadyCanonical,
    WouldChange { preview: String },
}

/// All errors the formatter can return.
#[derive(Debug)]
pub enum FmtError {
    /// The source has parse errors; fix those first.
    ParseError(DiagnosticBucket),
    /// Infrastructure error (non-UTF-8 input, missing file, etc.).
    InvalidInput(String),
}

impl std::fmt::Display for FmtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FmtError::ParseError(_) => write!(f, "source has parse errors; fix those first"),
            FmtError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
        }
    }
}

impl std::error::Error for FmtError {}
