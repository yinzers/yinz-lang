pub mod banned_jargon;
mod bucket;
mod diagnostic;
mod render;
mod span;

pub use bucket::DiagnosticBucket;
pub use diagnostic::{Diagnostic, RelatedSpan, Severity};
pub use render::render;
pub use span::SourceSpan;
