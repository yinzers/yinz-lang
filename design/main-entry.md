# Main Function — Design Decisions

User spec: `spec/main.md`

---

## Function Name Always `main`, File Name Flexible

The entry function is always `main`. The entry file is configured in `yinz.toml`.

**Why `main` is fixed**: Every compiled language uses `main`. It's universal and requires zero learning. No configuration for the function name — one convention, always.

**Why the file name is flexible**: Developers organize projects differently. A web server might be `server.ynz`. A CLI tool might be `app.ynz`. Forcing `main.ynz` is arbitrary. The `entry` field in `yinz.toml` gives flexibility without giving up the fixed function name convention.

### Proposed convention: `entrypoint.ynz` as the canonical default (proposed 2026-05-18 — up for discussion)

Rather than defaulting to `main.ynz` in docs and examples, use `entrypoint.ynz`. The function inside is still `function main()` — that stays universal. Only the file name changes.

**Rationale**: `entrypoint.ynz` is self-documenting per Golden Rule 2 — a developer unfamiliar with C conventions immediately knows what the file is. `main.ynz` carries meaning only if you already know `main` is the C entry convention. Golden Rule 12 (human-readable over programmer jargon) pushes in the same direction.

**What changes if adopted**: `spec/main.md` default entry value, all docs and examples that show `main.ynz`, and the canonical basics demo (`examples/basics/src/main.ynz` → `examples/basics/src/entrypoint.ynz`). Compiler behavior is unchanged — entry file is always configurable. Pure convention/docs change.

**What does NOT change**: the function name stays `function main()`. Familiar to every developer; no jargon replaced there.

---

## CLI Arguments from stdlib, Not Parameters

`main()` takes no parameters. Arguments come from `cli.args()`, `cli.flag()`, `cli.option()`.

**Why**: C's `main(int argc, char* argv[])` is famously ugly. Different languages have invented different conventions. Using the standard library keeps the signature clean (`function main() -> nothing`), puts argument parsing where it belongs (the CLI module), and is consistent with how everything else in the language works — dot methods, autocomplete-discoverable.

---

## Errors from `main` Go to the Default Handler

`function main() -> nothing errors` lets all errors propagate to the default error handler.

**Why**: The alternative is wrapping all top-level code in explicit error handling, which is boilerplate for the 90% case where the default handler (print trace, exit 1) is exactly right. Developers who need different behavior use `setErrorHandler()` before any errors can occur.

---

## Exit Codes via `process.exit()`

`process.exit(1)` for explicit exit with a code. Normal completion automatically exits with code 0.

**Why `process.exit()` over a return value from `main()`**: Returning an exit code from `main()` would mean `-> int` as the return type, conflicting with `-> nothing` and `-> nothing errors`. A stdlib function keeps the signature clean. `process.exit()` is also callable from anywhere — not just at the end of `main()`.
