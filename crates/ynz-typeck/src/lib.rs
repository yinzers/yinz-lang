pub mod check;
pub mod intrinsics;
pub mod queries;
pub mod scope;
pub mod types;

pub use check::{check, TypedModule};
pub use intrinsics::PrimitiveIntrinsicTable;
pub use queries::{check_query, CheckOutput};
pub use types::Type;
