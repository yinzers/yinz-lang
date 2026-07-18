// Rust equivalent of opt_pipeline_calibration.rs's cpu_loop workload: 2^24
// iterations of rem + add accumulation, print the closed-form-checkable total
// ((reps/7)*28 + r(r+1)/2 where r = reps % 7). Same hardcoded rep count as the
// Yinz source; loop written the way a Rust developer would (for over 0..reps),
// not transliterated — the comparison is idiomatic-vs-idiomatic by design.

fn main() {
    let reps: i64 = 16_777_216;
    let mut acc: i64 = 0;
    for i in 0..reps {
        acc += (i % 7) + 1;
    }
    println!("{acc}");
}
