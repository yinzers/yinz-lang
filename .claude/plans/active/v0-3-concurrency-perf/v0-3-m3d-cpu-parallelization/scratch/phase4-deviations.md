# v0-3-m3d-cpu-parallelization Phase 4 Deviations — captured 2026-06-19 (Round 2)

D_count: 7

## Scope Deviations (verbatim from executor report — Round 2)

- **Scope Deviation #1** (`crates/ynz-typeck/src/cpu_admission.rs`, NEW file): outside BLOCK-named files. Rationale: FIX 1 mandates a single shared admission decision; crate direction is codegen→typeck so the predicate MUST live in typeck for both consumers to read it (plan line 62 authorizes this as "a relocation, not a parallel implementation"). New self-contained module = lowest blast radius. Diff hunks: crates/ynz-typeck/src/cpu_admission.rs:1-560.
- **Scope Deviation #2** (`crates/ynz-codegen/src/emit.rs`): beyond FIX-1 hint files. Rationale: relocation requires codegen's spike_* gate + 4 AST walkers to DELEGATE to the new typeck module (one-line bodies, signatures preserved so ~60 call sites + codegen tests untouched); removed orphaned helpers/imports. Diff hunks: crates/ynz-codegen/src/emit.rs:61-64, 3331-3335, 6779-6783, 6829-6835, 6858-6864, 6878-6880, 6897-6907, 6989-6999.
- **Scope Deviation #3** (`crates/ynz-codegen/tests/golden.rs`): binary-side parity proof. Rationale: FIX 1b "hint set == codegen spawn set" needs the binary half proven where codegen IR is reachable (typeck can't run codegen). Diff hunks: crates/ynz-codegen/tests/golden.rs:861-903.
- **Scope Deviation #4** (`crates/ynz-registry/build.rs`): emit new schema fields. Rationale: MUTED_HINT_DOMAINS is codegen'd from features.toml by build.rs; FIX-4 hover fields can't exist on the struct without build.rs writing them into the generated literal (mirrors keyword-entry hover-field pattern). Diff hunks: crates/ynz-registry/build.rs:434-453.

## Approach Deviations (verbatim from executor report — Round 2)

- **Approach Deviation #1** (FIX 1): full relocation of the pure-AST admission gate to ynz-typeck::cpu_admission with codegen delegating up, rather than re-deriving the decision separately in the inlay pass. Rationale: re-deriving = two implementations = no-duct-tape #7 drift; relocation is the genuine single-source-of-truth (plan line 62). Diff hunks: crates/ynz-typeck/src/cpu_admission.rs:1-560, crates/ynz-codegen/src/emit.rs:61-64, 3331-3335.
- **Approach Deviation #2** (FIX 4 WHY contextuality): a contextual-but-static WHY ("the listed lines have no shared reads or writes — neither reads what the other writes") rather than literal per-call-site binding names. Rationale: the registry hover is one static template per domain; runtime line numbers live in the inline muted label, not the registry tooltip; the tooltip carries the contextual RELATIONSHIP. Diff hunks: registry/features.toml:2104-2107.

## Carried deviation (from Round 1 — re-judge to confirm BLOCK cleared)

- **Carried Deviation (placeholder)** (`tooling/vscode-ynz/screenshots/cpu-parallel-hints.png.PLACEHOLDER`): Round-1 deviation-judge BLOCKED (ships in .vsix; unanchored deferral). Round-2 fix: `.vscodeignore` excludes `screenshots/*.PLACEHOLDER`; coordinator anchored the 4-field deferral in the plan (Patrick-approved). Re-judge must confirm the BLOCK is cleared. Diff hunks: tooling/vscode-ynz/.vscodeignore:1, tooling/vscode-ynz/screenshots/cpu-parallel-hints.png.PLACEHOLDER:1-11.

## Resolved spawn list (orchestrator's parsed view)

7 deviations. D > 4 → §3.d.1 consolidation gate fires. Coordinator routing: see plan Findings Log Round 2 entry for the individual-vs-consolidated decision. The 4 scope deviations are one logical relocation (best judged holistically across files); approach #1 (relocation correctness — does it preserve codegen IR byte-identically?), approach #2 (hover WHY quality / GR11), and the placeholder re-judge are the distinct adversarial targets.
