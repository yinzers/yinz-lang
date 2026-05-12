# Changelog

## v0.1.0-m1 — Hello-world end-to-end (M1 milestone)

**Release tag:** `v0.1.0-m1`

### What ships

M1 is the walking skeleton: `ynz run hello.ynz` compiles and runs a Yinz
program for the first time. The full pipeline — lex → parse → type check →
LLVM codegen → link → execute — is wired together through salsa queries.

### Language surface (M1)

The M1 language surface is intentionally minimal:

- `function main() -> nothing { ... }` — entry point only
- `print("string literal")` — builtin that writes text + newline to stdout
- String literals (raw UTF-8 bytes, no escape processing)
- The `nothing` return type

Everything else (variables, arithmetic, types, ownership, generics, etc.)
lands in M2–M8.

### Compiler features

- **Diagnostics** (`ynz-diagnostics`): mandatory three-part WHAT/WHAT-INSTEAD/WHY
  format, 50-error cap, ariadne-rendered output, automated banned-jargon audit.
- **Lexer** (`ynz-parser`): salsa-tracked `lex_query`, UTF-8 source, M1 token set,
  error recovery on unknown characters and unterminated strings.
- **Parser** (`ynz-parser`): hand-written recursive-descent `parse_query`, error
  recovery with `Expr::Error` / `Type::Error` placeholder nodes.
- **Type checker** (`ynz-typeck`): salsa-tracked `check_query`, verifies `main`
  signature, resolves `print` builtin, type-checks arguments, parse-error gate
  prevents cascade noise.
- **Codegen** (`ynz-codegen`): salsa-tracked `codegen_query`, emits LLVM IR via
  inkwell (LLVM 18), links against libc `puts`. SHA-256 golden hash for
  reproducibility testing.
- **Driver** (`ynz-driver`): `ynz build <file>` and `ynz run <file>` subcommands.

### Tests

51 tests across 6 crates, all passing. Includes unit tests, snapshot tests
(insta), integration tests that run the actual `ynz` binary, and a SHA-256
golden hash test for codegen reproducibility.
