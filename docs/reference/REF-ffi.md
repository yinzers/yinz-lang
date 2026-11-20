---
name: "REF-ffi"
description: "Call functions from C libraries, system APIs, and other non-Yinz code."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Foreign Function Interface (FFI)

Call functions from C libraries, system APIs, and other non-Yinz code. *FFI* stands for foreign function interface: a *foreign* function is one written in another language.

> **Not available yet.** `foreign` is planned for a later release. The compiler does not recognize it today (you can still use `foreign` as a normal name). The rest of this page describes the planned design. Until then, use the standard modules for file access, networking and math. They already call C code for you, so you never see the foreign boundary yourself.

---

## Declaring foreign functions

```
foreign function sqrt(value: float) -> float from `libm`
foreign function sdlInit(flags: int) -> int from `SDL2`
foreign function curlEasyPerform(handle: int) -> int from `libcurl`
```

---

## The rule: always wrap in safe Yinz functions

Keep the foreign boundary as small as possible. Declare the foreign function, then wrap it right away in a safe Yinz function that the rest of your code uses:

```
// The foreign declaration — kept in one place
foreign function readFd(fd: int, buffer: int, size: int) -> int from `libc`

// The safe wrapper — this is what everyone else calls
export function readBinaryFile(path: string) -> array<byte> errors {
  // all the unchecked foreign calls stay in here
  // callers see normal Yinz types and the errors system
}
```

The foreign call never escapes the wrapper. That keeps unchecked code in one place, so the compiler can still check the rest of your program.

---

## What the compiler cannot do with foreign code

The compiler cannot see what a foreign function does. It cannot check it for ownership mistakes or for problems when two jobs run at the same time. So:

- **Every foreign call needs `wait`.** The compiler can't tell whether it is safe to run the call alongside other work.
- **Foreign calls never run at the same time as other lines on their own.**
- **The compiler warns on every foreign call:** `Foreign call: compiler cannot verify memory safety. Consider wrapping in a safe Yinz function.`

---

## Mapping C types to Yinz types

How C types such as `void*`, `char*` and raw pointers map to Yinz types is not decided yet, so this page does not cover it.
