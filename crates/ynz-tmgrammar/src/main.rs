use std::path::Path;

fn main() {
    // Anchor to the workspace root so the generator works regardless of CWD.
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let out_path = workspace_root.join("tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json");

    let grammar = ynz_tmgrammar::build_grammar();
    let output = serde_json::to_string_pretty(&grammar).expect("serialize grammar");

    std::fs::create_dir_all(out_path.parent().unwrap()).expect("create syntaxes dir");
    std::fs::write(&out_path, &output).expect("write grammar file");

    println!("Generated {}", out_path.display());
}
