# Main Function — Design Decisions

User spec: `spec/main.md`

---

## Function Name Always `entrypoint`, File Name Flexible

The entry function is always `entrypoint`. The entry file is configured in `yinz.toml`.

**Why `entrypoint` over `main`**: `main` is C folklore — you know it means the entry point only because you've been taught it. `entrypoint` is plain English; a developer who's never seen a compiled language immediately understands what the function is. Golden Rule 2 (self-documenting syntax) and Golden Rule 12 (human-readable over programmer jargon) both push in this direction. When two rules point the same way against a competing rule (Golden Rule 6 — familiar syntax), the lower-numbered rules win. Decided 2026-05-18.

**Why the file name is flexible**: Developers organize projects differently. A web server might be `server.ynz`. A CLI tool might be `app.ynz`. Forcing `entrypoint.ynz` is arbitrary. The `entry` field in `yinz.toml` gives flexibility without giving up the fixed function name convention. The canonical default in docs and examples is `entrypoint.ynz` — self-documenting at a glance.

---

## CLI Arguments from stdlib, Not Parameters

`entrypoint()` takes no parameters. Arguments come from `cli.args()`, `cli.flag()`, `cli.option()`.

**Why**: C's `main(int argc, char* argv[])` is famously ugly. Different languages have invented different conventions. Using the standard library keeps the signature clean (`function entrypoint() -> nothing`), puts argument parsing where it belongs (the CLI module), and is consistent with how everything else in the language works — dot methods, autocomplete-discoverable.

---

## Errors from `entrypoint` Go to the Default Handler

`function entrypoint() -> nothing errors` lets all errors propagate to the default error handler.

**Why**: The alternative is wrapping all top-level code in explicit error handling, which is boilerplate for the 90% case where the default handler (print trace, exit 1) is exactly right. Developers who need different behavior use `setErrorHandler()` before any errors can occur.

---

## Exit Codes via `process.exit()`

`process.exit(1)` for explicit exit with a code. Normal completion automatically exits with code 0.

**Why `process.exit()` over a return value from `entrypoint()`**: Returning an exit code from `entrypoint()` would mean `-> int` as the return type, conflicting with `-> nothing` and `-> nothing errors`. A stdlib function keeps the signature clean. `process.exit()` is also callable from anywhere — not just at the end of `entrypoint()`.
