---
slug: v0-1-compiler
owner: patrick
status: active
files:
  - Cargo.toml
  - crates/**
  - tests/**
  - rust-toolchain.toml
  - .github/workflows/**
created: 2026-05-12
last_updated: 2026-05-12-r3
---

# Plan: v0.1 Compiler Implementation

Created: 2026-05-12
Status: m1_complete

## Context & Why

**Goal.** Build the Yinz v0.1 compiler — the first runnable slice of the Yinz language. v0.1's scope is "core language only, no stdlib" per `design/mvp-scope.md`. Output: an `ynz` CLI that can `build` and `run` programs written against the v0.1 language surface (variables, functions, types, ownership, generics, collections, options/unions/maybe, control flow, strings, errors, modules, concurrency keywords parsing sequentially, decimal128 numerics, doc comments, sensitive modifier).

**Why now.** All design work is done (~30 spec files, ~30 design files, decisions log, golden rules). Implementation has not started — zero Rust files exist. Continuing to design without building risks paper-spec drift: a feature that reads great in markdown but doesn't survive contact with the type checker. v0.1 is the first time the design gets verified against working code.

**Background.** Yinz is a compiled systems language (LLVM-targeting, no GC, ownership-based). The compiler is written in Rust (decision: `design/compiler-language.md`) with `inkwell` for LLVM, `salsa` for incremental computation (also serves the v0.2 LSP), `ariadne` for diagnostics, and a hand-written recursive-descent parser (so error messages can carry position-specific suggestions per Golden Rule 11 — the compiler is a teacher).

**Constraints.**
- Rust stable toolchain.
- LLVM via `inkwell` — LLVM version is pinned (target LLVM 18; revisit if inkwell stable lags).
- Salsa from day 1 (non-negotiable per `design/compiler-language.md` — retrofit cost would be a 6-month side-quest before v0.2 LSP).
- No external runtime dependencies in produced binaries except libc (for `puts`/`printf`/`malloc`/`free`) until decimal128 lands and we pull in a decimal library.
- Compiler-error format must follow the WHAT/WHAT-INSTEAD/WHY three-part shape from `design/compiler-errors.md` from day 1.

**Success criteria for the full v0.1 release (not M1):**
- `ynz run hello.ynz` works for every program covered by `spec/**/*.md` examples that don't import stdlib.
- All compiler errors follow the three-part format and pass an audit against `design/compiler-errors.md`'s banned-jargon list.
- Incremental rebuilds hit the sub-second target from `design/compiler.md` (single-file change, warm cache, typical project).
- The compiler is structured as queries (salsa) so v0.2 LSP can wrap them without restructuring.

**Success criteria for M1 (this milestone's contract):**
- `ynz run hello.ynz` where the source is `function main() -> nothing { print("hello, yinz") }` compiles and prints exactly `hello, yinz\n` and exits 0.
- The full pipeline (lex → parse → typeck → codegen → link) runs as salsa queries, not as straight function calls.
- A deliberately broken source file produces a three-part error message (WHAT/WHAT-INSTEAD/WHY) rendered by `ariadne` with the correct span.

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

## Research Findings

- `design/mvp-scope.md` defines v0.1 exhaustively — no design open questions remain for v0.1's surface.
- `design/compiler.md` defines architecture goals: Rust-like full-build cost is acceptable; Go-like incremental cost is the target; sub-second single-file incremental in typical projects.
- `design/compiler-language.md` locks the stack: cargo workspace, hand-written parser, `ariadne`, `salsa`, `inkwell`, optional `cranelift` later for fast debug builds.
- `design/compiler-errors.md` is the style spec for all diagnostics: required three-part WHAT/WHAT-INSTEAD/WHY format, banned-jargon list, multi-error strategy.
- `design/teaching-mission.md` codifies Golden Rule 11: every diagnostic teaches.
- `inkwell` releases track LLVM major versions. As of plan-write: latest stable inkwell supports LLVM 4–18 via feature flags. **Action in P1**: pin to LLVM 18 + matching `inkwell` feature flag.
- `salsa` 0.x is unstable (the `salsa-2022` rewrite consolidated under the `salsa` crate). **Action in P1**: pick the current stable salsa release and pin it; document in the plan that salsa version bumps are intentional changes, not casual upgrades.
- `decimal128` has no off-the-shelf fully-spec-compliant Rust crate. Candidates: Intel's `libdfp` via C bindings; `bigdecimal` (not IEEE 754 decimal128); roll our own minimal subset. **Decision deferred** — M1 doesn't need numerics. Decimal128 lands in M2 (literals) or M8 (deferred numerics polish).
- Unicode: `unicode-segmentation` crate handles graphemes; `char` handles code points natively in Rust; bytes are trivial. Pulls in for M7 (strings).
- M1 does not need decimal128, Unicode segmentation, ownership analysis, generics, monomorphization, or anything else. Walking-skeleton scope is genuinely minimal.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| LLVM/inkwell version mismatch on contributor machines | High | Blocks builds | Document required LLVM version in README + `rust-toolchain.toml`. CI pins exact LLVM version. Optionally vendor a `flake.nix` / devcontainer later. |
| Salsa API churn between minor versions breaks the build | Medium | Forced rewrites | Pin exact salsa version in workspace `Cargo.toml`. Treat salsa upgrade as its own PR with its own plan. |
| Hand-written parser produces poor error messages despite goal | High | Violates Golden Rule 11; eats away at the "teaching" mission | Phase 2 ships diagnostics infrastructure BEFORE the parser. Phase 4 requires snapshot tests for malformed-input errors, not just well-formed AST. Every parser PR after M1 gets a "bad input" test added in the same PR. |
| Walking-skeleton scope creep (just one more feature in M1) | High | M1 takes 3 weeks instead of one session, defeats the milestone principle | Explicit "out of scope for M1" list below. Mechanical enforcement gate in P3/P4: a `Token` enum variant-count test and AST `Stmt`/`Expr` variant-count test — adding a variant requires an inline `// test-ratchet: <reason>` marker AND a plan update. Reviewer vigilance alone is not enough. |
| Windows-platform support deferred without trigger date | Low (M1) → Medium (long-term) | Compiler refuses to run on contributor's platform | Re-evaluate at M8 verification sweep regardless of contributor mix; do not let "later" remain unbounded. M1+M2+...+M7 may pass on Linux/macOS only. |
| `cc` vs `clang` linker-flag differences across distros | Medium | M1 passes in CI, fails on a dev's Arch box | P7 driver invokes the system `cc` and documents the macOS-uses-clang/Linux-uses-distro-default reality. Failure mode is loud (linker stderr) not silent. Re-evaluate if a distro-specific bug actually bites. |
| LLVM IR non-determinism (module IDs, metadata numbering, ConstantStruct ordering drift between runs) | Medium | P6 reproducibility test passes locally, flakes in CI; "just update the snapshot" pattern erodes the contract | Lock the reproducibility contract to **object-file SHA-256 identity** with an explicit module identifier set on the `inkwell::Module`. IR text snapshot is informational only — failure of the SHA-256 check is the gate, not the IR-text diff. |
| Non-ASCII bytes in M1 string literals: undefined codegen behaviour | Medium | Silent wrong output; `print("café")` codegen path not specified | M1 lexer accepts UTF-8 source bytes inside string literals and passes them through codegen as raw bytes to `puts`. This is a one-line spec decision documented in P3 and P5. NOT a multi-byte-char rendering decision (that's M7 strings work) — purely "what bytes go into the global constant." |
| `>50 errors` cap from `design/compiler-errors.md` slips to a later milestone and becomes a surprise | Medium | A user with a broken file gets 800 diagnostics flooding stderr | P2 `DiagnosticBucket` enforces the 50-error cap with the standard "... and N more errors hidden" footer from day 1. Tested via snapshot of an artificially-generated 60-error bucket. |
| Decimal128 design rabbit hole | Medium | Could swallow weeks if attempted early | Out of scope for M1. Re-plan decimal strategy when M2 (literals) starts. |
| Snapshot tests rot — golden files updated reflexively when output changes | High | Defeats the test's purpose; matches the test-weakening graveyard corpse pattern | Snapshot file updates require an inline `// test-ratchet: <reason>` marker (project convention from global rules) AND a WHY-comment style note in the test explaining what invariant the snapshot protects. |
| Pipeline as straight function calls "for speed", retrofit salsa later | Medium | Same retrofit cost we're trying to avoid | Phase 3 onward expose work as salsa queries from day 1, even when the query has a single caller. The discipline matters more than the optimization. |
| LLVM IR codegen bugs slip through because we only test on hello-world | Medium (during M1) → High (downstream milestones) | Wrong binaries that run anyway | M1 codegen test asserts BOTH `cargo`-side snapshot of the IR text AND that the produced binary actually prints the expected stdout. Both checks, every codegen change. |
| Cross-platform path handling (Windows vs Unix) | Low (M1) → Medium (long-term) | Compiler refuses to run on contributor's platform | Restrict M1 to Linux + macOS (document in README). Windows is a deferred concern; pick it up before v0.1 ships if there's a contributor on Windows. |

---

## Questions

(None outstanding — strategic questions were answered in the planning conversation. Decimal128 implementation strategy is a deferred sub-decision, not a blocker for M1.)

---

## Risk Assessment & Rollout Strategy

**Risk level: LOW (production) / MEDIUM (architectural debt).**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | — |
| Touches auth/permissions | No | — |
| Raw SQL / literals | No | — |
| Modifies existing data | No | Greenfield |
| Third-party integration | Yes | `inkwell` (LLVM), `salsa`, `ariadne`, system linker |
| Changes existing endpoints | No | — |
| Wrong foundational choice cascades | Yes | Salsa retrofit, monolithic-crate split, parser style — all expensive to undo |

**Mitigations applied:**
- Salsa from day 1 → eliminates the retrofit-before-v0.2 risk.
- Cargo workspace with per-concern crates from day 1 → avoids the monolithic-rewrite trap.
- Snapshot tests + integration tests on the produced binary → IR drift caught fast.
- Hand-written parser style locked in P4 → parser style won't be retroactively replaced with a generator.

**Rollout plan:** N/A. The compiler is a development tool with no users yet. Each milestone tags a release (`v0.1.0-m1`, `v0.1.0-m2`, ..., `v0.1.0`). v0.1 final tag ships when M8 completes.

---

## Roadmap (milestones)

### Milestone 1 (M1): Hello-world end-to-end — single session
End-to-end walking skeleton. `function main() -> nothing { print("hello, yinz") }` compiles and runs. Proves the full pipeline (lex → parse → typeck → codegen → link → execute) works, with salsa wiring in place from the start.
**Flag**: N/A
**Status**: COMPLETE (2026-05-12) — 51 tests green, `ynz run hello.ynz` outputs `hello, yinz`

### Milestone 2 (M2): Literals + variables + arithmetic — multi-session
`let` / `const`, integer literals, float literals, binary arithmetic (`+ - * / %`), comparison (`< <= > >= == !=`), logical (`&& || !`), local type inference, printing numbers. Decimal128 design decision lands here.
**Flag**: N/A
**Status**: planned
**Depends on**: M1

### Milestone 3 (M3): Control flow + user functions — multi-session
`if` / `else`, multi-case `if`, `for x in ...` (with a temporary `range` builtin until proper iterables), `while`, early `return`, user-defined functions with parameters and return types, block scoping.
**Flag**: N/A
**Status**: planned
**Depends on**: M2

### Milestone 4 (M4): Types + ownership — multi-session
`type Foo { ... }` declarations with fields and methods. Ownership modifiers (`share`, `lend`, `give`, `copy`, `.freeze`). Ownership analysis as a salsa query. Heap allocation via libc `malloc`/`free`. Drop-on-scope-exit. Hardest milestone in v0.1 — ownership is the core safety property.
**Flag**: N/A
**Status**: planned
**Depends on**: M3

### Milestone 5 (M5): Generics + collections — multi-session
Function generics `function foo[T](...)` and type generics `array[T]` / `fixed[T]` / `map[K,V]`. Monomorphization. Bracket sugar (`arr[i]`, `m[k]`) desugars to `.get()` / `.set()`. `Iterable[T]` contract reserved for M7.
**Flag**: N/A
**Status**: planned
**Depends on**: M4

### Milestone 6 (M6): Options + unions + maybe + narrowing — multi-session
`options Status { ... }` declarations, union types `A or B`, `maybe T` sugar for `T or none`, `if (x is Type)` pattern narrowing as a flow-sensitive analysis.
**Flag**: N/A
**Status**: planned
**Depends on**: M5

### Milestone 7 (M7): Strings (full) + errors + iterables — multi-session
Full Unicode strings (`.get` for code points, `.byteAt`, `.graphemeAt`), interpolation, the `errors` keyword with flow-sensitive auto-propagation ("cascades"), `Iterable[T]` and `FallibleIterable[T]` contracts wired to `for x in iter` desugaring.
**Flag**: N/A
**Status**: planned
**Depends on**: M6

### Milestone 8 (M8): Modules + remaining + v0.1 tag — multi-session
`import` / `export`, root-relative paths, aliases with `as`, duplicate-name compile error. Doc comments (`///`) parsed and preserved on signatures. Sensitive type modifier (auto-redact in print output). Concurrency keywords (`wait`, `background`) parse and type-check, run sequentially. Decimal128 numerics finalised. Polish + audit + v0.1.0 tag.
**Flag**: N/A
**Status**: planned
**Depends on**: M7

---

## Current Milestone: M1 — Hello-world end-to-end

### What M1 explicitly is NOT (deferred to later milestones)

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

**If a phase below feels like it's drifting into any of the above, STOP and re-plan.**

---

### Phase 1: Repo scaffolding (cargo workspace + CI)
**PR scope**: Establish the Rust workspace, pin tool versions, set up CI. No compiler logic.
**Branch**: `feat/repo-scaffolding`
**Flag**: N/A
**Est. lines**: ~150 (mostly TOML + CI YAML)
**Objective**: Empty workspace builds clean; `ynz --version` returns a string; CI passes on a no-op PR.
**Why this phase exists**: Lock in workspace structure and version pins BEFORE any compiler code lands. Avoids the "we'll restructure later" trap.
**Current-state anchors**: greenfield — no Rust files exist yet.
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
**Deviation rule**: Any new crate / file beyond this list requires updating the plan first.
**Steps**:
1. Initialise `Cargo.toml` workspace with the seven crates above.
2. Add direct deps: `salsa = "<latest stable>"`, `inkwell = { version = "<llvm-18-compatible>", features = ["llvm18-0"] }`, `ariadne = "<latest>"`, `unicode-segmentation = "<latest>"` (pin exact versions; lockfile committed).
3. `rust-toolchain.toml` pins `channel = "stable"` and lists components `clippy`, `rustfmt`.
4. `ynz-driver` `main.rs`: parse `--version` and print a literal version string. Exit 0. No other behaviour.
5. CI workflow runs `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `cargo build --workspace --release` against Ubuntu and macOS runners with LLVM 18 installed.
6. README contains a one-paragraph project description + LLVM-18 install instructions for Linux + macOS.
**Acceptance criteria**:
- [ ] `cargo build --workspace` succeeds on a clean clone with LLVM 18 installed.
- [ ] `cargo run -p ynz-driver -- --version` prints a non-empty version string and exits 0.
- [ ] `cargo clippy --workspace -- -D warnings` passes.
- [ ] `cargo fmt --check` passes.
- [ ] CI is green on a PR that touches only this scaffolding.
- [ ] `Cargo.lock` is committed.
**Quality gate**:
- [ ] No `unwrap()` in `ynz-driver` main path (use `?` + error type even for the version command — sets convention from line 1).
- [ ] No dependency uses a `*` version constraint.
- [ ] CI uses LLVM 18, not "whatever the runner has."
**Verification**: `cargo build --workspace && cargo test --workspace && ./target/debug/ynz --version` on a fresh clone.

---

### Phase 2: Diagnostics infrastructure (before any parsing)
**PR scope**: `ynz-diagnostics` crate ships the three-part WHAT/WHAT-INSTEAD/WHY diagnostic type, `ariadne`-rendered output, multi-error accumulator with 50-error cap, severity tiers (Error / Warning / Suggestion — Suggestion is reserved for v0.4 but the tier exists from day 1), automated banned-jargon grep test over the workspace.
**Branch**: `feat/diagnostics`
**Flag**: N/A
**Est. lines**: ~500
**Objective**: A consumer crate can build a `Diagnostic` with a span and three-part message, push it to a `DiagnosticBucket`, and render the bucket as a string identical to a committed snapshot. The bucket enforces a 50-error cap with a standard "... and N more errors hidden" footer. An automated workspace-wide grep test fails CI if any banned-jargon word appears in diagnostic-construction call sites.
**Why this phase exists**: Golden Rule 11 (compiler is a teacher) is load-bearing. Shipping the parser before diagnostics infrastructure invites "we'll polish the errors later" which is exactly the duct-tape framing `~/.claude/rules/no-duct-tape.md` prohibits. Build the teaching layer first, then make the rest of the compiler use it. Per `design/compiler-errors.md`, the 50-error cap is a spec requirement — enforcing it from P2 means every later phase inherits the behaviour for free.
**Current-state anchors**:
- `design/compiler-errors.md` — the style spec the type must encode (three-part format, banned jargon, severity tiers, 50-error cap).
- `design/teaching-mission.md` — the rationale.
**Files (expected scope)**:
- `crates/ynz-diagnostics/src/lib.rs`
- `crates/ynz-diagnostics/src/diagnostic.rs` (the `Diagnostic` struct: severity, span, three fields `what` / `what_instead` / `why`)
- `crates/ynz-diagnostics/src/bucket.rs` (multi-error accumulator with 50-error cap)
- `crates/ynz-diagnostics/src/render.rs` (`ariadne` integration, "and N more hidden" footer)
- `crates/ynz-diagnostics/src/span.rs` (`SourceSpan` — file id + byte range)
- `crates/ynz-diagnostics/src/banned_jargon.rs` (const array of banned words extracted from `design/compiler-errors.md`)
- `crates/ynz-diagnostics/tests/snapshots.rs` + `crates/ynz-diagnostics/tests/__snapshots__/` (uses `insta` for snapshot testing)
- `crates/ynz-diagnostics/tests/jargon_audit.rs` (workspace-wide grep test — fails if any banned word appears in a diagnostic-construction context)
**Deviation rule**: Severity tiers, three-part field shape, span representation, 50-error cap value, and banned-jargon list are all load-bearing for every downstream crate. Any change to these requires re-planning.
**Steps**:
1. Define `Severity { Error, Warning, Suggestion }` as a Rust enum (no `as_const` here — this is Rust, idiomatic enums are correct).
2. Define `Diagnostic { severity, span, what: String, what_instead: String, why: String, related: Vec<RelatedSpan> }`. `what_instead` and `why` are required (not `Option`). Constructor panics with a clear message if any of the three is empty — encoding Golden Rule 11 in the type system prevents "I'll add WHY later."
3. Define `DiagnosticBucket` that owns a `Vec<Diagnostic>` plus a `hidden_count: usize`. `push` accepts diagnostics up to a cap of 50 errors (warnings + suggestions not counted against the cap); pushes beyond the cap increment `hidden_count`. Exposes `iter`, `has_errors`, `into_iter`, `hidden_count`.
4. Define `BANNED_JARGON: &[&str]` constant extracted verbatim from the table in `design/compiler-errors.md`. Test ensures the constant stays in sync (snapshot of sorted constant vs sorted parse of the markdown table — drift between the two fails the test).
5. Implement `render(bucket, source_map) -> String` using `ariadne`. Render order: all errors first by source position, then warnings, then suggestions. If `hidden_count > 0`, append `... and {N} more errors hidden` footer.
6. Automated banned-jargon audit test (`tests/jargon_audit.rs`): walk every `.rs` file in `crates/`, parse string literals passed to `Diagnostic::new` / `Diagnostic::error` / `Diagnostic::warning` / `Diagnostic::suggestion` construction calls (cheap regex over call sites, no full AST parse needed for v0.1), assert none contain any banned word. Test runs on `cargo test --workspace`. Fails CI on first banned word.
7. Add `insta` snapshot tests for: (a) single error with span, (b) two errors in the same file at different spans, (c) one error + one warning, (d) suggestion-only diagnostic (proves the tier renders even though M1 won't emit one), (e) 60-error bucket — confirms the cap at 50 + "...and 10 more errors hidden" footer.
8. Add a `// WHY:` comment on every snapshot test stating the invariant it protects (e.g., "WHY: three-part format is mandatory per Golden Rule 11. Snapshots catch silent collapse to one-line error strings."). Per global testing rules.
**Acceptance criteria**:
- [ ] `Diagnostic` constructor panics with a clear message if any of `what` / `what_instead` / `why` is empty.
- [ ] `ariadne` renders the diagnostic with the source line, a caret pointing to the span, and the three fields visible in the output.
- [ ] `DiagnosticBucket` caps at 50 errors; the 51st error increments `hidden_count` and the rendered output ends with `... and N more errors hidden`.
- [ ] Snapshot tests exist for the five cases listed above and match committed golden files.
- [ ] `tests/jargon_audit.rs` passes on the empty workspace (no diagnostic call sites yet — passes trivially) and is wired to run on every `cargo test --workspace`.
- [ ] `BANNED_JARGON` constant stays in sync with `design/compiler-errors.md` via a snapshot-style sync test.
- [ ] Public API exposes `Diagnostic`, `DiagnosticBucket`, `Severity`, `SourceSpan`, `render` — and nothing else (no escape hatch for two-part diagnostics).
**Quality gate**:
- [ ] No `unwrap()` outside test code.
- [ ] No `String::new()` placeholder messages — empty strings rejected at construction.
- [ ] `tests/jargon_audit.rs` exists and runs in CI (proves the automated enforcement gate is real, not a comment).
**Verification**: `cargo test -p ynz-diagnostics` passes; `cargo test --workspace` exercises the jargon audit; snapshot files committed under `crates/ynz-diagnostics/tests/__snapshots__/`.

---

### Phase 3: Lexer (M1 surface only) as salsa query
**PR scope**: `ynz-parser::lex` salsa query takes source text and returns a `TokenStream`. Handles only what M1 needs: identifiers, the keywords `function` and `nothing`, string literals, `(`, `)`, `{`, `}`, `->`, and whitespace/newlines (skipped). Multi-error: unknown characters and unterminated strings produce diagnostics, lexer continues to next sensible boundary.
**Branch**: `feat/lexer`
**Flag**: N/A
**Est. lines**: ~400
**Objective**: Lexing the M1 source `function main() -> nothing { print("hello, yinz") }` produces the expected token stream (asserted against a snapshot) and produces zero diagnostics.
**Why this phase exists**: Lexing is the first salsa input → output query. Establishes the salsa pattern that every later compiler stage will follow.
**Current-state anchors**:
- `spec/variables.md`, `spec/functions.md`, `spec/strings.md` — for token shapes (identifiers, keywords, string literals).
- `design/compiler-language.md` — confirms hand-written, not generator.
**Files (expected scope)**:
- `crates/ynz-parser/src/token.rs` (`Token` enum: `Function`, `Nothing`, `Identifier(String)`, `StringLit(String)`, `LParen`, `RParen`, `LBrace`, `RBrace`, `Arrow`, `Eof`)
- `crates/ynz-parser/src/lexer.rs` (the lexing logic, hand-written)
- `crates/ynz-parser/src/queries.rs` (salsa input + first query: `lex(source_id) -> (Vec<Spanned<Token>>, DiagnosticBucket)`)
- `crates/ynz-parser/tests/lex.rs` + snapshots
**Deviation rule**: Only the token kinds listed above. Adding `Let`, `Const`, integer literals, etc. = M2 work. Enforced by a `variant_count!(Token)` test that pins the variant count — bumping it requires an inline `// test-ratchet: <reason>` marker AND a plan update.

**Spec decision (locked in this phase):** string-literal contents in M1 are UTF-8 byte sequences passed through unchanged from source to codegen. The lexer does NOT decode them into a `String` — they're stored as `Vec<u8>` on the `StringLit` token. ASCII source is the common path; non-ASCII bytes (e.g., `"café"`) are accepted and round-trip as raw bytes to `puts`. No `\n` / `\t` / `\"` escape processing in M1 (escape decoding lands with the M2 strings work). Source files MUST be valid UTF-8; non-UTF-8 source bytes produce a three-part diagnostic at file load time (`ynz-driver` responsibility, not the lexer).
**Steps**:
1. Define `Token` with the variants above. Wrap in `Spanned<T> { value: T, span: SourceSpan }` for position tracking. `StringLit` carries `Vec<u8>` of the literal bytes excluding the surrounding quotes.
2. Hand-write the lexer: a byte iterator (not char iterator — we work at byte level to preserve UTF-8 bytes inside strings) with peek, branching on first byte of each token. Standard recursive-descent-friendly shape.
3. Set up salsa: define the `SourceText` input (file id → string), the `lex` tracked query taking a source id and returning `(Vec<Spanned<Token>>, Vec<Diagnostic>)`.
4. Error cases: unknown byte → diagnostic + skip byte + continue. Unterminated string → diagnostic at the opening quote + recover at next newline or `}`.
5. Mechanical scope-creep gate: add a `#[test] fn m1_token_variant_count_locked()` test asserting `std::mem::variant_count::<Token>()` (or a manual count via match-exhaustive helper if the unstable intrinsic isn't available) equals the M1 count. Bumping the count fails the test until the inline `// test-ratchet: M2 adds N for {feature}` marker is present. The test-ratchet hook (global) enforces the marker requirement on Edit/Write.
6. Tests: snapshot of the token stream for the M1 source. Negative tests for:
   - `function main() -> nothing { print("oops` (unterminated string)
   - `function main() -> nothing { print($) }` (unknown char)
   - `function main() -> nothing { print("café") }` (non-ASCII bytes inside string — should lex clean and produce a `StringLit` with the UTF-8 bytes intact)
   - Empty source (zero tokens, no diagnostics)
   - Whitespace-only source (zero tokens, no diagnostics)
   Each negative test asserts the diagnostic count AND that the lexer still produced a recoverable token stream (no panic, no early bail).
7. WHY-comments on tests per global rules — "WHY: unterminated string must not bail the lexer; downstream stages need a complete token stream to give the user all errors at once."
**Acceptance criteria**:
- [ ] Lexing the M1 source produces a token stream matching the committed snapshot.
- [ ] Unknown-char input produces a diagnostic with the correct span AND a non-empty recovered token stream.
- [ ] Unterminated string input produces a diagnostic at the opening quote AND a non-empty recovered token stream.
- [ ] Non-ASCII bytes inside a string lex clean — bytes round-trip via the token's `Vec<u8>` payload.
- [ ] Empty and whitespace-only source produce zero tokens and zero diagnostics.
- [ ] All token positions (start byte, end byte) are correct — verified by reconstructing the lexeme from the source via the span.
- [ ] The `lex` query is salsa-tracked (changing the input invalidates the cache).
- [ ] `m1_token_variant_count_locked` test pins the variant count for M1.
**Quality gate**:
- [ ] No `unwrap()` in lexer code paths.
- [ ] No banned-jargon strings in any diagnostic emitted from the lexer (enforced by P2's `jargon_audit` test now that real diagnostics exist).
- [ ] No silent panics — every error path goes through `DiagnosticBucket`.
**Verification**: `cargo test -p ynz-parser lex::` passes. Snapshot files committed. `jargon_audit` test from P2 stays green now that the lexer emits real diagnostic strings.

---

### Phase 4: AST + parser (M1 surface) as salsa query
**PR scope**: `ynz-ast` defines the AST node types; `ynz-parser::parse` is a salsa query taking the token stream and returning a `Module` AST. Multi-error: parse errors accumulate, parser recovers to next `}` or end of file.
**Branch**: `feat/parser`
**Flag**: N/A
**Est. lines**: ~600
**Objective**: Parsing the M1 source produces an AST matching the committed snapshot, with zero diagnostics. Malformed input (missing return type, missing body, missing closing brace) produces ariadne-rendered three-part errors per snapshot.
**Why this phase exists**: First time the parser hand-rolled approach is tested. Error-recovery patterns established here will be copied by every later parser PR.
**Current-state anchors**:
- `spec/functions.md` — function decl syntax.
- `spec/main.md` — entry-point shape.
- `design/compiler-language.md` — hand-written justification.
**Files (expected scope)**:
- `crates/ynz-ast/src/lib.rs` (re-exports)
- `crates/ynz-ast/src/nodes.rs` (`Module`, `FunctionDecl`, `Block`, `Stmt::ExprStmt`, `Expr::Call`, `Expr::Ident`, `Expr::StringLit`, `Type::Nothing`, `Type::Named`)
- `crates/ynz-parser/src/parser.rs` (recursive-descent parser)
- `crates/ynz-parser/src/queries.rs` (add `parse(source_id) -> (Module, Vec<Diagnostic>)`)
- `crates/ynz-parser/tests/parse.rs` + snapshots
**Deviation rule**: Only the AST nodes above. `Let`, `BinOp`, `If`, etc. are M2+ — explicitly forbidden in this phase. Enforced by `m1_stmt_variant_count_locked` and `m1_expr_variant_count_locked` tests that pin the `Stmt` and `Expr` enum variant counts; bumping either fails the test until an inline `// test-ratchet: <reason>` marker is added.
**Steps**:
1. Define AST nodes with spans on every node (downstream diagnostics need them).
2. Implement parser as a struct holding a token cursor + diagnostic bucket. Methods per non-terminal: `parse_module`, `parse_function_decl`, `parse_block`, `parse_stmt`, `parse_expr`, `parse_call`, `parse_string_lit`.
3. Error recovery strategy: on unexpected token, emit diagnostic, then scan forward to the next `}` or eof. Document the strategy in a top-of-file comment.
4. Salsa: `parse(source_id)` depends on `lex(source_id)`. Tracked.
5. Tests:
   - Snapshot of `Module` AST for M1 source.
   - Negative: missing `->` between `()` and return type → three-part diagnostic, parser recovers and produces an AST with a placeholder `Type::Error` node.
   - Negative: missing closing `}` → diagnostic at end-of-file with span pointing to the unclosed `{`.
   - Negative: `print` called with no arguments → currently this parses fine (no arg list typing yet); add to M2's test list, not M1.
   - Adversarial: trailing garbage after the function (`function main() -> nothing { } extra }`) — parser emits "unexpected token after function decl" diagnostic, recovers cleanly to EOF.
   - Adversarial: empty source — `Module` is produced with zero items, zero diagnostics. (Note: typeck then emits "no `main` defined" — that test lives in P5.)
   - Adversarial: source with only whitespace and a comment — same shape as empty source, zero items, zero diagnostics.
   - Adversarial: BOM-prefixed source (`\u{FEFF}` at byte 0) — lexer-level concern technically, but the parser must not crash if the lexer passes through a leading BOM. Decide: skip BOM in lexer (M1) → parser sees no BOM token, identical to non-BOM source.
6. WHY-comments per the testing rules: "WHY: parser must accumulate errors; bailing on first error means the user only sees one bug at a time. Snapshot guards against silent regression to 'return on first error.'"
**Acceptance criteria**:
- [ ] M1 source parses to the snapshot AST.
- [ ] Three malformed-input cases above produce three-part diagnostics with correct spans.
- [ ] Parser recovers from each malformed case and produces a partial AST (not `None`, not `panic!`).
- [ ] `parse` is salsa-tracked.
- [ ] Every AST node carries a `SourceSpan` field (no spanless nodes).
**Quality gate**:
- [ ] No `unwrap()` in parser code.
- [ ] No silent `return None` on error — every error path emits a diagnostic.
- [ ] Banned-jargon audit clean on emitted diagnostic strings.
- [ ] Recovery strategy documented as a doc comment on the parser struct.
**Verification**: `cargo test -p ynz-parser parse::` passes.

---

### Phase 5: Type check + name resolution (M1 surface) as salsa queries
**PR scope**: Minimal type checker. Resolves the name `print` to a built-in. Verifies `main` is present and has signature `() -> nothing`. Verifies `print` is called with a string-typed argument. Treats `print` as a hardcoded built-in (no module system yet). Defines the parse-error-blocks-typeck gate.
**Branch**: `feat/typeck`
**Flag**: N/A
**Est. lines**: ~450
**Objective**: M1 source type-checks clean. Malformed signatures, undefined names, missing-`main`, and wrong-argument-types all produce three-part diagnostics. Files with parse errors do not produce additional typeck noise.
**Why this phase exists**: First salsa query that touches more than one input (it depends on the AST AND on a built-ins table). Establishes the pattern.

**Spec decisions locked in this phase:**
- **`print` semantics**: `print(s)` writes `s` to stdout followed by a single `\n` newline. This is the println-style behaviour, locked because the M1 codegen relies on libc `puts` (which appends `\n`). Decision documented in `crates/ynz-typeck/src/builtins.rs` doc comment so it's findable from the codebase, not just the plan.
- **Parse-error gate**: per `design/compiler-errors.md`, typeck does NOT run on functions whose body had parse errors — cascade errors mask the original bug. The `check` query inspects the AST for `Type::Error` / `Expr::Error` placeholder nodes inserted by the parser recovery path. If a function body contains any error placeholder, typeck skips that function and emits no diagnostics for it. Top-level diagnostics (missing `main`) still run.

**Current-state anchors**:
- `spec/main.md` — `main` shape (`() -> nothing`).
- `spec/types.md` — for `nothing` as a type.
- `spec/strings.md` — for the string-literal type.
- `design/compiler-errors.md` — the "type-check only runs if parse has no errors in the relevant scope" rule.
**Files (expected scope)**:
- `crates/ynz-typeck/src/types.rs` (`Type::Nothing`, `Type::String`, `Type::Error`)
- `crates/ynz-typeck/src/builtins.rs` (production table: `print` → `(string) -> nothing`; test-only additions via `#[cfg(test)]` helper for M1's type-mismatch testability)
- `crates/ynz-typeck/src/check.rs` (the type-check logic, including the parse-error gate)
- `crates/ynz-typeck/src/queries.rs` (`check(source_id) -> (TypedModule, Vec<Diagnostic>)`)
- `crates/ynz-typeck/tests/check.rs` + snapshots
**Deviation rule**: Only `nothing`, `string`, `Type::Error` types in M1. No `int`, `float`, `number`, no user-defined types — that's M2/M4. Enforced by a `m1_type_variant_count_locked` test.
**Steps**:
1. Define the minimal `Type` enum and a `TypedModule` (AST + per-expr type annotation).
2. `builtins.rs`: hardcode `print(string) -> nothing` with a doc-comment stating the trailing-newline semantics. Expose a `#[cfg(test)]` constructor `BuiltinTable::with_test_builtin(name, sig)` so the type-mismatch path can be tested with a synthetic `_test_takes_nothing(nothing) -> nothing` builtin. Production code path never adds test builtins.
3. Check pass: walk top-level declarations, verify `main` exists with `() -> nothing` signature. For each function, check the parse-error gate first; if clean, walk the body, type each `Expr`, verify call sites match callee signature. Emit three-part diagnostics on mismatch.
4. Tests (all with `// WHY:` comments):
   - **Happy path**: clean M1 source type-checks with empty diagnostic bucket.
   - **Missing `main`**: empty AST (zero items) → three-part diagnostic about needing `function main()`. (Covers reviewer adversarial case "empty source file".)
   - **Wrong `main` return type**: `function main() -> string { ... }` → three-part diagnostic.
   - **Wrong `main` arity**: `function main(x: string) -> nothing { ... }` → three-part diagnostic about `main` taking no parameters.
   - **Undefined identifier**: `print(unknownIdent)` → three-part diagnostic about the unresolved name.
   - **Type mismatch (the real test)**: a fixture using the test-only `_test_takes_nothing` builtin called with a string literal (`_test_takes_nothing("hi")`) → three-part diagnostic about expected `nothing`, got `string`. THIS is the test that covers the load-bearing type-mismatch path. Built with the `#[cfg(test)]` builtin helper so M1 doesn't ship with integer literals just to enable the test.
   - **Parse-error gate**: a fixture whose body had a parser-recovery error (mimicked by hand-constructing an AST with an `Expr::Error` node) → typeck emits ZERO diagnostics about that body; only top-level diagnostics fire. Asserts no cascade noise.
5. WHY-comments — "WHY: signature mismatch must produce three-part diagnostic. Catches regression where typeck silently degrades to one-line errors." "WHY: parse-error gate prevents cascade-noise. Per design/compiler-errors.md, typeck on broken parses doubles user-facing error count for the same underlying bug."
**Acceptance criteria**:
- [ ] M1 source type-checks clean (zero diagnostics).
- [ ] Empty source / missing `main` → three-part diagnostic.
- [ ] `main` with wrong return type → three-part diagnostic.
- [ ] `main` with parameters → three-part diagnostic.
- [ ] `print` called with undefined identifier → three-part diagnostic.
- [ ] Real type-mismatch path (via test-only builtin) → three-part diagnostic. **This is the load-bearing test that prevents Required Fix #3's silent gap.**
- [ ] Parse-error gate works: AST with `Expr::Error` produces zero typeck diagnostics in that scope.
- [ ] `check` is salsa-tracked and depends on `parse`.
**Quality gate**:
- [ ] No `unwrap()` in check code.
- [ ] `tests/jargon_audit.rs` (from P2) stays green now that typeck emits real diagnostic strings.
- [ ] `print` builtin is in a single source-of-truth table — not hardcoded in the check function (sets pattern for M8 expansion).
- [ ] Test-only builtin helper is gated by `#[cfg(test)]` — confirmed by inspecting the production binary symbols for absence of `_test_takes_nothing`.
- [ ] Parse-error gate has its own dedicated test, not asserted as a side effect of another test.
**Verification**: `cargo test -p ynz-typeck` passes; `cargo test --workspace` still passes the jargon audit.

---

### Phase 6: LLVM codegen (M1 surface)
**PR scope**: `ynz-codegen` takes a `TypedModule` and emits a relocatable object file via `inkwell`. Emits: one function `main` with C ABI (returns `i32`), a global constant string for the literal, an `extern "C"` declaration for `puts`, a call to `puts`. Returns 0 from `main`.
**Branch**: `feat/codegen`
**Flag**: N/A
**Est. lines**: ~450
**Objective**: Generated object file's SHA-256 is byte-identical across repeated runs on the same host and matches a committed golden hash. Generated IR (informational only) is also snapshotted for debugging codegen regressions.
**Why this phase exists**: First time the pipeline produces an executable artifact. Codegen bugs are notoriously hard to catch — both an artifact-hash check AND end-to-end execution tests are required from day 1.

**Spec decisions locked in this phase (Required Fix #1 + #2):**

- **Salsa output type — DECIDED.** The `codegen(source_id)` salsa query returns `Arc<CompiledArtifact>` where `CompiledArtifact { object_bytes: Vec<u8>, ir_text: String, sha256: [u8; 32] }`. **Why this and not the alternatives:**
  - `String` (IR text only) was rejected: IR is informational; object bytes are what the linker consumes. Storing IR text alone forces the driver to re-run codegen to produce the object — defeats the cache.
  - Raw `inkwell::Module` was rejected: lifetimes don't cross salsa query boundaries cleanly, and the LLVM context isn't `Send + Sync` in a usable way.
  - `Vec<u8>` (just object bytes) was rejected: we want the IR text alongside for snapshot regression detection. Bundling both in an `Arc<CompiledArtifact>` costs one allocation per cache miss — negligible.
  - The `inkwell::Module` and `inkwell::Context` are confined to a scoped non-salsa helper function `emit_artifact(typed_module) -> Result<CompiledArtifact>` that returns owned bytes; salsa never sees the inkwell types. This is the discipline that keeps salsa-from-day-1 honest at the codegen boundary.

- **Reproducibility contract — DECIDED.** The byte-level contract is **SHA-256 of the relocatable object-file bytes**. Steps to guarantee determinism:
  1. Set an explicit module identifier on the `inkwell::Module` (e.g., `"ynz-m1-{file_id}"`) — overrides LLVM's default which may include timestamps.
  2. Set the target triple explicitly per host (e.g., `x86_64-unknown-linux-gnu` on Linux CI, `aarch64-apple-darwin` on macOS CI). The SHA-256 golden is therefore **per-target-triple**, not single-value — store as `__golden__/hello.{triple}.sha256`.
  3. Use deterministic LLVM target-machine options: disable PIC randomization, fixed code model, no debug info in M1 (debug info embeds paths and timestamps).
  4. IR text is snapshotted via `insta` for codegen regression detection but **the IR text snapshot is informational; only the SHA-256 check is the gate**. IR text can drift between LLVM patch versions in ways that don't affect the binary; that's fine for IR but unacceptable for the object.
  5. If the LLVM version on the runner doesn't match the pinned LLVM 18 version exactly, the SHA-256 will differ. Acceptance criterion below requires LLVM version to be asserted at codegen-test setup time.

**Current-state anchors**:
- `design/compiler.md` — confirms LLVM via inkwell.
- `design/compiler-language.md` — inkwell decision rationale.
**Files (expected scope)**:
- `crates/ynz-codegen/src/lib.rs`
- `crates/ynz-codegen/src/artifact.rs` (`CompiledArtifact` struct + SHA-256 helper)
- `crates/ynz-codegen/src/emit.rs` (the inkwell-driven emission logic; scoped — inkwell types never escape this module)
- `crates/ynz-codegen/src/queries.rs` (`codegen(source_id) -> Arc<CompiledArtifact>`)
- `crates/ynz-codegen/tests/golden.rs` + `crates/ynz-codegen/tests/__golden__/hello.x86_64-linux.sha256` + `crates/ynz-codegen/tests/__golden__/hello.aarch64-darwin.sha256` + `crates/ynz-codegen/tests/__snapshots__/hello.ll.snap` (informational IR snapshot)
**Deviation rule**: Only `puts` from libc; no `printf`, no `malloc`, no other externals. Adding more externals = breach.
**Steps**:
1. Initialise an `inkwell::context::Context`, build a `Module` with an explicit deterministic identifier.
2. Declare `extern "C" puts(*const u8) -> i32`.
3. Build a global constant: null-terminated bytes for the literal.
4. Build `main`: `define i32 @main()`, call `puts(ptr_to_constant)`, return `0`.
5. Verify the module (`module.verify()?`).
6. Configure the target machine with the explicit triple + deterministic options.
7. Emit relocatable object bytes via `target_machine.write_to_memory_buffer(...)`.
8. Compute SHA-256 over the object bytes.
9. Snapshot the IR text via `insta` (informational only).
10. Construct `Arc<CompiledArtifact>` and return from the salsa query. WHY-comment on the SHA-256 test: "WHY: object-file bytes are the reproducibility contract — IR text drifts on LLVM patch versions, object bytes do not. If this golden changes, the underlying binary changed."
11. Salsa: `codegen(source_id)` depends on `check(source_id)`. `inkwell::Module` lifetime confined to `emit_artifact` — verified by inspecting `emit.rs` for `pub` exposure of any inkwell type (none allowed).
**Acceptance criteria**:
- [ ] Generated object verifies (module-level verify passes inside `emit_artifact`).
- [ ] Generated object SHA-256 matches the committed golden for the current host triple.
- [ ] IR text snapshot matches `__snapshots__/hello.ll.snap` (informational — failure here triggers test-ratchet review but does NOT block on its own).
- [ ] Test setup asserts `inkwell` is linked against LLVM 18 (use `inkwell::support::get_llvm_version()` or equivalent) and fails with a three-part diagnostic if the version differs.
- [ ] Running codegen twice on the same source produces SHA-256-identical object bytes (cross-run determinism test).
- [ ] `codegen` is salsa-tracked, depends on `check`, returns `Arc<CompiledArtifact>`.
- [ ] No `inkwell::Module` or `inkwell::Context` type is exposed outside `emit.rs` (grep-asserted in the test file: `! rg "pub.*inkwell::Module" crates/ynz-codegen/src/`).
**Quality gate**:
- [ ] No `unwrap()` in codegen code outside of the verify call (which is a sanity check on output we ourselves built).
- [ ] Codegen returns a `Result` that propagates diagnostics, not panics.
- [ ] LLVM context lifetime is documented at the top of `emit.rs`.
- [ ] Salsa output is `Arc<CompiledArtifact>`, never a borrowed inkwell type.
**Verification**: `cargo test -p ynz-codegen` passes on both Linux and macOS runners. Both `__golden__/hello.{triple}.sha256` files committed.

---

### Phase 7: Driver + linking + integration test
**PR scope**: `ynz` CLI exposes `ynz build <file>` and `ynz run <file>`. Pipeline: drive salsa queries → write `.o` via LLVM → invoke system `cc` to link with libc → produce a binary. `ynz run` execs the binary and propagates exit status.
**Branch**: `feat/driver`
**Flag**: N/A
**Est. lines**: ~400
**Objective**: `ynz run hello.ynz` (containing the M1 source) prints `hello, yinz\n` and exits 0. Captured in an integration test that runs the actual binary on the actual host.
**Why this phase exists**: First and most important end-to-end test. The walking skeleton is incomplete until a binary runs.
**Current-state anchors**:
- `design/compiler.md` — debug-build is the default for `ynz build`.
- `spec/tooling.md` — `ynz build` / `ynz run` user-facing behaviour.
**Files (expected scope)**:
- `crates/ynz-driver/src/main.rs` (CLI parsing — manual or via `clap`; pick clap)
- `crates/ynz-driver/src/build.rs` (orchestration: read file → drive queries → emit object → link)
- `crates/ynz-driver/src/run.rs` (build + exec child)
- `crates/ynz-driver/src/load.rs` (read source file, verify UTF-8 — emit three-part diagnostic if invalid)
- `crates/ynz-driver/tests/integration.rs` (compiles a fixture, runs it, asserts stdout)
- `crates/ynz-driver/tests/fixtures/hello.ynz`
- `crates/ynz-driver/tests/fixtures/broken_main.ynz` (malformed fixture for negative test)
- `crates/ynz-driver/tests/__snapshots__/broken_main.stderr.snap` (full rendered diagnostic — what the user sees)
- `script/check-llvm` (shell script verifying LLVM 18 is installed and matches the inkwell-pinned version; runnable locally + invoked by CI)

**Deviation rule**: `ynz build --release`, `ynz watch`, `ynz fmt` — all out of scope. M1 ships `ynz build` (debug only) and `ynz run` (debug only).

**Cross-distro reality note**: On macOS, `cc` resolves to clang via Xcode CLT. On Linux, `cc` resolves to gcc or clang depending on distro defaults. M1 driver invokes the system `cc` and relies on standard libc linkage (`-lc` is usually implicit). Failure mode if the linker behaves unexpectedly is loud (linker stderr surfaces to the user), not silent — that's acceptable for M1. If a contributor reports a distro-specific bug we re-evaluate; we don't pre-emptively code around hypothetical linker differences.

**Steps**:
1. Add `clap = "<latest stable>"` to `ynz-driver`. Define `build` and `run` subcommands.
2. `load.rs`: read source file from disk, verify the bytes are valid UTF-8, emit a three-part diagnostic on invalid UTF-8 with a span pointing at the offending byte offset.
3. `build <file>`: load source → call codegen query → write object via LLVM target machine to a temp dir → invoke `cc` (system path lookup; if not found, emit three-part diagnostic) to link with `-lc` → emit binary next to the source (e.g. `hello` next to `hello.ynz`).
4. `run <file>`: invoke `build`, then `std::process::Command::new(binary).status()` and propagate exit code.
5. **Malformed fixture content (locked):** `tests/fixtures/broken_main.ynz` contains:
   ```
   function main() {
     print("missing return type")
   }
   ```
   This is `main` declared without `-> nothing`. The parser recovers (per P4), typeck then emits one three-part diagnostic about the missing return type on `main`. Single-error fixture so the snapshot is small and stable.
6. **Negative-test assertion shape (locked):** the test snapshots the FULL stderr via `insta` and asserts an exact match against `__snapshots__/broken_main.stderr.snap`. NOT a substring match. NOT a header-presence check. Full byte-for-byte match (modulo ANSI color codes — disable colors in test env via `CLICOLOR=0`). If the diagnostic shape changes, the snapshot fails loudly and the test-ratchet hook requires an explicit `// test-ratchet: <reason>` to update it.
7. **File-path-with-spaces integration test:** create a temp directory with a space in its name (`std::env::temp_dir().join("my folder/hello.ynz")`), copy the hello fixture into it, run `ynz run "{path}"`. Asserts the driver quotes the path correctly when shelling out to `cc`. Passes on both Linux and macOS.
8. **Empty-source integration test:** `tests/fixtures/empty.ynz` (zero bytes) — `ynz build` exits non-zero with a three-part diagnostic about missing `main`. Snapshot of stderr committed.
9. WHY-comments — "WHY: this is the contract for the entire compiler. If this test ever flips green-to-red without an intentional behaviour change, something foundational broke." "WHY: file paths with spaces are the canonical shell-injection / quoting trap. If this test fails, the driver is shelling out unsafely."
**Acceptance criteria**:
- [ ] `ynz run tests/fixtures/hello.ynz` prints exactly `hello, yinz\n` (yes, with trailing newline from libc `puts`) and exits 0 on both Linux and macOS in CI.
- [ ] `ynz build tests/fixtures/hello.ynz` produces an executable on disk and exits 0.
- [ ] Malformed-fixture (`broken_main.ynz`) integration test exits non-zero with a stderr matching `__snapshots__/broken_main.stderr.snap` byte-for-byte.
- [ ] Empty-source integration test exits non-zero with a stderr matching `__snapshots__/empty.stderr.snap` byte-for-byte.
- [ ] File-path-with-spaces integration test passes on Linux and macOS.
- [ ] Invalid-UTF-8 source produces a three-part diagnostic from `load.rs` (no panic, no garbled string).
- [ ] No salsa cache leaks across runs (each CLI invocation builds a fresh database).
- [ ] `script/check-llvm` verifies LLVM 18 on the host and is invoked by CI before the test suite runs.
**Quality gate**:
- [ ] No `unwrap()` in driver code; errors propagate to a top-level handler that renders them via `ariadne` and exits with a non-zero code.
- [ ] CLI usage text mentions only the two M1 subcommands (no stubs for `watch`/`fmt`/`test`).
- [ ] Linker invocation handles `cc` not being present with a clear three-part error.
- [ ] All paths shelled out via `Command::new(...).arg(path)` (never via shell string concat) — verified by grep for `Command::new("sh")` or `format!` into linker args (should be zero matches).
**Verification**: `cargo test -p ynz-driver integration::` passes on both Linux and macOS.

---

### Phase 8: M1 verification sweep + tag `v0.1.0-m1`
**PR scope**: No new features. TODO/FIXME sweep across the repo. Quality-checklist verification with evidence. Confirm M1 surface explicit-non-goals list still holds (nothing crept in). Tag the milestone.
**Branch**: `chore/m1-verification`
**Flag**: N/A
**Est. lines**: ~50 (mostly grep results + minor cleanups + a CHANGELOG entry)
**Objective**: Repo is in a state where M1 can be tagged without regret.
**Why this phase exists**: Per Step 10 of `/plan` skill, every plan needs a verification sweep. The discipline matters more than the line count.
**Current-state anchors**: whatever the repo looks like after Phase 7.
**Files (expected scope)**:
- `CHANGELOG.md` (create — first M1 entry)
- Any code-comment TODOs migrated to `.claude/plans/active/v0_1-compiler.md` "Soon" list or to `.claude/todos.md` if cross-milestone.
**Deviation rule**: No new features. Cleanups only. Anything that requires non-trivial code = its own future PR.
**Steps**:
1. **Broad TODO sweep** (per `~/.claude/CLAUDE.md` Rule 6 + `no-duct-tape.md` Phrases-That-Trigger-Review):
   `rg -i 'TODO|FIXME|HACK|XXX|TEMP|PLACEHOLDER|acceptable for now|works in current state|fine until|we.?ll revisit|for now|good enough for the MVP|executor will figure' crates/`
   Migrate findings to the plan file (if M1+ work) or `.claude/todos.md` (if cross-milestone). Delete code comments. Zero results required to proceed.
2. Cross-check the M1 "explicitly NOT" list against actual code — confirm nothing slipped in (e.g., no `Let` token in the lexer, no `BinOp` node in the AST). The variant-count tests from P3/P4/P5 mechanically enforce this; this step is a sanity audit on top.
3. Re-evaluate Windows-platform support (per Risk table trigger) — even if no Windows contributor has appeared. Document the M8 status in the plan.
4. Run the full quality checklist below and check each item with evidence (file path, test name, or grep command + result).
5. Add `CHANGELOG.md` entry for M1.
6. Tag `v0.1.0-m1` after merge.
**Acceptance criteria**:
- [ ] Broad TODO sweep returns zero matches.
- [ ] M1 "explicitly NOT" list manually audited — no slips (variant-count tests confirm mechanically).
- [ ] Windows-support status re-evaluated and documented.
- [ ] All quality-checklist items below ticked with evidence.
- [ ] CHANGELOG entry committed.
- [ ] Git tag created.
**Verification**: The grep above returns empty. `git tag -l v0.1.0-m1` returns the tag.

---

## Quality Checklist (verify at completion of M1)

(Adapted from the /plan skill template — Yinz-specific. Rust crate version, not TS.)

- [ ] All compiler-emitted diagnostics use the three-part WHAT/WHAT-INSTEAD/WHY format (Diagnostic struct enforces this at construction)
- [ ] No banned-jargon strings in emitted diagnostics (enforced by P2's `tests/jargon_audit.rs` — automated grep over diagnostic call sites, fails CI on any banned word)
- [ ] No `unwrap()` outside test code anywhere in `crates/`
- [ ] No `panic!()` outside test code anywhere in `crates/` (panic should be impossible in a well-typed compiler path; if reached, it's a bug not a user error)
- [ ] All public APIs are documented with at least one-line `///` doc comments
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt --check` passes
- [ ] All snapshot tests have inline `// WHY:` comments stating the invariant they protect (per global testing rules — applies to Rust tests via `#[test]` and `insta`)
- [ ] CI passes on both Linux and macOS
- [ ] `ynz run hello.ynz` integration test green
- [ ] Salsa queries: every cross-stage call goes through a query, not direct function call (lex → parse → check → codegen all wired)
- [ ] LLVM context lifetimes documented at the codegen module level
- [ ] No dependency uses a `*` version constraint; `Cargo.lock` committed
- [ ] `Tests follow Rust conventions (cargo test, #[test], insta)` — testing.md from the global rules is TypeScript-specific (`bun:test`, `*.spec.ts`); not applicable to this Rust codebase. The PRINCIPLES still apply (WHY-comments on tests, test the contract not the implementation, never weaken a test to make it pass).

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each of the 8 phases is one PR with one branch. No phase mashes itself into "I'll split it later" — the branch name and est-lines are written down here, and each PR's scope is bounded by its phase block.
- **Shadow main branches**: every phase merges to `main` before the next starts. No long-lived `m1` umbrella branch. The integration risk of merging often (each PR may temporarily break the M1 end-to-end test until P7) is the smaller risk than the alternative.
- **Building the engine before shipping value**: M1 itself is the walking-skeleton answer to this anti-pattern. After M1, we have a real binary. After M2, we have variables. Every milestone produces a usable artifact, not a layer. **Honest disclosure**: P2 (diagnostics) is infrastructure ahead of value — it ships before any user-runnable output. We accept this because Golden Rule 11 (compiler is a teacher) is load-bearing for every later milestone; shipping a parser before diagnostics infrastructure is the duct-tape framing `no-duct-tape.md` prohibits. P2 isn't dressed up as value — it's explicitly infra-first, and we're calling it that.
- **Hotfix that isn't**: N/A — no production users. Will revisit when v0.1 actually ships and someone outside the team uses it.
- **Abandoned branches**: each phase is single-session-scoped. Branches that go stale (no merge within the session that opened them) get either merged or deleted at session end — they don't accumulate.
- **Flag graveyards**: N/A — the compiler doesn't use feature flags. `--release` is a user-facing build mode, not a feature flag. If we add an experimental flag for compiler-internal A/B (none planned), it gets a removal trigger documented at the time of introduction.

---

## Out-of-Scope For This Plan (do NOT slip these into M1)

(Explicit redundancy with the "What M1 explicitly is NOT" section above, restated here as a final guardrail.)

- Variables (`let`, `const`) — M2
- Arithmetic and operators — M2
- Numeric types (`int`, `float`, `number`, decimal128) — M2 / decimal128 design decision deferred
- User-defined functions beyond `main` — M3
- Control flow (`if`, `for`, `while`, early return) — M3
- User-defined types (`type Player { ... }`) — M4
- Ownership analysis — M4
- Generics, collections, monomorphization — M5
- Options, unions, maybe types, narrowing — M6
- Strings beyond ASCII byte arrays for codegen — M7
- The `errors` keyword and cascades — M7
- Iterables (`for x in iter`) — M7
- Modules (`import`, `export`) — M8
- Doc comment parsing — M8
- Sensitive type modifier — M8
- Concurrency keywords parsing — M8
- `ynz watch`, `ynz fmt`, LSP — v0.2 (separate plan)
- `ynz test`, the test runner — v0.13 (separate plan)

If you find yourself adding code that touches any item above, STOP and either re-plan this milestone or escalate the work to its proper milestone.

---

## Reviewer Disputes

(none yet — populated during Step 7 review iterations if/when planner pushes back)
