# Foreign Function Interface (FFI)

Call functions from C libraries, system APIs, and other non-Yinz code.

---

## Declaring foreign functions

```
foreign function c_sqrt(value: float) -> float from "libm"
foreign function SDL_Init(flags: int) -> int from "SDL2"
foreign function curl_easy_perform(handle: int) -> int from "libcurl"
```

---

## The rule: always wrap in safe Yinz functions

Keep the foreign boundary as small as possible. Declare the foreign function, then immediately wrap it in a safe Yinz function that the rest of your code uses:

```
// The foreign declaration — isolated and contained
foreign function c_read(fd: int, buffer: int, size: int) -> int from "libc"

// The safe wrapper — this is what everyone else calls
export function readBinaryFile(path: string) -> array[byte] errors {
  // all the unsafe stuff is contained here
  // callers see safe Yinz types and the errors system
}
```

The foreign call never leaks outside the wrapper. This keeps unsafe code isolated and makes the rest of the codebase analyzable by the compiler.

---

## What the compiler cannot do with foreign code

The compiler knows nothing about what a foreign function actually does. It cannot analyze foreign code for ownership safety or concurrency safety. As a result:

- **All foreign calls require `wait`** — the compiler cannot determine if they're safe to run concurrently
- **Foreign functions are excluded from auto-parallelization**
- **The compiler warns on every foreign call** — "Foreign call: compiler cannot verify memory safety. Consider wrapping in a safe Yinz function."

---

## Why `foreign` over `unsafe`

`foreign` describes what it IS — code from another language. `unsafe` only says it's dangerous without saying why. A junior developer reading `foreign function c_sqrt(...)` immediately understands: this comes from outside Yinz, the compiler can't check it.

---

## Type mapping — note

How C types like `void*`, `char*`, and raw pointers map to Yinz types is an open design question. See `design/open-questions.md`.
