# Reliability Analysis — Yinz Compiler

Scope: `crates/ynz-driver/src/{build,run,load,main}.rs`, `crates/ynz-codegen/src/{emit,queries,artifact}.rs`, integration tests.

Cross-boundary surfaces in this codebase are file-system writes, subprocess invocation (linker), and LLVM FFI. No DB, no network, no queue.

---

## Finding 1 — High: `.o` file stranded when `rt_lib` write or linker probe fails

- **File**: `crates/ynz-driver/src/build.rs:295` (object file written), `:311-318` (rt_lib write failure returns without cleanup), `:321-331` (linker-not-found returns without cleanup)
- **Boundary**: filesystem write
- **Failure**: `obj_path` written successfully at L295, then `fs::write(rt_lib_tmp, ...)` at L311 fails (full /tmp, RO tmpfs) → `build_failed()` at L318 returns without cleaning `obj_path`. Same gap L321-330 for missing linker. The cleanup at L352 is only reached if linker actually runs.
- **Recovery gap**: `obj_path` persists silently next to user source; can clobber a same-named `.o` from a different toolchain
- **Pattern**: artifact written → side effect fails → artifact remains with no record

## Finding 2 — Medium: Partial/corrupt output binary after linker crash

- **File**: `crates/ynz-driver/src/build.rs:333-373` (single-file), `:139-171` (project path via `link_objects`)
- **Boundary**: linker subprocess writing output binary in-place
- **Failure**: Most linkers write the output in-place, not atomically. If linker crashes / killed mid-write, partial binary left at `binary_path`. Error paths surface diagnostic but never `remove_file(&binary_path)`. User running the partial binary post-failure executes a corrupt file.
- **Recovery gap**: No cleanup of `binary_path` on link failure. Self-heals on next successful build.

## Finding 3 — Medium: Concurrent `ynz build` races on same `obj_path`

- **File**: `crates/ynz-driver/src/build.rs:294` (`obj_path = source_path.with_extension("o")`)
- **Boundary**: filesystem write
- **Failure**: Two concurrent invocations on `hello.ynz` both derive `hello.o`; concurrent writes → linker reads interleaved bytes → corrupt binary or link error. Cleanup `remove_file` also races but errors silently.
- **Recovery gap**: No locking, no temp dir, no PID/nonce-qualified name. Integration tests use per-test temp dirs (awareness exists in test infra but not prod code).

## Finding 4 — Medium: Project `.o` files pollute source tree on SIGKILL

- **File**: `crates/ynz-driver/src/build.rs:115` (`entry.path.with_extension("o")`)
- **Boundary**: filesystem write
- **Failure**: Every source file in `src/` produces a sibling `.o`. Cleanup loops at L129-133 and L145-147 run under normal termination, but SIGKILL/OOM/^C between writes and cleanup orphans N-1 `.o` files next to user sources.
- **Recovery gap**: Self-heals on next successful build but leaves visible artifact pollution. Fix: write intermediates to per-build temp dir like the integration tests do.

## Finding 5 — Low: PID-based temp runtime lib name

- **File**: `crates/ynz-driver/src/build.rs:310`, `:182`
- **Note**: `libynz_runtime_{pid}.a` is predictable but non-exploitable in build-tool context. Real gap is cleanup-on-abnormal-termination (Finding 4 covers this).

## Finding 6 — Non-finding: LLVM context lifetime correctly scoped

- **File**: `crates/ynz-codegen/src/emit.rs:62-111`
- File's own comment documents the constraint. Context drops at function exit; `CodegenOutput` returned to Salsa contains only `Vec<u8>` + `String`. Golden test asserts `CompiledArtifact: Send + Sync`. No leak.

## Finding 7 — Low: Partial `.o` from failed `fs::write` not cleaned

- **File**: `crates/ynz-driver/src/build.rs:116-126`
- **Failure**: Out-of-disk during `fs::write` returns `Err` with partial file on disk; path was never pushed to `object_files`, so cleanup skips it. Self-healing on next successful build.

---

## Out-of-scope (does not apply to this codebase)

- **Salsa cache corruption**: in-process only (`CompilerDb::default()` per invocation, no on-disk cache).
- **External API partial failure**: no network, no queue, no DB.

## Summary

| # | Severity | File | Issue |
|---|---|---|---|
| 1 | High | `ynz-driver/build.rs:295/311-331` | `.o` stranded when rt_lib or linker probe fails |
| 2 | Medium | `ynz-driver/build.rs:333-373/139-171` | Partial binary left after linker crash |
| 3 | Medium | `ynz-driver/build.rs:294` | Concurrent builds race on same `obj_path` |
| 4 | Medium | `ynz-driver/build.rs:115` | Project `.o` files orphan on SIGKILL |
| 5 | Low | `ynz-driver/build.rs:310/182` | PID-named temp lib (non-exploitable) |
| 7 | Low | `ynz-driver/build.rs:116-126` | Partial `.o` from failed write not cleaned |
