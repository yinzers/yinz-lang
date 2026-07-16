//! R1 build-blocking grep-audit — the channel/handle (conduit) runtime path must contain
//! ZERO synchronous blocking calls in production code.
//!
//! # Why this test exists
//!
//! v0.3-M2 was HALTED because a `block_on` sync bridge shipped in direct contradiction of the
//! no-function-coloring design (the whole-program may-block model). The v0.3-M4 plan's R1 risk
//! row mandates a grep-audit of the send/recv/handle path for synchronous blocking calls; this
//! test is that audit made permanent and build-blocking instead of a one-time manual grep. Every
//! suspension in the conduit path must route through poll-yield state machines with real
//! endpoint futures and forwarded wakers (the `ynz_rt_async_sleep_poll` discipline) — a
//! synchronous block anywhere in it is the M2 corpse class and can wedge the cooperative pool
//! (the composed-suspension deadlock, risk R5).
//!
//! # Scope
//!
//! Scans `src/channel.rs` + `src/handle.rs` — the two modules that ARE the conduit runtime
//! path. `src/runtime.rs` is deliberately NOT scanned: it legitimately contains
//! `spawn_blocking` (the CPU pool is the blocking pool by design). Test modules
//! (`#[cfg(test)]` onward) are excluded — the test harness legitimately uses
//! `Runtime::block_on` to drive spawned tasks — and comment lines are excluded (the modules'
//! own docs NAME the banned calls).

use std::fs;
use std::path::Path;

/// The conduit-path modules under audit.
const CONDUIT_MODULES: &[&str] = &["src/channel.rs", "src/handle.rs"];

/// The banned synchronous-blocking tokens (the R1 class). `park` is matched as `.park(` /
/// `::park(` / `park_timeout` so the word inside identifiers like `parked_sends` cannot
/// false-positive.
const BANNED_TOKENS: &[&str] = &[
    "block_on",
    "blocking_send",
    "blocking_recv",
    "spawn_blocking",
    "thread::sleep",
    ".park(",
    "::park(",
    "park_timeout",
];

/// Production-code lines only: everything BEFORE the first `#[cfg(test)]` attribute, minus
/// comment lines. Yields `(1-based line number, line)`.
fn production_lines(contents: &str) -> impl Iterator<Item = (usize, &str)> {
    contents
        .lines()
        .enumerate()
        .take_while(|(_, l)| !l.trim_start().starts_with("#[cfg(test)]"))
        .filter(|(_, l)| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("//!") || t.starts_with("///"))
        })
        .map(|(i, l)| (i + 1, l))
}

#[test]
#[cfg_attr(
    miri,
    ignore = "pure static-analysis grep-audit over source-file text — no unsafe/concurrency \
              surface for Miri to exercise, and it needs real fs::read_to_string, which Miri's \
              default isolation blocks (`open` not available when isolation is enabled; see \
              https://github.com/rust-lang/miri#isolation). Real coverage of the conduit path's \
              actual runtime behavior comes from m2_runtime.rs / m2_spike.rs, which DO run under \
              Miri; this file only checks the checked-in source text."
)]
fn conduit_runtime_path_has_zero_synchronous_blocking_calls() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mut offenders: Vec<String> = Vec::new();

    for module in CONDUIT_MODULES {
        let path = Path::new(manifest_dir).join(module);
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("conduit module {} must exist", path.display()));
        for (line_no, line) in production_lines(&contents) {
            for token in BANNED_TOKENS {
                if line.contains(token) {
                    offenders.push(format!(
                        "{module}:{line_no} → `{token}` in: {}",
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "R1 audit: a synchronous blocking call was found in the conduit (channel/handle) \
         runtime path. Every suspension there must be a poll-yield with a real endpoint future \
         and a forwarded waker — a synchronous block is the v0.3-M2 `block_on` HALT class and \
         can wedge the cooperative pool (composed-suspension deadlock, R5). Route through the \
         poll protocol instead. Offending site(s):\n{}",
        offenders.join("\n")
    );
}

/// Guard the guard: the scan actually sees non-trivial production code in BOTH modules — a
/// file move or an accidental leading `#[cfg(test)]` must not let the audit silently pass
/// over nothing.
#[test]
#[cfg_attr(
    miri,
    ignore = "same fs::read_to_string isolation limitation as \
              conduit_runtime_path_has_zero_synchronous_blocking_calls above — this is the guard-\
              the-guard test for that same pure-text audit, not a runtime/concurrency check."
)]
fn conduit_audit_scans_real_production_code() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    for module in CONDUIT_MODULES {
        let path = Path::new(manifest_dir).join(module);
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("conduit module {} must exist", path.display()));
        let count = production_lines(&contents).count();
        assert!(
            count > 50,
            "{module}: expected substantial production code before the test module; \
             got only {count} scannable lines — did the module move or get test-gated?"
        );
    }
}
