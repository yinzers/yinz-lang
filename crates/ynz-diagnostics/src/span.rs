/// A location in source code: a file name plus a byte range.
///
/// Byte offsets are 0-indexed. `end` is exclusive (past the last byte of the span).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    /// The source file this span belongs to.
    pub file: String,
    /// First byte of the span (inclusive, 0-indexed).
    pub start: usize,
    /// One past the last byte of the span (exclusive).
    pub end: usize,
}

impl SourceSpan {
    pub fn new(file: impl Into<String>, start: usize, end: usize) -> Self {
        Self {
            file: file.into(),
            start,
            end,
        }
    }
}

impl ariadne::Span for SourceSpan {
    type SourceId = str;

    fn source(&self) -> &str {
        &self.file
    }

    fn start(&self) -> usize {
        self.start
    }

    fn end(&self) -> usize {
        self.end
    }
}
