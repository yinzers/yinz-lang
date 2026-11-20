// Rust equivalent of the C7 release-overhead workload `channel_share`: 2^22 iterations, each taking
// one counted channel reference from a function that hands it back and giving it back at the end of
// the iteration (Yinz: `ynz_channel_share` at the callee's `return`, `ynz_channel_free` at the scope
// exit; Rust: `Arc::clone` and its drop). total = reps.

use std::hint::black_box;
use std::sync::Arc;

struct Channel {
    capacity: i64,
}

fn pass_on(channel: &Arc<Channel>) -> Arc<Channel> {
    Arc::clone(channel)
}

fn main() {
    let reps: i64 = 4_194_304;
    let channel = Arc::new(Channel { capacity: 64 });
    let mut acc: i64 = 0;
    for _ in 1..=reps {
        let held = pass_on(black_box(&channel));
        acc += held.capacity / 64;
    }
    println!("{acc}");
}
