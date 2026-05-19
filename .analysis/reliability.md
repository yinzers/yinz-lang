# Reliability Analysis — Yinz Compiler

Scope: `crates/ynz-driver/src/{build,run,load,main}.rs`, `crates/ynz-codegen/src/{emit,queries,artifact}.rs`, integration tests.

Cross-boundary surfaces in this codebase are file-system writes, subprocess invocation (linker), and LLVM FFI. No DB, no network, no queue.

---

## ~~Finding 1 — High: `.o` file stranded when `rt_lib` write or linker probe fails~~ FIXED (Batch 4b)

`CleanupGuard` drop impl on each object path ensures cleanup on any return path. `NamedTempFile` handles runtime lib cleanup. All `.o` intermediates are in `tempdir()` — not in source tree at all.

## ~~Finding 2 — Medium: Partial/corrupt output binary after linker crash~~ FIXED (Batch 4b)

Both `build_project` and `build_single_file` now call `fs::remove_file(&binary_path)` on the linker-failure arm. Self-heals on success-path too.

## ~~Finding 3 — Medium: Concurrent `ynz build` races on same `obj_path`~~ FIXED (Batch 4b)

All intermediates now written to `tempfile::tempdir()`. Two concurrent builds on the same source file get independent temp directories — no shared paths.

## ~~Finding 4 — Medium: Project `.o` files pollute source tree on SIGKILL~~ FIXED (Batch 4b)

All project `.o` intermediates now go into `tempfile::tempdir()`. The temp dir is cleaned by the OS on process death — no orphaned `.o` files in the source tree even under SIGKILL.

## ~~Finding 5 — Low: PID-based temp runtime lib name~~ FIXED (Batch 4b)

Now uses `tempfile::NamedTempFile` — random suffix, O_CREAT|O_EXCL. Drops when linker finishes.

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
| 1 | ~~High~~ | `ynz-driver/build.rs` | `.o` stranded — **FIXED Batch 4b** |
| 2 | ~~Medium~~ | `ynz-driver/build.rs` | Partial binary after linker crash — **FIXED Batch 4b** |
| 3 | ~~Medium~~ | `ynz-driver/build.rs` | Concurrent build race on obj_path — **FIXED Batch 4b** |
| 4 | ~~Medium~~ | `ynz-driver/build.rs` | Project .o files orphan on SIGKILL — **FIXED Batch 4b** |
| 5 | ~~Low~~ | `ynz-driver/build.rs` | PID-named temp lib — **FIXED Batch 4b** |
| 7 | Low | `ynz-driver/build.rs:116-126` | Partial `.o` from failed write not cleaned |
