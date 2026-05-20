/// Helper binary: run ynz_fmt::format on a .ynz file and write the .formatted output.
/// Usage: gen_golden <path.ynz>
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: gen_golden <path.ynz>");
        std::process::exit(1);
    }
    let source = std::fs::read_to_string(&args[1]).expect("could not read file");
    match ynz_fmt::format(&source) {
        Ok(out) => print!("{out}"),
        Err(e) => {
            eprintln!("format error: {e}");
            std::process::exit(1);
        }
    }
}
