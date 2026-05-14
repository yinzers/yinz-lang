use crate::diagnostic::{Diagnostic, Severity};

/// Maximum number of errors before additional errors are hidden.
/// Per `design/compiler-errors.md`: "if a file has more than 50 errors, stop after 50."
const ERROR_CAP: usize = 50;

/// Accumulates diagnostics from a single compilation pass.
///
/// Errors are capped at 50. After the cap is reached, further errors increment
/// `hidden_count` and are dropped. Warnings and suggestions are never capped.
///
/// After a full pass, call `has_errors()` to decide whether to proceed to the next
/// compiler stage.
#[derive(Debug, Default)]
pub struct DiagnosticBucket {
    diagnostics: Vec<Diagnostic>,
    hidden_count: usize,
}

impl DiagnosticBucket {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a diagnostic. Errors beyond the cap are silently dropped (counted in
    /// `hidden_count`). Warnings and suggestions are always accepted.
    pub fn push(&mut self, diag: Diagnostic) {
        if diag.severity == Severity::Error {
            let current_errors = self
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count();
            if current_errors >= ERROR_CAP {
                self.hidden_count += 1;
                return;
            }
        }
        self.diagnostics.push(diag);
    }

    /// Returns `true` if any Error-severity diagnostics are present.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Number of errors that were dropped due to the cap.
    pub fn hidden_count(&self) -> usize {
        self.hidden_count
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }
}

impl IntoIterator for DiagnosticBucket {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.into_iter()
    }
}
