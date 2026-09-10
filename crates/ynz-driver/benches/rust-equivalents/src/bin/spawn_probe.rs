// Rust spawn-overhead probe — the twin of the harness's Yinz reps=0 overhead
// baseline: prints 0 and exits. Each language's workload numbers are read net
// of its OWN spawn baseline; the difference between the two baselines is the
// measured Yinz runtime-init cost (a named comparability item in the
// raw-numbers record).

fn main() {
    println!("0");
}
