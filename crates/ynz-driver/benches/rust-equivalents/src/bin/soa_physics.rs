// Rust equivalent of opt_pipeline_calibration.rs's soa_physics workload: the
// M5-characterized Player hot-x/y scan — 64 heap-stored 64-byte 8-field
// structs, read-accumulate x+y over the whole collection, repeated 262,144
// times (16.8M element visits); total = reps * 3 * n(n+1)/2. Vec<Player>
// mirrors Yinz's heap `array<Player>` (AoS layout — this program is the
// idiomatic-Rust baseline a Yinz SoA-admitted binary is compared against).

// The six cold fields are never read — they exist to reproduce the Yinz
// Player's 64-byte stride, which is the whole point of the workload.
#[allow(dead_code)]
struct Player {
    x: i64,
    y: i64,
    dx: i64,
    dy: i64,
    health: i64,
    stamina: i64,
    gold: i64,
    level: i64,
}

fn main() {
    let players: Vec<Player> = (1..=64)
        .map(|i| Player {
            x: i,
            y: 2 * i,
            dx: 1,
            dy: 1,
            health: 100,
            stamina: 50,
            gold: 7,
            level: 3,
        })
        .collect();
    let reps: i64 = 262_144;
    let mut acc: i64 = 0;
    for _ in 0..reps {
        for p in &players {
            acc += p.x + p.y;
        }
    }
    println!("{acc}");
}
