---
name: "IMP-ffi"
description: "Design rationale for Yinz's foreign function interface: the 'foreign' keyword over 'unsafe', the always-wrap-in-safe-Yinz-functions pattern, and required 'wait' on foreign calls."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Foreign Function Interface — Design Decisions

No user-facing reference page: `foreign` is deferred to v2+ and not implemented.

> **Status: deferred to v2+, not implemented.** `foreign` is a registered
> `[[deferred_language_feature]]` (`registry/features.toml`, design doc
> [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md) v2+ section) — it is not yet
> a reserved keyword, and none of the mechanisms below exist in the compiler today. Stdlib modules
> that need C interop call C internally via compiler-private mechanisms users never see. The
> decisions below are the locked DESIGN for when v2+ builds this out, not a description of current
> behavior.

---

## The `foreign` Keyword

`foreign function name(...) from `library`` — the keyword says where the code comes from (another language), which tells a junior developer reading `foreign function cSqrt(...)` why the call needs care. Golden Rule 5.

Declarations (designed, not built):

```ynz
foreign function sqrt(value: Float) -> Float from `libm`
foreign function sdlInit(flags: Int) -> Int from `SDL2`
foreign function curlEasyPerform(handle: Int) -> Int from `libcurl`
```

---

## Always Wrap in Safe Yinz Functions

The foreign declaration is never used directly by the rest of the codebase. It's wrapped immediately in a safe Yinz function with proper types, ownership annotations, and error handling.

```ynz
// The foreign declaration — kept in one place
foreign function readFd(fd: Int, buffer: Int, size: Int) -> Int from `libc`

// The safe function — this is what everyone else calls
export function readBinaryFile(path: String) -> Array<Int> errors {
  // all the unchecked foreign calls stay in here;
  // callers see normal Yinz types and the errors system
}
```

The foreign call never escapes the safe function, so the compiler can still check the rest of the program. (`Array<Int>` stands in for a byte array: Yinz has no `byte` type.)

**Why**: The foreign boundary is where safety guarantees break down. Keeping it small and contained limits the blast radius. Everything outside the wrapper is fully analyzable by the compiler. This is the same principle as keeping I/O at the edges of a system.

---

## Compiler Requires `wait` on All Foreign Calls

The compiler cannot analyze foreign code. It doesn't know if a foreign function has side effects, blocks, or modifies shared state. Requiring `wait` prevents the auto-parallelization system from accidentally racing foreign calls.

**The exclusion is explicit**: the developer sees `wait cRead(...)` and knows this is a sequential call. The compiler never runs a foreign call at the same time as other code on its own.

**Warning on every foreign call** (designed text): `Foreign call: compiler cannot verify memory safety. Consider calling it only through a safe Yinz function.` The compiler cannot see ownership mistakes or task-clash problems inside foreign code, which is why the warning and the required `wait` exist.

---

## Open Questions

- **C type mapping**: How do C types like `void*`, `char*`, `struct*`, and function pointers map to Yinz types? Is there a raw `pointer` type? See [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../scratchpad/SCRATCH-open-questions.md).
- **Error handling**: How do C-style error codes (return -1 on failure) integrate with the `errors` system? The wrapper function presumably converts, but the interface needs design.
