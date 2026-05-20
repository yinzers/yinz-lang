use std::path::Path;

/// Stub formatter handler — prints "not yet implemented" and exits 1.
pub fn fmt(_path: Option<&Path>, _all: bool, _check: bool, _stdin: bool) -> i32 {
    eprintln!("ynz fmt: not yet implemented");
    1
}
