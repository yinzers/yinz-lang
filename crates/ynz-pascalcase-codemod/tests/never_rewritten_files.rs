//! The codemod must leave the two test files that hold old spellings on purpose alone, however
//! the root is given, without skipping files that merely look similar.

use std::path::{Path, PathBuf};
use std::process::Command;

const OLD_SPELLING_SOURCE: &str =
    "pub const SOURCE: &str = \"function f(x: int) -> int {\\n  return x\\n}\\n\";\n";

const PROTECTED_FILES: &[&str] = &[
    "crates/ynz-typeck/tests/builtin_type_names.rs",
    "crates/ynz-lsp/tests/old_builtin_type_name_quick_fix.rs",
];

/// Same trailing names or same directory as a protected file, but not protected: a name-only or
/// substring match would wrongly skip these.
const LOOK_ALIKE_FILES: &[&str] = &[
    "crates/other/tests/builtin_type_names.rs",
    "xcrates/ynz-typeck/tests/builtin_type_names.rs",
    "crates/ynz-typeck/tests/other.rs",
];

fn build_tree(name: &str) -> PathBuf {
    let tree_root =
        std::env::temp_dir().join(format!("pascalcase-codemod-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tree_root);
    for relative in PROTECTED_FILES.iter().chain(LOOK_ALIKE_FILES) {
        let file = tree_root.join(relative);
        std::fs::create_dir_all(file.parent().expect("file has a parent")).expect("create dirs");
        std::fs::write(&file, OLD_SPELLING_SOURCE).expect("write file");
    }
    tree_root
}

/// Files the dry run reports renames for, as paths relative to the tree root, sorted.
fn files_reported_for_rewrite(tree_root: &Path, root_argument: &str) -> Vec<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_pascalcase-codemod"))
        .args(["--dry-run", "--mode", "rust"])
        .arg(root_argument)
        .current_dir(tree_root)
        .output()
        .expect("run the codemod");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 output");
    let mut reported: Vec<String> = stdout
        .lines()
        .filter_map(|line| line.split_once(": 2 renames"))
        .map(|(path, _)| {
            let path = Path::new(path);
            let relative = path.strip_prefix(tree_root).unwrap_or(path);
            let relative = relative.strip_prefix(".").unwrap_or(relative);
            relative.to_string_lossy().into_owned()
        })
        .collect();
    reported.sort();
    reported
}

macro_rules! never_rewritten_case {
    ($test_name:ident, $root_argument:expr) => {
        // bug: the codemod rewrites a never-rewritten test file, or skips a look-alike file, when the root is not the plain repo-relative path | tier: Medium
        #[test]
        fn $test_name() {
            let tree_root = build_tree(stringify!($test_name));
            let root_argument = $root_argument(&tree_root);
            let reported = files_reported_for_rewrite(&tree_root, &root_argument);
            let _ = std::fs::remove_dir_all(&tree_root);
            let mut expected: Vec<String> = LOOK_ALIKE_FILES
                .iter()
                .map(|file| file.to_string())
                .collect();
            expected.sort();
            assert_eq!(reported, expected);
        }
    };
}

never_rewritten_case!(
    root_given_as_dot_skips_only_the_protected_files,
    |_: &Path| ".".to_string()
);
never_rewritten_case!(
    root_given_as_absolute_path_skips_only_the_protected_files,
    |tree_root: &Path| tree_root.to_string_lossy().into_owned()
);
