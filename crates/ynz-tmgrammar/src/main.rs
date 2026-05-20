mod grammar;

use std::path::Path;

fn main() {
    let out_path = Path::new("tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json");

    let grammar = grammar::build_grammar();
    let output = serde_json::to_string_pretty(&grammar).expect("serialize grammar");

    std::fs::create_dir_all(out_path.parent().unwrap()).expect("create syntaxes dir");
    std::fs::write(out_path, &output).expect("write grammar file");

    println!("Generated {}", out_path.display());
}
