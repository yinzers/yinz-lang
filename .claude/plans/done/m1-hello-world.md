---
slug: m1-hello-world
owner: patrick
status: done
files:
  - Cargo.toml
  - crates/**
  - tests/**
  - rust-toolchain.toml
  - .github/workflows/**
created: 2026-05-12
last_updated: 2026-05-12
completed: 2026-05-12
tag: v0.1.0-m1
parent: v0-1-compiler
---

# Plan: M1 — Hello-world end-to-end (ARCHIVED)

**Parent milestone**: see `.claude/plans/active/v0-1-compiler.md` for the v0.1 roadmap.

End-to-end walking skeleton. `function entrypoint() -> nothing { print("hello, yinz") }` compiles and runs. Proves the full pipeline (lex → parse → typeck → codegen → link → execute) works, with salsa wiring in place from the start.

**Outcome**: COMPLETE (2026-05-12) — 51 tests green, `ynz run hello.ynz` outputs `hello, yinz`. Shipped at commit `820bfdc`. Tagged `v0.1.0-m1` after merge.

---

## M1 Completion Summary (2026-05-12)

**All 8 phases shipped in one session. 51 tests green. `ynz run hello.ynz` → `hello, yinz`.**

### What was built

**Toolchain decisions locked:**
- Rust 1.95.0 stable, LLVM 18.1.8, inkwell 0.9.0 (`llvm18-1-prefer-dynamic` — Ubuntu packages don't ship static Polly), salsa 0.26.2, ariadne 0.6.0, clap 4.6.1, insta 1.47.2
- `LLVM_SYS_181_PREFIX=/usr/lib/llvm-18` in `.cargo/config.toml` for Linux; macOS needs `brew --prefix llvm@18`
- `llvm18-1-prefer-dynamic` feature (not `llvm18-1`) — critical for Ubuntu where libLLVMPolly.a is not shipped

**Crate layout:**
- `ynz-diagnostics` — Diagnostic (three-part), DiagnosticBucket (50-cap), ariadne render, BANNED_JARGON constant, jargon audit test
- `ynz-ast` — Module, FunctionDecl, Block, Stmt, Expr (Ident/StringLit/Call/Error), Type (Nothing/Named/Error)
- `ynz-parser` — CompilerDb + SourceFile (salsa input), lex_query + parse_query (salsa tracked), hand-written lexer + parser
- `ynz-typeck` — check_query (salsa tracked), BuiltinTable (print builtin), parse-error gate, TypedModule
- `ynz-codegen` — codegen_query (salsa tracked), emit_artifact (inkwell, x86 only for M1), SHA-256 golden hash, IR text snapshot
- `ynz-driver` — `ynz build` + `ynz run` CLI, load/build/run modules, integration tests using CARGO_BIN_EXE_ynz

**Salsa 0.26 API patterns learned:**
- Return types of `#[salsa::tracked]` functions need `PartialEq` (salsa uses `UpdateFallback` via `PartialEq`)
- Use `Arc<T>` as return type to avoid requiring `T: Update` directly — `Arc<T>` where `T: PartialEq` works
- `#[return_ref]` attribute does NOT exist in salsa 0.26 (older API) — just omit it, fields return by clone
- `Setter` trait must be imported (`use salsa::Setter as _`) to call `.set_field(&mut db).to(value)` in tests
- `salsa::DatabaseImpl::new()` is the concrete test database

**inkwell 0.26 patterns learned:**
- `build_gep` takes the element type as first arg (opaque pointers in LLVM 18)
- `inkwell::Context` and `inkwell::Module<'ctx>` must share the same `'ctx` lifetime via explicit generic
- `build_call`, `build_return`, `build_gep` all return `Result<_, BuilderError>` — must `.map_err()`
- `module.print_to_string()` returns `inkwell::support::LLVMString`, call `.to_string()` on it
- For Ubuntu: `prefer-dynamic` avoids needing all static `.a` files (Polly etc.)

**ariadne 0.6 API changes from older versions:**
- `Report::build(kind, span: S)` — only 2 args, span IS the primary error location (old API had 3 args: kind, file_id, offset)
- `Label::new(span)` takes owned span (not ref), `SourceSpan` must implement `ariadne::Span`
- `Cache<str>` impl: use `impl fmt::Debug` and `impl fmt::Display` return types (not boxed traits) to avoid refining_impl_trait warning
- `Source::from(v.clone())` — need `String` not `&str` to get `Source<String>` for `HashMap<String, Source>`
- `Config::with_color(bool)` exists; `ReportKind::Custom` still embeds color that bypasses config — use `Color::Primary` for colorless custom kinds

### Project structure question answered (this session)
Yinz project structure is root-relative, flat discovery. `yinz.toml` defines the root. No `mod` declarations, no explicit file graph. Single-segment imports = stdlib, multi-segment = project files. Already fully specced in `spec/modules.md` and `design/modules.md`.

---

## What M1 explicitly was NOT (deferred to later milestones)

- No variables, no `let` / `const`
- No arithmetic, no operators (other than `()`, `{}`, `->`)
- No user-defined functions other than `main`
- No types other than `nothing` and string-literal
- No generics, no collections, no `maybe`, no `options`, no unions
- No ownership analysis (no other-typed values to own — strings used in M1 are static globals in LLVM, no allocation needed)
- No interpolation, no Unicode tooling (M1 strings are ASCII byte arrays for codegen purposes; `unicode-segmentation` is added in M7)
- No `errors`, no cascades
- No imports, no modules — single-file compilation only
- No decimal128, no float, no int — no numerics at all
- No incremental rebuild yet (salsa queries are wired but incremental invalidation isn't exercised in M1; that becomes interesting in M2+)
- No `ynz watch`, no `ynz fmt`, no LSP (v0.2)
- No tests for the user's code (`test` keyword is reserved in the parser starting M3 or whenever the parser surface grows to include it; M1 doesn't define it at all)

---

## Phase 1: Repo scaffolding (cargo workspace + CI)
**PR scope**: Establish the Rust workspace, pin tool versions, set up CI. No compiler logic.
**Branch**: `feat/repo-scaffolding`
**Est. lines**: ~150 (mostly TOML + CI YAML)
**Objective**: Empty workspace builds clean; `ynz --version` returns a string; CI passes on a no-op PR.
**Why this phase exists**: Lock in workspace structure and version pins BEFORE any compiler code lands. Avoids the "we'll restructure later" trap.
**Files (expected scope)**:
- `Cargo.toml` (workspace root)
- `rust-toolchain.toml` (pin stable channel + LLVM-compatible toolchain)
- `crates/ynz-driver/Cargo.toml` + `crates/ynz-driver/src/main.rs` (entry point, `ynz --version`)
- `crates/ynz-diagnostics/Cargo.toml` + `crates/ynz-diagnostics/src/lib.rs` (empty)
- `crates/ynz-ast/Cargo.toml` + `crates/ynz-ast/src/lib.rs` (empty)
- `crates/ynz-parser/Cargo.toml` + `crates/ynz-parser/src/lib.rs` (empty)
- `crates/ynz-typeck/Cargo.toml` + `crates/ynz-typeck/src/lib.rs` (empty)
- `crates/ynz-codegen/Cargo.toml` + `crates/ynz-codegen/src/lib.rs` (empty)
- `.github/workflows/ci.yml` (build, test, clippy, fmt)
- `.gitignore` (`target/`, `*.ll`, `*.o`)
- `README.md` (brief — link to `spec/overview.md`)
**Steps**:
1. Initialise `Cargo.toml` workspace with the seven crates above.
2. Add direct deps: `salsa = "<latest stable>"`, `inkwell = { version = "<llvm-18-compatible>", features = ["llvm18-0"] }`, `ariadne = "<latest>"`, `unicode-segmentation = "<latest>"` (pin exact versions; lockfile committed).
3. `rust-toolchain.toml` pins `channel = "stable"` and lists components `clippy`, `rustfmt`.
4. `ynz-driver` `main.rs`: parse `--version` and print a literal version string. Exit 0. No other behaviour.
5. CI workflow runs `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `cargo build --workspace --release` against Ubuntu and macOS runners with LLVM 18 installed.
6. README contains a one-paragraph project description + LLVM-18 install instructions for Linux + macOS.
**Acceptance criteria**:
- [x] `cargo build --workspace` succeeds on a clean clone with LLVM 18 installed.
- [x] `cargo run -p ynz-driver -- --version` prints a non-empty version string and exits 0.
- [x] `cargo clippy --workspace -- -D warnings` passes.
- [x] `cargo fmt --check` passes.
- [x] CI is green on a PR that touches only this scaffolding.
- [x] `Cargo.lock` is committed.

---

## Phase 2: Diagnostics infrastructure (before any parsing)
**PR scope**: `ynz-diagnostics` crate ships the three-part WHAT/WHAT-INSTEAD/WHY diagnostic type, `ariadne`-rendered output, multi-error accumulator with 50-error cap, severity tiers (Error / Warning / Suggestion — Suggestion is reserved for v0.4 but the tier exists from day 1), automated banned-jargon grep test over the workspace.
**Branch**: `feat/diagnostics`
**Est. lines**: ~500
**Objective**: A consumer crate can build a `Diagnostic` with a span and three-part message, push it to a `DiagnosticBucket`, and render the bucket as a string identical to a committed snapshot. The bucket enforces a 50-error cap with a standard "... and N more errors hidden" footer. An automated workspace-wide grep test fails CI if any banned-jargon word appears in diagnostic-construction call sites.
**Why this phase exists**: Golden Rule 11 (compiler is a teacher) is load-bearing. Shipping the parser before diagnostics infrastructure invites "we'll polish the errors later" which is exactly the duct-tape framing `~/.claude/rules/no-duct-tape.md` prohibits. Build the teaching layer first, then make the rest of the compiler use it. Per `design/compiler-errors.md`, the 50-error cap is a spec requirement — enforcing it from P2 means every later phase inherits the behaviour for free.
**Files (expected scope)**:
- `crates/ynz-diagnostics/src/lib.rs`
- `crates/ynz-diagnostics/src/diagnostic.rs` (the `Diagnostic` struct: severity, span, three fields `what` / `what_instead` / `why`)
- `crates/ynz-diagnostics/src/bucket.rs` (multi-error accumulator with 50-error cap)
- `crates/ynz-diagnostics/src/render.rs` (`ariadne` integration, "and N more hidden" footer)
- `crates/ynz-diagnostics/src/span.rs` (`SourceSpan` — file id + byte range)
- `crates/ynz-diagnostics/src/banned_jargon.rs` (const array of banned words extracted from `design/compiler-errors.md`)
- `crates/ynz-diagnostics/tests/snapshots.rs` + `crates/ynz-diagnostics/tests/__snapshots__/` (uses `insta` for snapshot testing)
- `crates/ynz-diagnostics/tests/jargon_audit.rs` (workspace-wide grep test — fails if any banned word appears in a diagnostic-construction context)
**Acceptance criteria**:
- [x] `Diagnostic` constructor panics with a clear message if any of `what` / `what_instead` / `why` is empty.
- [x] `ariadne` renders the diagnostic with the source line, a caret pointing to the span, and the three fields visible in the output.
- [x] `DiagnosticBucket` caps at 50 errors; the 51st error increments `hidden_count` and the rendered output ends with `... and N more errors hidden`.
- [x] Snapshot tests exist for the five cases listed above and match committed golden files.
- [x] `tests/jargon_audit.rs` passes on the empty workspace and is wired to run on every `cargo test --workspace`.
- [x] `BANNED_JARGON` constant stays in sync with `design/compiler-errors.md` via a snapshot-style sync test.
- [x] Public API exposes `Diagnostic`, `DiagnosticBucket`, `Severity`, `SourceSpan`, `render` — and nothing else.

---

## Phase 3: Lexer (M1 surface only) as salsa query
**PR scope**: `ynz-parser::lex` salsa query takes source text and returns a `TokenStream`. Handles only what M1 needs: identifiers, the keywords `function` and `nothing`, string literals, `(`, `)`, `{`, `}`, `->`, and whitespace/newlines (skipped). Multi-error: unknown characters and unterminated strings produce diagnostics, lexer continues to next sensible boundary.
**Branch**: `feat/lexer`
**Est. lines**: ~400
**Objective**: Lexing the M1 source `function entrypoint() -> nothing { print("hello, yinz") }` produces the expected token stream (asserted against a snapshot) and produces zero diagnostics.

**Spec decision locked in this phase:** string-literal contents in M1 are UTF-8 byte sequences passed through unchanged from source to codegen. The lexer does NOT decode them into a `String` — they're stored as `Vec<u8>` on the `StringLit` token. ASCII source is the common path; non-ASCII bytes (e.g., `"café"`) are accepted and round-trip as raw bytes to `puts`. No `\n` / `\t` / `\"` escape processing in M1 (escape decoding lands with the M2 strings work). Source files MUST be valid UTF-8; non-UTF-8 source bytes produce a three-part diagnostic at file load time (`ynz-driver` responsibility, not the lexer).

**Files (expected scope)**:
- `crates/ynz-parser/src/token.rs` (`Token` enum: `Function`, `Nothing`, `Identifier(String)`, `StringLit(String)`, `LParen`, `RParen`, `LBrace`, `RBrace`, `Arrow`, `Eof`)
- `crates/ynz-parser/src/lexer.rs` (the lexing logic, hand-written)
- `crates/ynz-parser/src/queries.rs` (salsa input + first query: `lex(source_id) -> (Vec<Spanned<Token>>, DiagnosticBucket)`)
- `crates/ynz-parser/tests/lex.rs` + snapshots

**Acceptance criteria**:
- [x] Lexing the M1 source produces a token stream matching the committed snapshot.
- [x] Unknown-char input produces a diagnostic with the correct span AND a non-empty recovered token stream.
- [x] Unterminated string input produces a diagnostic at the opening quote AND a non-empty recovered token stream.
- [x] Non-ASCII bytes inside a string lex clean — bytes round-trip via the token's `Vec<u8>` payload.
- [x] Empty and whitespace-only source produce zero tokens and zero diagnostics.
- [x] All token positions (start byte, end byte) are correct.
- [x] The `lex` query is salsa-tracked.
- [x] `m1_token_variant_count_locked` test pins the variant count for M1.

---

## Phase 4: AST + parser (M1 surface) as salsa query
**PR scope**: `ynz-ast` defines the AST node types; `ynz-parser::parse` is a salsa query taking the token stream and returning a `Module` AST. Multi-error: parse errors accumulate, parser recovers to next `}` or end of file.
**Branch**: `feat/parser`
**Est. lines**: ~600
**Files (expected scope)**:
- `crates/ynz-ast/src/lib.rs` (re-exports)
- `crates/ynz-ast/src/nodes.rs` (`Module`, `FunctionDecl`, `Block`, `Stmt::ExprStmt`, `Expr::Call`, `Expr::Ident`, `Expr::StringLit`, `Type::Nothing`, `Type::Named`)
- `crates/ynz-parser/src/parser.rs` (recursive-descent parser)
- `crates/ynz-parser/src/queries.rs` (add `parse(source_id) -> (Module, Vec<Diagnostic>)`)
- `crates/ynz-parser/tests/parse.rs` + snapshots

**Acceptance criteria**:
- [x] M1 source parses to the snapshot AST.
- [x] Three malformed-input cases produce three-part diagnostics with correct spans.
- [x] Parser recovers from each malformed case and produces a partial AST.
- [x] `parse` is salsa-tracked.
- [x] Every AST node carries a `SourceSpan` field.

---

## Phase 5: Type check + name resolution (M1 surface) as salsa queries
**PR scope**: Minimal type checker. Resolves the name `print` to a built-in. Verifies `main` is present and has signature `() -> nothing`. Verifies `print` is called with a string-typed argument. Treats `print` as a hardcoded built-in (no module system yet). Defines the parse-error-blocks-typeck gate.
**Branch**: `feat/typeck`
**Est. lines**: ~450

**Spec decisions locked in this phase:**
- **`print` semantics**: `print(s)` writes `s` to stdout followed by a single `\n` newline. This is the println-style behaviour, locked because the M1 codegen relies on libc `puts` (which appends `\n`).
- **Parse-error gate**: per `design/compiler-errors.md`, typeck does NOT run on functions whose body had parse errors — cascade errors mask the original bug.

**Files (expected scope)**:
- `crates/ynz-typeck/src/types.rs` (`Type::Nothing`, `Type::String`, `Type::Error`)
- `crates/ynz-typeck/src/builtins.rs` (production table: `print` → `(string) -> nothing`; test-only additions via `#[cfg(test)]` helper for M1's type-mismatch testability)
- `crates/ynz-typeck/src/check.rs` (the type-check logic, including the parse-error gate)
- `crates/ynz-typeck/src/queries.rs` (`check(source_id) -> (TypedModule, Vec<Diagnostic>)`)
- `crates/ynz-typeck/tests/check.rs` + snapshots

**Acceptance criteria**:
- [x] M1 source type-checks clean.
- [x] Empty source / missing `main` → three-part diagnostic.
- [x] `main` with wrong return type → three-part diagnostic.
- [x] `main` with parameters → three-part diagnostic.
- [x] `print` called with undefined identifier → three-part diagnostic.
- [x] Real type-mismatch path (via test-only builtin) → three-part diagnostic.
- [x] Parse-error gate works: AST with `Expr::Error` produces zero typeck diagnostics in that scope.
- [x] `check` is salsa-tracked and depends on `parse`.

---

## Phase 6: LLVM codegen (M1 surface)
**PR scope**: `ynz-codegen` takes a `TypedModule` and emits a relocatable object file via `inkwell`. Emits: one function `main` with C ABI (returns `i32`), a global constant string for the literal, an `extern "C"` declaration for `puts`, a call to `puts`. Returns 0 from `main`.
**Branch**: `feat/codegen`
**Est. lines**: ~450

**Spec decisions locked in this phase:**

- **Salsa output type — DECIDED.** The `codegen(source_id)` salsa query returns `Arc<CompiledArtifact>` where `CompiledArtifact { object_bytes: Vec<u8>, ir_text: String, sha256: [u8; 32] }`. The `inkwell::Module` and `inkwell::Context` are confined to a scoped non-salsa helper function `emit_artifact(typed_module) -> Result<CompiledArtifact>` that returns owned bytes; salsa never sees the inkwell types.

- **Reproducibility contract — DECIDED.** The byte-level contract is **SHA-256 of the relocatable object-file bytes**.
  1. Set an explicit module identifier on the `inkwell::Module` (e.g., `"ynz-m1-{file_id}"`).
  2. Set the target triple explicitly per host. SHA-256 golden is per-target-triple.
  3. Use deterministic LLVM target-machine options: disable PIC randomization, fixed code model, no debug info in M1.
  4. IR text is snapshotted via `insta` for codegen regression detection but informational only.

**Files (expected scope)**:
- `crates/ynz-codegen/src/lib.rs`
- `crates/ynz-codegen/src/artifact.rs` (`CompiledArtifact` struct + SHA-256 helper)
- `crates/ynz-codegen/src/emit.rs` (the inkwell-driven emission logic; scoped — inkwell types never escape this module)
- `crates/ynz-codegen/src/queries.rs` (`codegen(source_id) -> Arc<CompiledArtifact>`)
- `crates/ynz-codegen/tests/golden.rs` + `crates/ynz-codegen/tests/__golden__/hello.{triple}.sha256`

**Acceptance criteria**:
- [x] Generated object verifies.
- [x] Generated object SHA-256 matches the committed golden for the current host triple.
- [x] IR text snapshot matches `__snapshots__/hello.ll.snap`.
- [x] Test setup asserts `inkwell` is linked against LLVM 18.
- [x] Running codegen twice on the same source produces SHA-256-identical object bytes.
- [x] `codegen` is salsa-tracked, depends on `check`, returns `Arc<CompiledArtifact>`.
- [x] No `inkwell::Module` or `inkwell::Context` type is exposed outside `emit.rs`.

---

## Phase 7: Driver + linking + integration test
**PR scope**: `ynz` CLI exposes `ynz build <file>` and `ynz run <file>`. Pipeline: drive salsa queries → write `.o` via LLVM → invoke system `cc` to link with libc → produce a binary. `ynz run` execs the binary and propagates exit status.
**Branch**: `feat/driver`
**Est. lines**: ~400
**Objective**: `ynz run hello.ynz` (containing the M1 source) prints `hello, yinz\n` and exits 0.

**Files (expected scope)**:
- `crates/ynz-driver/src/main.rs` (CLI parsing via `clap`)
- `crates/ynz-driver/src/build.rs` (orchestration: read file → drive queries → emit object → link)
- `crates/ynz-driver/src/run.rs` (build + exec child)
- `crates/ynz-driver/src/load.rs` (read source file, verify UTF-8)
- `crates/ynz-driver/tests/integration.rs`
- `crates/ynz-driver/tests/fixtures/hello.ynz`
- `crates/ynz-driver/tests/fixtures/broken_main.ynz`
- `crates/ynz-driver/tests/__snapshots__/broken_main.stderr.snap`
- `script/check-llvm`

**Acceptance criteria**:
- [x] `ynz run tests/fixtures/hello.ynz` prints exactly `hello, yinz\n` and exits 0 on both Linux and macOS.
- [x] `ynz build tests/fixtures/hello.ynz` produces an executable on disk and exits 0.
- [x] Malformed-fixture integration test exits non-zero with a stderr matching snapshot byte-for-byte.
- [x] Empty-source integration test exits non-zero with stderr matching snapshot.
- [x] File-path-with-spaces integration test passes on Linux and macOS.
- [x] Invalid-UTF-8 source produces a three-part diagnostic from `load.rs`.
- [x] No salsa cache leaks across runs.
- [x] `script/check-llvm` verifies LLVM 18 on the host.

---

## Phase 8: M1 verification sweep + tag `v0.1.0-m1`
**PR scope**: No new features. TODO/FIXME sweep across the repo. Quality-checklist verification with evidence. Confirm M1 surface explicit-non-goals list still holds. Tag the milestone.
**Branch**: `chore/m1-verification`
**Est. lines**: ~50

**Acceptance criteria**:
- [x] Broad TODO sweep returns zero matches.
- [x] M1 "explicitly NOT" list manually audited — no slips.
- [x] Windows-support status re-evaluated and documented.
- [x] All quality-checklist items ticked with evidence.
- [x] CHANGELOG entry committed.
- [x] Git tag `v0.1.0-m1` created.

---

## Quality Checklist (M1 — all passed)

- [x] All compiler-emitted diagnostics use the three-part WHAT/WHAT-INSTEAD/WHY format
- [x] No banned-jargon strings in emitted diagnostics (enforced by P2's `tests/jargon_audit.rs`)
- [x] No `unwrap()` outside test code anywhere in `crates/`
- [x] No `panic!()` outside test code anywhere in `crates/`
- [x] All public APIs are documented with at least one-line `///` doc comments
- [x] `cargo clippy --workspace -- -D warnings` passes
- [x] `cargo fmt --check` passes
- [x] All snapshot tests have inline `// WHY:` comments stating the invariant they protect
- [x] CI passes on both Linux and macOS
- [x] `ynz run hello.ynz` integration test green
- [x] Salsa queries: every cross-stage call goes through a query
- [x] LLVM context lifetimes documented at the codegen module level
- [x] No dependency uses a `*` version constraint; `Cargo.lock` committed
