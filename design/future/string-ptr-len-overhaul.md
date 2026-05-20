# String Representation Overhaul — `{ptr, len}` Slices

**Status**: Locked direction, full implementation deferred. Target version TBD (probably alongside v0.5 file I/O or whenever a real use case for embedded NUL bytes lands).

User spec target: `spec/strings.md` (already partially describes the SSO layout that anticipates this overhaul).

---

## The decision

Yinz strings will eventually be represented as `{ptr, len}` slices instead of NUL-terminated C strings. The migration affects the lexer, parser, typeck, codegen, runtime, and every string method.

For v0.1, strings are C strings (NUL-terminated). The lexer rejects `\0` in string literals (Batch 5a.6) so NUL bytes never enter from source. The codegen-side defense-in-depth (this morning's batch) emits an ICE-style abort if a NUL ever sneaks in at codegen time — should be unreachable but loud-fails on regression.

---

## Why this overhaul exists

C strings have three problems Yinz will eventually need to solve:

1. **No embedded NUL bytes**. Today this is a footgun — `` `hello\0world` `` would silently truncate at runtime if the lexer didn't reject `\0` up-front. Real use cases that need embedded NULs:
   - **File I/O reading binary as a string-ish container** (v0.5 file stdlib). Binary data has NULs everywhere.
   - **FFI with C libraries that use length-prefixed buffers** (any future, even though FFI is v2+).
   - **Network protocols** (HTTP/2, gRPC, etc.) where some headers contain NULs.

2. **`length` is O(n) not O(1)**. With C strings, `.count()` requires `strlen` — walks the entire string until it hits a NUL. With `{ptr, len}`, `.count()` is a single load. Perf matters for any workload doing many `length`-related operations (parsing, validation, formatting).

3. **`strlen` in hot paths**. SIMD-accelerated `strlen` is fast but still wastes work compared to "length is in the value already." Pattern matching, substring extraction, and string concat all benefit from O(1) length access.

The flip side: C strings are simpler to interoperate with (the C ABI, FFI calls into libc, etc. all assume NUL termination). The overhaul needs a clean answer for FFI compat — probably "the C-ABI shim layer converts at the boundary."

---

## Migration scope

This is a multi-day rewrite. Layers affected:

**Parser**: relax the `\0` reject in `lex_backtick_content` (it was added in Batch 5a.6 as a defense). Once `{ptr, len}` ships, embedded NULs are fine.

**AST**: string literals carry their byte content + length, same as today. No AST change.

**Typeck**: the `Type::String` representation doesn't change at the typeck layer — it's just a primitive. No typeck change beyond removing the post-overhaul-stale NUL-reject diagnostic.

**Codegen**: the big change. Currently strings are emitted as `*const i8` pointers to NUL-terminated byte arrays. New representation: a struct `{ ptr: *const u8, len: u64 }` (or a fat pointer). LLVM IR for every string operation (literal emit, `len`, `eq`, `concat`, etc.) needs updating.

**Runtime**: every C-ABI function in `crates/ynz-runtime/src/lib.rs` that takes or returns a string. Some examples that need new signatures:
- `ynz_print(s: *const i8)` → `ynz_print(ptr: *const u8, len: u64)` OR `ynz_print(s: YnzStr)` where `YnzStr` is the slice struct.
- All the `ynz_string_*` methods (`startsWith`, `endsWith`, `contains`, etc.).
- Map keys when keyed by string.

**Stdlib (when it lands)**: file I/O, regex, JSON parsing — every byte-touching API.

**SSO (small-string optimization)**: `spec/strings.md` already describes a 23-byte inline SSO layout. That layout's design anticipates `{ptr, len}` — the inline form stores bytes-and-length-and-cap inline; the heap form points-to-ptr + length-and-cap. So SSO is already designed for this overhaul.

**FFI compat (when FFI ships in v2+)**: at the C-ABI boundary, convert `{ptr, len}` to NUL-terminated C strings on-demand. Some runtime helper like `ynz_to_cstring(s: YnzStr) -> *const i8` that allocates a NUL-terminated copy if the slice doesn't already have a NUL at `ptr + len`. Reverse direction (`ynz_from_cstring(s: *const i8) -> YnzStr`) is `strlen` + heap-copy.

---

## Triggers to implement

Any one of these flips the calculus from "lexer-side reject is fine" to "we need the overhaul":

1. **A real use case needs embedded NULs in source-level strings.** The most likely is file I/O reading binary data into a string-typed buffer.

2. **A hot path's `strlen` shows up in profiles.** Once Yinz has real workloads being profiled, `strlen` in a tight loop is the kind of perf bug that justifies the multi-day rewrite.

3. **FFI design starts (v2+).** The C-ABI conversion will already be load-bearing; doing the overhaul at the same time consolidates the design pressure.

4. **A regression hits the codegen-side NUL-byte check.** If `crates/ynz-codegen/src/emit.rs`'s "embedded NUL at codegen time" ICE ever fires in real builds, it's a sign some upstream phase started producing NULs. The clean fix is the overhaul, not a per-phase audit.

---

## What ships in v0.1 (the defense-in-depth state)

- Lexer rejects `\0` escape in string literals (Batch 5a.6). Source-level NULs can't enter strings.
- Codegen verifies no embedded NULs at emit time. Should be unreachable per the lexer reject; loud-fails as ICE if it fires (compiler bug indicator).
- Runtime's C-string handling stays as-is. `print`, `toString`, etc. assume NUL termination because that's what the codegen emits.

This state is correct + safe for v0.1's workloads. The overhaul lands when one of the triggers above fires.

---

## Open sub-questions for the implementation milestone

- **Representation**: thin pointer + separate length, OR fat pointer (`{ptr, len}` struct)? Rust uses fat pointers for `&str`; Swift uses a more elaborate tagged representation. Probably fat pointer for Yinz simplicity.
- **Mutability**: `String` (owned, mutable) vs `&str` (borrowed, immutable) distinction? Or a single `string` type that's always owned-immutable in Yinz's model? Yinz currently has one `string` type — keep it that way unless a real need surfaces.
- **Encoding**: stay UTF-8-only (per `spec/strings.md` and stdlib-design.md Rule 3). No change.
- **Concatenation semantics**: `a + b` allocates a new string today; preserve that. Document the perf cost.
- **C-ABI shim**: where does the shim live — in `ynz-runtime` (where the C calls happen) or a separate `ynz-ffi` crate?

---

## Cross-references

- `spec/strings.md` — the user-facing string spec; SSO layout already anticipates this
- `design/strings.md` — the design rationale for UTF-8 + SSO + locale-invariant case ops
- `.claude/rules/stdlib-design.md` Rule 3 (no platform-default encoding) — overhaul keeps UTF-8 default
- `.claude/rules/stdlib-design.md` Rule 8 (SIMD where available) — `{ptr, len}` enables SIMD-friendly bulk byte ops
- `crates/ynz-parser/src/lexer.rs` (Batch 5a.6 NUL reject) — relax this when the overhaul ships
- `crates/ynz-codegen/src/emit.rs` (codegen NUL defense-in-depth) — remove when overhaul ships
