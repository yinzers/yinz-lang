// Rust equivalent of the C7 release-overhead workload `nested_cell`: 2^22 iterations, each building
// a shape that holds a nested shape (a heap cell in Yinz, a `Box` here), reading it back and
// releasing both at the end of the iteration. total = sum_{i=1}^{reps} (i + 3).
// `black_box` keeps the allocation, as Yinz's nested cell is a real `ynz_alloc`.

use std::hint::black_box;

struct Tag {
    id: i64,
}

struct Wrap {
    inner: Box<Tag>,
    count: i64,
}

fn main() {
    let reps: i64 = 4_194_304;
    let mut acc: i64 = 0;
    for i in 1..=reps {
        let wrap = Wrap {
            inner: Box::new(Tag { id: i }),
            count: 3,
        };
        let wrap = black_box(wrap);
        acc += wrap.inner.id + wrap.count;
    }
    println!("{acc}");
}
