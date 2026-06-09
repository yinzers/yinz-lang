# `examples/primantis-orders/` — Per-Milestone Compile-Error Gallery

**Layout: gallery** (loose `.ynz` files, no `yinz.toml`). Not a Yinz project — these files are standalone scripts the compiler ingests one at a time to show error diagnostics.

**Theme:** Primanti's kitchen — orders going sideways during the rush. Each milestone's error classes triggered with restaurant-flavored code (sandwiches, orders, tabs, toppings). Deeper theming on the M1/M2/M4 files; the remaining files have a themed header + light variable renames with the error-trigger patterns kept identical to the originals.

One file per milestone showing every compile error that milestone's diagnostics can produce. Yinz multi-errors (up to 50 per compile per `design/compiler-errors.md`), so each file demonstrates many simultaneous diagnostics in one compile run.

## How to run

```bash
source $HOME/.cargo/env
./target/debug/ynz build examples/primantis-orders/m1_errors.ynz   # prints all M1 errors
./target/debug/ynz build examples/primantis-orders/m2_errors.ynz   # M1 + M2 errors
./target/debug/ynz build examples/primantis-orders/m3_errors.ynz   # M1 + M2 + M3 errors
# ... and so on per milestone
```

These files are INTENTIONALLY broken — they're not meant to compile. The point is to see the teaching diagnostics in action.

## What each milestone covers

| File | Status | Error classes demonstrated |
|---|---|---|
| **m1_errors.ynz** | ✅ shipped | Unterminated string, unknown character, banned `fn` keyword, missing function body, type mismatch (string vs nothing) |
| **m2_errors.ynz** | ✅ shipped | const reassignment, mixed-type arithmetic, integer overflow at parse time, banned compound assignment (`+=`), bignum deferral (`number<N>` for N > 34), division by zero (runtime) |
| **m3_errors.ynz** | ✅ shipped | Missing return path, dead code after return, banned `match`/`switch` keywords, loop variable mutation, argument arity mismatch, argument type mismatch, duplicate function name, `is` type-pattern deferral |
| **m4_errors.ynz** | in-progress (M4 phases) | Methods inside shape body, `override` keyword used, struct-literal prefix form (`Player { ... }`), `const` passed to `lend` parameter, use-after-give, double-lend, cyclic extends, missing `follows` method, base-shape instantiation, banned `type`/`struct`/`class`/`interface`/`enum`/`abstract` keywords |
| **m5_errors.ynz** | future | generics + collections error classes |
| **m6_errors.ynz** | future | options + union narrowing error classes |
| **m7_errors.ynz** | future | string + errors + iterables error classes |
| **m8_errors.ynz** | future | module + sensitive + concurrency error classes |
| **v0_2_m1_errors.ynz** | ✅ shipped | feature-registry SSOT diagnostics |
| **v0_2_m2_errors.ynz** | ✅ shipped | LSP diagnostic flow verification |
| **v0_2_m3_errors.ynz** | ✅ shipped | watch-daemon diagnostics |
| **v0_2_m4_errors.ynz** | ✅ shipped | file-watcher path errors |
| **v0_2_m5_errors.ynz** | ✅ shipped | LSP rename + refactor errors |
| **[v0_3_m1_errors.ynz](v0_3_m1_errors.ynz)** | ✅ shipped | share-param (carry-forward), lend-cross-thread, large-copy warning, kernel-mode rejections (commented, triggered in-process) |
| **[v0_3_m3e_errors.ynz](v0_3_m3e_errors.ynz)** | ✅ shipped | DynamicDispatch reject (vtable call in suspending context); FFI cross-module suspension (not yet reachable — documented for when FFI ships). Universal cross-module reject removed — all analyzable cross-module suspending calls now compile and run. |

## Why this exists

`.claude/rules/plan-invariants.md` `### Demo & Error Gallery` subsection: every phase that adds new compile-error classes MUST extend the corresponding milestone gallery file with intentional triggers. Each trigger has a `// WHY:` comment naming the diagnostic class. This gives Patrick a visible reference for the language's teaching diagnostics — the human-eyes-on layer that automated snapshot tests can't replace.

## Companion: `examples/pirates-roster/`

The `examples/pirates-roster/entrypoint.ynz` file is the parallel success-path showcase — every feature working in context. Together, `pirates-roster/` + `primantis-orders/` give end-to-end coverage of what Yinz produces in both happy-path and error-path cases.
