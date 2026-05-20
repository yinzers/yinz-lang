pub mod banned_jargon;
pub mod deferred_feature;
mod bucket;
mod diagnostic;
mod render;
mod span;

pub use bucket::DiagnosticBucket;
pub use diagnostic::{Diagnostic, DiagnosticKind, RelatedSpan, Severity};
pub use render::render;
pub use span::SourceSpan;
