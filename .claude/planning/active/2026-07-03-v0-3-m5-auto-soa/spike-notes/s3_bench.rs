//! M5 P0 S3 — bench noise probe (AoS scan vs contiguous/SoA scan).
//!
//! Raw Rust microbench, no harness deps: measures the run-to-run variance of the
//! Docker-on-WSL2 `dev` container environment so Phase 6's SIZE_THRESHOLD
//! calibration knows its noise floor (risk E2: a threshold delta smaller than the
//! noise floor is not evidence). Kept in the plan dir for Phase 6 reuse.
//!
//! Build + run (from /work in the dev container):
//!   rustc -O -o /tmp/s3_bench .claude/planning/active/2026-07-03-v0-3-m5-auto-soa/spike-notes/s3_bench.rs
//!   /tmp/s3_bench
use std::hint::black_box;
use std::time::Instant;

// 64-byte element: one hot i64 field + 7 cold i64 fields — the AoS worst case a
// 2-field-hot-loop SoA transform targets (hot field uses 1/8 of each cache line).
#[repr(C)]
struct Part {
    hot: i64,
    cold: [i64; 7],
}

const N: usize = 1_000_000;
const REPS: usize = 12;

fn main() {
    // AoS: Vec<Part>, sum the hot field.
    let aos: Vec<Part> = (0..N)
        .map(|i| Part {
            hot: i as i64,
            cold: [i as i64; 7],
        })
        .collect();
    // Contiguous (SoA segment): Vec<i64> of the hot field only.
    let soa: Vec<i64> = (0..N).map(|i| i as i64).collect();

    // Warmup (touch everything once; first-touch page faults out of the timing).
    let mut w = 0i64;
    for p in &aos {
        w = w.wrapping_add(p.hot);
    }
    for v in &soa {
        w = w.wrapping_add(*v);
    }
    black_box(w);

    let mut aos_ns: Vec<u128> = Vec::new();
    let mut soa_ns: Vec<u128> = Vec::new();
    for _ in 0..REPS {
        let t = Instant::now();
        let mut s = 0i64;
        for p in &aos {
            s = s.wrapping_add(black_box(p.hot));
        }
        black_box(s);
        aos_ns.push(t.elapsed().as_nanos());

        let t = Instant::now();
        let mut s = 0i64;
        for v in &soa {
            s = s.wrapping_add(black_box(*v));
        }
        black_box(s);
        soa_ns.push(t.elapsed().as_nanos());
    }

    report("aos", &aos_ns);
    report("soa", &soa_ns);
}

fn report(name: &str, ns: &[u128]) {
    let mean = ns.iter().sum::<u128>() as f64 / ns.len() as f64;
    let var = ns
        .iter()
        .map(|&x| {
            let d = x as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / ns.len() as f64;
    let sd = var.sqrt();
    let min = *ns.iter().min().unwrap();
    let max = *ns.iter().max().unwrap();
    println!(
        "{name}: reps={} mean_us={:.1} sd_us={:.1} cv={:.1}% min_us={:.1} max_us={:.1} spread={:.1}%",
        ns.len(),
        mean / 1e3,
        sd / 1e3,
        100.0 * sd / mean,
        min as f64 / 1e3,
        max as f64 / 1e3,
        100.0 * (max - min) as f64 / min as f64
    );
    let list: Vec<String> = ns.iter().map(|x| format!("{:.0}", *x as f64 / 1e3)).collect();
    println!("{name}_all_us: [{}]", list.join(", "));
}
