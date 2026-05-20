pub mod banned_jargon;
mod bucket;
pub mod deferred_feature;
mod diagnostic;
mod render;
mod span;

pub use bucket::DiagnosticBucket;
pub use diagnostic::{Diagnostic, DiagnosticKind, RelatedSpan, Severity};
pub use render::render;
pub use span::SourceSpan;
