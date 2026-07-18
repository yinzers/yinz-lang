// Rust equivalent of opt_pipeline_calibration.rs's shape_alloc workload: 2^23
// iterations, each constructing a 3-field struct literal in the loop body and
// reading all three fields back; total = 3*reps*(reps+1)/2 + 3*reps. Same
// hardcoded rep count as the Yinz source; idiomatic Rust shape (a plain struct
// literal is stack-constructed, mirroring the Yinz shape literal's allocas).

struct Point {
    x: i64,
    y: i64,
    z: i64,
}

fn main() {
    let reps: i64 = 8_388_608;
    let mut acc: i64 = 0;
    for i in 1..=reps {
        let p = Point {
            x: i,
            y: i + i,
            z: 3,
        };
        acc += p.x + p.y + p.z;
    }
    println!("{acc}");
}
