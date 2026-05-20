//! Primitive rendering helpers used by the AST walker.

pub const LINE_WIDTH: usize = 100;

/// Returns `n * 2` spaces as an indentation string.
pub fn indent(n: usize) -> String {
    "  ".repeat(n)
}

/// Decide whether the given string would fit on one line.
pub fn fits_on_one_line(s: &str) -> bool {
    s.len() <= LINE_WIDTH && !s.contains('\n')
}
