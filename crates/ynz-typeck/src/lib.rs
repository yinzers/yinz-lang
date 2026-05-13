pub mod check;
pub mod intrinsics;
pub mod queries;
pub mod return_paths;
pub mod scope;
pub mod signatures;
pub mod types;

pub use check::{check, TypedModule};
pub use intrinsics::PrimitiveIntrinsicTable;
pub use queries::{check_query, module_signatures_query, CheckOutput, SignatureOutput};
pub use signatures::SignatureTable;
pub use types::Type;
