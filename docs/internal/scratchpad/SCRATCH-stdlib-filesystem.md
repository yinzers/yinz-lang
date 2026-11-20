---
name: "SCRATCH-stdlib-filesystem"
description: "Early scratchpad ideas for a future stdlib file-system module covering file read/write/append and directory operations, all using the errors system."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Standard Library — File System

---

## File Operations

All file operations use the `errors` system.

```
let content = file.read(`data.txt`)                      // -> String errors
let lines = file.readLines(`data.txt`)                   // -> Array<String> errors (loads all lines)
let lazyLines = file.lines(`data.txt`)                   // -> Iterable<String> errors (lazy — one line at a time)
let bytes = file.readBytes(`image.png`)                  // -> Array<Byte> errors

file.write(`output.txt`, content)                        // -> Nothing errors
file.appendLine(`log.txt`, `new entry`)                  // -> Nothing errors

if (file.exists(`config.ynz`)) { ... }
let size = file.size(`data.txt`)                         // -> Int errors
let modified = file.lastModified(`data.txt`)             // -> Date errors
```

---

## Directory Operations

```
let files = directory.list(`/path/to/dir`)               // -> Array<String> errors
directory.create(`/path/to/new`)                         // -> Nothing errors
directory.delete(`/path/to/old`)                         // -> Nothing errors
```

---

## Path Utilities

```
let full = path.join(`src`, `utils`, `helpers.ynz`)
let ext = path.extension(`photo.jpg`)                    // `jpg`
let name = path.filename(`/app/src/entrypoint.ynz`)            // `entrypoint.ynz`
let dir = path.directory(`/app/src/entrypoint.ynz`)            // `/app/src`
```

---

## Expansion Candidates

- File watching (notify on changes)
- Temp file/directory creation with auto-cleanup
- File locking
- Streaming reads/writes for large files
- Glob pattern matching
- File permissions management
- Symlink support
- Archive support (zip, tar, gzip)
- File copy/move helpers

---

## v0.5+ Async I/O Surface

The canonical deferral spec for async file operations lives in the feature registry:

```
registry/features.toml → [[deferred_tooling_feature]] name = "async-io-stdlib-intrinsics-v0-5"
```

That registry entry is the SSOT — this section is a cross-reference, not the authority.

**Planned async surface (ships with v0.5 file module)**:

- `readFileAsync(path) -> String errors` — non-blocking file read; ``wait readFileAsync(`data.txt`)`` suspends the caller while the OS reads the file; thread is freed during the I/O.
- `writeFileAsync(path, content) -> Nothing errors` — non-blocking file write.
- `readBytesAsync(path) -> Array<Byte> errors` — non-blocking binary read.

These back on to `tokio::fs` in the runtime. The state-machine ABI that makes `wait` work was validated in v0.3-M2 using the internal `__testFallibleAsync` intrinsic — v0.5 inherits a working errors-through-state-machine ABI.

**Why deferred**: file path encoding, error kind design (file-not-found vs permission-denied vs I/O error), and the `errors` propagation shape all belong to the v0.5 file module milestone, not to the state-machine milestone.

### ⚠️ Performance — I/O backend (DO NOT FORGET at v0.5)

When the v0.5 async file module lands, the runtime's file-I/O path MUST use the fastest available kernel syscall layer per target, NOT the default blocking-thread-pool fallback:

- **Linux: `io_uring`.** `tokio::fs` defaults to a blocking thread pool (each file op ties up a pool thread for the syscall) — that's the *correct-but-slower* default, the file-I/O analogue of the v0.3-M2 sync-bridge thread-hold. `io_uring` submits I/O to the kernel via a shared ring buffer with zero per-op thread handoff and batched submission/completion — materially fewer syscalls and no thread-per-inflight-op. The runtime should detect kernel support (`io_uring` ≥ 5.1, mature ≥ 5.11) and use it as the default backend for async file (and network) I/O, falling back to the thread pool only on older kernels / unsupported targets. Reference: `tokio-uring`, or a direct `io_uring` binding in `libynz_rt`.
- **macOS / BSD: `kqueue`; Windows: IOCP** — the platform-native completion mechanisms, same principle (no thread-per-op).
- **SIMD UTF-8 validation on read** — already mandated by the SIMD bullet of [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) (validate decoded bytes via `simdutf`/equivalent, ~0.7 cycles/byte vs ~8 scalar). Cross-referenced here so the v0.5 read path wires it in.

This is the I/O-layer counterpart to v0.3's "fastest mechanism for known code" decision: the *task model* (stackless state machines) is already optimal; `io_uring` makes the *syscall layer* underneath it optimal too. Capturing here so the v0.5 milestone plan's `### Performance` invariant subsection picks it up rather than shipping the slower thread-pool default and discovering it later. WHY recorded now: surfaced during the v0.3-M2 re-spike "is there a faster way" review (2026-05-31) as the I/O-backend asterisk to the concurrency model — belongs in the file module, not v0.3.
