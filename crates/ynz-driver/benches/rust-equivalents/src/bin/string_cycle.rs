// Rust equivalent of the C7 release-overhead workload `string_cycle`: 2^22 iterations, each building
// a heap string from the loop counter and releasing it at the end of the iteration.
// total = sum of the decimal digit counts of 1..=reps.

fn main() {
    let reps: i64 = 4_194_304;
    let mut acc: i64 = 0;
    for i in 1..=reps {
        let text = i.to_string();
        acc += text.len() as i64;
    }
    println!("{acc}");
}
