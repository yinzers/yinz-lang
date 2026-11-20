---
name: "REF-ffi"
description: "Yinz cannot call C libraries or other non-Yinz code yet."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Foreign Function Interface (FFI)

A *foreign function interface* (FFI) lets a program call functions written in another language, such as C. Yinz has none: the compiler does not recognize a `foreign` keyword, so you cannot declare or call a function from a C library.

Write the code you need in Yinz instead. See [Functions](REF-functions.md).
