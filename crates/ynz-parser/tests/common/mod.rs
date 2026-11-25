//! Helpers shared by the parser integration tests (`lex.rs`, `parse.rs`).

/// How many diagnostics a table case expects: a plain `usize` means exactly that many.
pub trait ExpectedDiagnosticCount {
    fn accepts(&self, actual: usize) -> bool;
}

impl ExpectedDiagnosticCount for usize {
    fn accepts(&self, actual: usize) -> bool {
        actual == *self
    }
}

/// At least this many diagnostics; for inputs whose exact count is incidental.
pub struct AtLeast(pub usize);

impl ExpectedDiagnosticCount for AtLeast {
    fn accepts(&self, actual: usize) -> bool {
        actual >= self.0
    }
}
