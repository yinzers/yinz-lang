# macOS Platform Support (Deferred)

**Status:** deferred. macOS was removed from CI on 2026-06-01; Linux (x86_64) is the only
verified target. This doc records what's missing and what re-adding macOS requires, so the
gap is tracked rather than forgotten.

Spec: none yet — this is contributor-facing infrastructure, not a user-facing language feature.

---

## What happened

CI ran a `[ubuntu-latest, macos-latest]` matrix. For a long time the whole matrix was red at
the `cargo fmt --check` step, so nothing past it ever ran. After the 2026-06-01 CI repair
(fmt drift, clippy `-D warnings`, the runtime-archive bootstrap, snapshot abs-paths,
macOS siphash entropy), Ubuntu went fully green and the macOS job finally reached `cargo test`
— where **21 tests in `crates/ynz-codegen/tests/golden.rs` failed**.

macOS was dropped from the matrix rather than gated test-by-test, because the failures are
**not all** stale golden bytes — some indicate behaviour that can't be verified from a Linux
host (see below). Gating the tests off macOS would have asserted "macOS works except these
goldens," which we cannot currently stand behind.

## The two failure classes

1. **Target-specific golden VALUES (expected to differ).**
   - **IR-text snapshots** (`insta::assert_snapshot!("…_ir", artifact.ir_text)`, ~13 tests).
     The LLVM IR text embeds the target triple + datalayout + calling-convention details, so
     it legitimately differs on macOS. These tests are self-described as *informational*
     ("the SHA-256 test is the gate, not this one").
   - **Object-file SHA-256 goldens** (`load_golden`/`save_golden`, keyed by `triple_slug()` =
     `arch-os`). The committed goldens are only `*.x86_64-linux.sha256`. The harness is
     designed to *auto-record* a fresh golden when the per-triple file is missing — so on a
     first macOS run these should record-and-pass.

2. **Possible REAL macOS codegen differences (cannot verify from Linux).**
   - `m3_fib_sha256_golden` failed even though it auto-records when the macOS golden is
     missing — the only way it fails is `run_m3_fib_codegen()` returning `None`, i.e. codegen
     emitted diagnostics on macOS.
   - `m4_player_ir_has_readonly_on_share_param` is a substring assertion
     (`ir.contains("readonly") && ir.contains("noalias")`), not a snapshot, and it failed —
     suggesting the `readonly`/`noalias` attribute emission or IR shape differs on macOS.

   These are the reason we did not simply gate the tests: they may be real bugs, and confirming
   that needs an actual macOS runner.

## What re-adding macOS requires

When a Mac (or a macOS CI runner someone can iterate on) is available:

1. **Diagnose the non-golden failures first.** Run `cargo test -p ynz-codegen --test golden`
   on macOS and confirm whether `m3_fib_sha256_golden` / `m4_player_ir_has_readonly_on_share_param`
   are real codegen differences or environment issues (LLVM version, target setup). Fix any real
   codegen bug before trusting the platform.
2. **Record per-triple SHA goldens.** The infra already supports this: run on macOS with the
   golden file absent and the harness writes `*.{aarch64-macos}.sha256` (and x86_64-macos if that
   runner is used). Commit them.
3. **Make the IR-text snapshots target-aware.** insta uses one snapshot file per name; the IR
   text is target-specific. Options: name snapshots per-triple, gate the IR-text snapshots to the
   recording host, or redact the target-specific IR header. (They're informational, so gating to
   the golden host is acceptable.)
4. **Re-add `macos-latest`** to the matrix in `.github/workflows/ci.yml` and confirm green.

## Why Linux-only is acceptable for now

Yinz targets LLVM native machine code; x86_64-linux is the primary development and test target.
The compiler is not yet at a milestone where macOS is a shipped target. Linux-only CI verifies
the full pipeline (fmt, clippy, build, 110+ test binaries, `--version`) on the target that
actually matters today, without pretending macOS is validated.

## Cross-references

- `.github/workflows/ci.yml` — the matrix (currently `[ubuntu-latest]`)
- `crates/ynz-codegen/tests/golden.rs` — the golden/IR-snapshot tests + per-triple `load_golden`
- `crates/ynz-codegen/tests/__golden__/` — committed SHA goldens (`*.x86_64-linux.sha256`)
- `.claude/todos.md` — "macos-ci-codegen-support" tracked item
- `design/future/index.md` — future-locked design index
