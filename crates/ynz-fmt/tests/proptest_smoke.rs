//! Strategy quality gate: generates 100 ASTs from the proptest strategy and
//! asserts ≥95% produce non-trivial output (>20 bytes after formatting).
//!
//! WHY: catches "the generator emits empty modules 80% of the time" failures
//! up front, before the actual semantic-roundtrip proptest runs against those
//! trivial cases.
//!
//! This test uses the SAME `arb_module` from `tests/common/` as the idempotency
//! proptest so the gate actually guards the strategy it claims to guard.

mod common;
use common::arb_module;

use proptest::prelude::*;
use proptest::strategy::{Strategy, ValueTree};

/// Collect N generated module values from a strategy using a fixed seed.
fn sample_modules(n: usize) -> Vec<ynz_ast::nodes::Module> {
    let strategy = arb_module();
    let config = ProptestConfig::default();
    let mut runner = proptest::test_runner::TestRunner::new_with_rng(
        config,
        proptest::test_runner::TestRng::from_seed(
            proptest::test_runner::RngAlgorithm::default(),
            &[0u8; 32],
        ),
    );
    (0..n)
        .map(|_| {
            strategy
                .new_tree(&mut runner)
                .expect("strategy generation failed")
                .current()
        })
        .collect()
}

// test-ratchet: initial version of this test used proptest::strategy::ValueTree without the
// trait in scope (compile error); rewritten to import it and use the correct API.  A second
// rewrite moved the strategy into tests/common/ so this gate runs against the SAME arb_module
// as the idempotency proptest (not a cheaper fork).
#[test]
fn strategy_produces_nontrivial_output() {
    // WHY: guards against a degenerate strategy that emits mostly empty modules.
    // If <95% of cases produce >20 bytes, the proptest harness is testing trivia.
    let modules = sample_modules(100);
    let total = modules.len();
    let nontrivial = modules
        .iter()
        .filter(|m| ynz_fmt::walker::emit_module(m).len() > 20)
        .count();

    let pct = nontrivial * 100 / total;
    assert!(
        pct >= 95,
        "strategy quality gate failed: only {pct}% of {total} generated modules produced \
         non-trivial output (>20 bytes); strategy may be biased toward empty modules"
    );
}
