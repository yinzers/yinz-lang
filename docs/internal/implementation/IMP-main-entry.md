---
name: "IMP-main-entry"
description: "Design decisions for the Yinz program entry point — why the function is always named 'entrypoint', flexible entry file naming via yinz.toml, and how CLI arguments and errors flow through it."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Main Function — Design Decisions

User spec: [`docs/reference/REF-main.md`](../../reference/REF-main.md)

---

## Function Name Always `entrypoint`, File Name Flexible

The entry function is always `entrypoint`. The entry file is configured in `yinz.toml`.

**Why `entrypoint` over `main`**: `main` is C folklore — you know it means the entry point only because you've been taught it. `entrypoint` is plain English; a developer who's never seen a compiled language immediately understands what the function is. Golden Rule 4 (self-documenting syntax) and Golden Rule 5 (human-readable over programmer jargon) both push in this direction. When two rules point the same way against a competing rule (Golden Rule 12 — familiar syntax), the lower-numbered rules win.

**Why the file name is flexible**: Developers organize projects differently. A web server might be `server.ynz`. A CLI tool might be `app.ynz`. Forcing `entrypoint.ynz` is arbitrary. The `entry` field in `yinz.toml` gives flexibility without giving up the fixed function name convention. The canonical default in docs and examples is `entrypoint.ynz` — self-documenting at a glance.

---

## CLI Arguments from stdlib, Not Parameters

`entrypoint()` takes no parameters. Arguments come from `cli.args()`, `cli.flag()`, `cli.option()`. **Designed, not built:** the `cli` and `process` modules are planned for v0.7 and have no registry entry today, so a program using them does not build.

```ynz
function entrypoint() -> Nothing {
  let args = cli.args()                    // all arguments → Array<String>
  let verbose = cli.flag(`verbose`)        // --verbose flag → Boolean
  let port = cli.option(`port`, `3000`)    // --port option → String with default
}
```

**Why**: C's `main(int argc, char* argv[])` is famously ugly. Different languages have invented different conventions. Using the standard library keeps the signature clean (`function entrypoint() -> Nothing`), puts argument parsing where it belongs (the CLI module), and is consistent with how everything else in the language works — dot methods, autocomplete-discoverable.

---

## Errors from `entrypoint` Go to the Default Handler

`function entrypoint() -> Nothing errors` lets all errors propagate to the default error handler.

**Why**: The alternative is wrapping all top-level code in explicit error handling, which is boilerplate for the 90% case where the default handler (print trace, exit 1) is exactly right. Developers who need different behavior use `setErrorHandler()` before any errors can occur.

---

## Exit Codes via `process.exit()`

`process.exit(1)` for explicit exit with a code (designed, not built; see above). Normal completion automatically exits with code 0.

```ynz
function entrypoint() -> Nothing {
  if (setupFailed) {
    process.exit(1)    // exit with failure code
  }
}
```

**Why `process.exit()` over a return value from `entrypoint()`**: Returning an exit code from `entrypoint()` would mean `-> Int` as the return type, conflicting with `-> Nothing` and `-> Nothing errors`. A stdlib function keeps the signature clean. `process.exit()` is also callable from anywhere — not just at the end of `entrypoint()`.
