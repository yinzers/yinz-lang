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
last_updated: 2026-05-12-r4
---

# Plan: v0.1 Compiler Implementation

Created: 2026-05-12
Status: m2_planning

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
`let` / `const`, integer literals, float literals, binary arithmetic (`+ - * / %`), comparison (`< <= > >= == !=`), logical (`&& || !`), bitwise (`& | ^ ~ << >>`), local type inference, `bool` primitive type, polymorphic `print` over primitives, always-succeeds conversion intrinsics (`.toNumber`, `.toFloat`, `.toString`). Hand-rolled decimal128 (`number` = `number[34]` only) ships in a new `ynz-numerics` + `ynz-runtime` crate pair. `number[N]` for N > 34 (bignum) is syntactically reserved with a three-part deferral error pointing to M8. Mixed-type arithmetic is a compile error with a three-part diagnostic pointing at the relevant `.toX()` method.
**Flag**: N/A
**Status**: in planning (this milestone's detail is below)
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
`import` / `export`, root-relative paths, aliases with `as`, duplicate-name compile error. Doc comments (`///`) parsed and preserved on signatures. Sensitive type modifier (auto-redact in print output). Concurrency keywords (`wait`, `background`) parse and type-check, run sequentially. **Bignum `number[N]` for N ∈ (34, 4096]** — multi-u128 coefficient path with mixed-precision promotion + narrowing-warning rounding (per `design/numeric-types.md` lines 65–78). Polish + audit + v0.1.0 tag.
**Flag**: N/A
**Status**: planned
**Depends on**: M7
**Non-negotiable carry-from-M2**: `number[N]` for N > 34 is patrick's load-bearing v0.1 promise (exact decimal at any reasonable precision). M2 reserves the syntax and emits a three-part error pointing here. M8 must close the loop before v0.1.0 ships — if M8 has to drop something, it isn't this.

---

## Completed Milestone: M1 — Hello-world end-to-end

(Detail preserved below as historical context. See `## M1 Completion Summary` near the top of this file for the short version. Shipped at commit `820bfdc` with 51 tests green. Tagged `v0.1.0-m1` after merge per Phase 8.)

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

## Current Milestone: M2 — Literals + Variables + Arithmetic

**Scope**: `let` / `const` declarations with type annotations and local inference, integer / float / decimal literals, full operator set (arithmetic, comparison, boolean, bitwise) for `int` / `float` / `number[34]` / `bool`, polymorphic `print` over primitives, always-succeeds conversion intrinsics (`.toNumber`, `.toFloat`, `.toString`). Hand-rolled IEEE 754 decimal128 lives in a new `ynz-numerics` crate (Rust-internal) wrapped by a new `ynz-runtime` crate (C-ABI, builds to `libynz_rt.a`, linked into every generated binary).

**Branch convention**: each phase merges to `main` before the next starts. P1 opens on `feat/numerics-runtime`. Subsequent phases use their own `feat/m2-{lexer,parser,...}` branches. No `m2` umbrella branch (per M1's anti-pattern callouts).

**Headline integration test (M2 contract)**:
```yinz
function main() -> nothing {
  let price = 0.1 + 0.2          // number, exact 0.3
  let count: int = 42
  let active = true
  print(price)                    // 0.3
  print(count * count - 1)        // 1763
  print(active && (count > 0))    // true
}
```
This file compiles, runs, and prints exactly `0.3\n1763\ntrue\n` on both Linux and macOS. Captured as `crates/ynz-driver/tests/fixtures/m2_smoke.ynz`.

### What M2 explicitly is NOT (deferred to later milestones, with explicit owners)

- **Bignum `number[N]` for N > 34** — syntactically reserved in M2; compile error at typeck pointing to M8. **M8 carries the implementation. Tracked above in the M8 roadmap entry.**
- **Overflow escape valves: `.wrappingAdd()`, `.wrappingSub()`, `.wrappingMul()`, `.saturatingAdd()`, `.saturatingSub()`, `.saturatingMul()`** — int overflow panics; the escape methods don't exist yet. **M4 (types + methods) carries this** — recorded in M4's Out-of-Scope-Catch-Up list below.
- **Type-attached constants: `int.max`, `int.min`, `number.max`, `number.epsilon`, etc.** — module-style constants on a primitive type require either a primitive-namespace lookup or full type-method dispatch. **M4 carries this** — see Catch-Up list.
- **Fallible conversions: `.toInt()` on number/float, `string.toInt()` / `string.toNumber()` / `string.toFloat()`** — return `maybe T` which doesn't exist until M6. **M6 carries this** — see Catch-Up list.
- **User-defined types, methods, ownership** — M4.
- **Generics, collections, monomorphization** — M5.
- **Control flow** (`if`, `for`, `while`, early return) — M3. M2's smoke test is straight-line code.
- **User-defined functions beyond `main`** — M3.
- **`maybe`, `options`, unions, narrowing** — M6.
- **Strings beyond ASCII byte arrays** — full Unicode + interpolation lands in M7. M2 strings stay as M1 did: UTF-8 byte sequences passed to `puts`. `string.toString()` is identity; no interpolation, no concat.
- **Compound assignment / increment** — explicitly banned by `spec/operators.md`. Plan keeps it banned. Parser-level test asserts that `x += 1` produces a three-part diagnostic suggesting `x = x + 1`.
- **Ternary `?:`** — explicitly banned by `design/type-conversion.md`. Same shape of three-part error.

**Catch-Up list for downstream milestones** (recorded here so they aren't lost when M2 ships):
- **M4 must catch up**: overflow escape methods on `int`; `int.max` / `int.min` constants; `number.max` / `number.epsilon`; rewire M2's intrinsic-table dispatch to general method dispatch.
- **M6 must catch up**: `.toInt()` on number/float (returns `maybe int`); `string.toInt()` / `string.toNumber()` / `string.toFloat()` (return `maybe T`); compile-error suggestions for mixed-type arithmetic involving these fallible directions.
- **M8 must catch up**: bignum `number[N]` for N ∈ (34, 4096]; remove the M2 deferral compile error; finalise the IEEE-754-conformance test sweep on the bignum path.

If a phase below feels like it's drifting into any of the above, STOP and re-plan.

### Spec corrections required as part of M2

Two inconsistencies discovered during M2 planning. Both must be fixed in the same PR as the relevant phase, with a `// WHY:` style note in the commit message (no separate "fix typo" PR — keep the spec correction welded to the code that implements the canonical answer).

| Inconsistency | Canonical decision | Fix in phase |
|---|---|---|
| `spec/variables.md:48` says `let x = 42 // compiler knows: number`; `spec/numeric-types.md:206` says `let x = 42 // inferred as int`. | **`let x = 42` infers as `int`** per `spec/numeric-types.md` and Golden Rule 10 ("efficiency first, dynamic after — default = most performant"). `spec/variables.md` is wrong. | P4 (typeck) — typeck implementation enforces int; same PR edits `spec/variables.md:48` to read `// compiler knows: int`. |
| `spec/numeric-types.md:211` says "Mixed-type expressions promote to the most capable type in the expression." | **Mixed-type expressions are a compile error** with a three-part diagnostic pointing at the relevant `.toX()` method. No implicit numeric coercion, per `design/type-conversion.md` and patrick's M2-planning decision. | P4 (typeck) — same PR edits `spec/numeric-types.md:211` to describe the compile-error behavior, including the example three-part diagnostic. |

### Pre-Phase-1 decisions locked (Sonnet review 2026-05-12)

- **`FloatLit(f64)` removed from Token + AST**: The plan listed `FloatLit(f64)` as a Token variant and `Expr::FloatLit(f64)` as an AST node, but the same plan says "there is no `float` literal form." These were contradictory. Decision: `FloatLit` does NOT exist as a Token or AST node. All decimal literals (including `1.0`) produce `NumberLit(String)` at lex time. The `float` type is set by typeck when it sees a `: float` annotation — typeck records this in `expr_types`, following M1's pattern. Phase 2 and Phase 3 token/AST variant lists updated accordingly.
- **`PrimitiveIntrinsicTable` REPLACES `BuiltinTable`**: M2 does not add alongside M1's `BuiltinTable`. Phase 4 removes `BuiltinTable` and replaces it with `PrimitiveIntrinsicTable`. `print` becomes polymorphic over all primitive types.
- **`int.toString()` memory model = thread-local static buffer**: `@ynz_int_to_string` in the runtime uses a thread-local `[u8; 22]` buffer (64-bit int = max 20 digits + sign + null). Safe for M2's single-threaded programs. Same pattern: `float.toString()` uses `[u8; 32]`, `number.toString()` uses `[u8; 48]`. Documented in `ynz-runtime/src/lib.rs` with an explicit safety comment.
- **`libynz_rt.a` path mechanism = `build.rs` + `cargo:rustc-env`**: `crates/ynz-driver/build.rs` emits `cargo:rustc-env=YNZ_RT_LIB_DIR=<target/{profile}/>` and `cargo:rustc-env=YNZ_RT_LIB_NAME=ynz_rt`. Driver reads these at compile time via `env!()` and passes `-L`/`-l` to `cc`. `cargo build --workspace` handles transitivity.

### Architectural decisions locked at M2 planning

- **Two runtime crates: `ynz-numerics` (pure Rust, internal-use decimal128) + `ynz-runtime` (umbrella, C-ABI, `staticlib` target).** `ynz-numerics` is a normal Rust dep usable by the compiler (constant folding, test conformance). `ynz-runtime` re-exports it via `#[no_mangle] extern "C"` shims and builds to `libynz_rt.a`. Generated binaries link against `libynz_rt.a`; the compiler uses `ynz-numerics` directly. **Why two crates, not one with dual `crate-type = ["staticlib", "rlib"]`:** the C-ABI shim layer needs panic-handler glue and (eventually) heap-allocator glue that has no place in the pure decimal implementation. Splitting keeps `ynz-numerics` test-pure and lets `ynz-runtime` grow without polluting it. Single-crate dual-output was rejected as duct-tape framing — the shim layer is real work, not a cargo flag.
- **Decimal128 in M2 = IEEE 754 decimal128 core operations only.** Add, subtract, multiply, divide, negate, abs, compare. Quantize / rescale / round (used internally for arithmetic) are implementation details, not user surface. **No** sqrt, ln, exp, sin, fma, rem — these belong in the `math` module (v0.7). **No** `%` modulo on `number` — `number % number` is a compile error in M2 pointing at "the math module will provide `.rem()` in v0.7." Integer `%` and float `%` work normally (LLVM `srem` / `frem`).
- **Codegen calls into runtime via plain C-ABI extern fns.** `extern "C" ynz_decimal_add(*const Decimal128Bits, *const Decimal128Bits, *mut Decimal128Bits)` — pass-by-pointer to avoid LLVM struct-ABI traps. `Decimal128Bits` is `[u8; 16]` (16 bytes = 128 bits) at the ABI boundary; Rust side decodes to its internal representation. This matches the IEEE 754 decimal128 storage format (BID — binary integer significand) so the bits CAN round-trip through the runtime without conversion.
- **Integer overflow check codegen = LLVM checked-arithmetic intrinsics.** `llvm.sadd.with.overflow.i64`, `llvm.ssub.with.overflow.i64`, `llvm.smul.with.overflow.i64`. Each op produces `{i64, i1}`; the `i1` flag branches to a runtime panic stub on true. No escape valve in M2 (per the deferral above) — every `int` arithmetic op gets the check. Performance hit on the overflow check path is acceptable for v0.1; LLVM's instcombine collapses the check on constant operands.
- **Decimal-by-zero, int-by-zero**: runtime panic (three-part, source-spanned, "use `if (denom != 0)` to guard division" suggestion). **Float-by-zero**: returns IEEE infinity per binary64 spec (no panic — this is correct IEEE behavior and `float` users opt into it).
- **Operator `%` (modulo)**: M2 ships `%` for `int` (LLVM `srem`) and `float` (LLVM `frem`, returns NaN on zero divisor per IEEE). `number % number` is a compile error pointing at v0.7 `math` module. **Spec update required**: add `%` to `spec/operators.md` arithmetic list — currently missing.
- **Number literal forms**: M2 lexer accepts decimal (`42`, `3.14`, `1e5`, `2.5e-3`), hex integer (`0x2A`), binary integer (`0b1010`), and underscores for readability (`1_000_000`, `0xDEAD_BEEF`). **No** octal — too rarely useful, the form is a footgun (leading-zero ambiguity). Fractional hex/binary not supported.
- **Integer division**: truncates toward zero (matches LLVM `sdiv`, Rust, C, Go). `5 / 2 == 2`, `-5 / 2 == -2`. No surprise.
- **Mixed-precision `number[N]` arithmetic in M2**: not yet applicable since N is locked to 34. The promotion/narrowing logic is M8's problem. M2's typeck rejects `number[N]` for `N != 34` at parse-binding time with the deferral diagnostic.

- **Runtime to-string ABI = caller-owned buffer, no heap.** All formatting runtime functions take a caller-allocated buffer pointer and write into it; nothing in M2 returns an owned pointer. The "no heap allocation in M2" rule and the to-string return-type are reconciled by making the *caller* allocate the buffer on its stack via `alloca`. Same shape as the decimal binops (`*const, *const, *mut`). Avoids thread-local-buffer footguns (pointer-valid-until-next-call) and survives the v0.3 concurrency landing without rework. Sizes are locked here so codegen and the runtime agree:
  - `ynz_int_to_string(i64 %x, ptr %out_buf, i64 %buf_len)` — `%buf_len` must be ≥ 24 (`int.min` is `-9223372036854775808` = 20 chars + null + slack). Runtime asserts on overflow (panic with three-part diagnostic — the assertion is paranoia since codegen always allocates 24).
  - `ynz_float_to_string(double %x, ptr %out_buf, i64 %buf_len)` — buf_len ≥ 32. Holds any IEEE 754 binary64 formatted form including `±inf`, `nan`, and `±d.dddddddddddddddddE±NNN`.
  - `ynz_decimal_to_string(ptr %x, ptr %out_buf, i64 %buf_len)` — buf_len ≥ 48. Holds sign + 34 digits + decimal point + exponent.
  - All three return `i64` = number of bytes written, NOT including the null terminator. Codegen ignores the return for `print` (always passes to `puts` which finds the null on its own); future stdlib `string` work can use it.
  - Codegen pattern (locked here for P5 reference): `%buf = alloca [24 x i8]; %n = call i64 @ynz_int_to_string(i64 %x, ptr %buf, i64 24); call i32 @puts(ptr %buf)`.

- **`libynz_rt.a` discovery at link time = `build.rs` + `cargo:rustc-env`.** `crates/ynz-driver/build.rs` runs at compile time, walks up from `OUT_DIR` to find the workspace `target/` directory, joins `{target_dir}/{profile}/`, and emits `cargo:rustc-env=YNZ_RT_LIB_DIR={path}`. The driver at runtime reads `env!("YNZ_RT_LIB_DIR")` and passes it to `cc` as `-L{path} -lynz_rt`. **Why this and not alternatives:**
  - Hardcoding `../target/debug/libynz_rt.a` rejected: breaks when `CARGO_TARGET_DIR` is set OR when the driver is invoked from outside the workspace OR in release builds.
  - Cargo `--message-format=json` discovery rejected: requires re-invoking cargo at compile time; chicken-and-egg with the build it's supposedly informing.
  - `env!("CARGO_MANIFEST_DIR")` + relative path rejected: ties path resolution to the source tree, breaks for installed binaries.
  - `cargo:rustc-env` is the documented mechanism for build-script → compiler-env data flow. Standard Rust pattern. Survives `cargo install` (the env var becomes baked into the binary at install time — which is correct, the install target dir is where `libynz_rt.a` ended up).
  - `cargo:rerun-if-changed=build.rs` line included so build.rs doesn't re-run on every build.
  - Fallback for `ynz install`-style installs (not in v0.1; documented for later): when the driver can't find `libynz_rt.a` at the baked-in path, emit a three-part diagnostic. This is a v0.5 (package manager) concern; M2 just bakes the workspace path.

---

### Phase 1: `ynz-numerics` + `ynz-runtime` crates + decimal128 implementation
**PR scope**: Two new crates. `ynz-numerics` implements IEEE 754 decimal128 (BID encoding, u128-backed coefficient) with `add`/`sub`/`mul`/`div`/`neg`/`abs`/`compare`. `ynz-runtime` re-exports them as `#[no_mangle] extern "C"` shims and builds to `libynz_rt.a`. Compiler driver gains a `--link-runtime` path (always-on in M2). No language-surface changes yet — phase ends with a runtime library that nothing in the compiler calls.
**Branch**: `feat/numerics-runtime`
**Flag**: N/A
**Est. lines**: ~2500 (decimal128 is the bulk; conformance test harness ~800)
**Status**: COMPLETE (2026-05-12) — commit 59fcee2, 118 tests green, PR open at https://github.com/patrickrizzardi/ynz/pull/new/feat/numerics-runtime (awaiting `gh auth login` for CLI creation). Key correctness fixes: `round_half_even` using `2*r vs divisor`, alignment threshold `aligned_digits > 68`, single-signal division rounding `(d35*b+r) vs (5*b)`. Big-O docs added: `U256::div_rem` O(256) binary long division, Knuth Algorithm D replacement target at v0.4.
**Objective**: `ynz-numerics` passes the IEEE 754-2008 decimal128 conformance test vectors AND a differential test against Python `decimal` on 10k random `(a, b, op)` tuples. `ynz-runtime` builds to a static archive on Linux + macOS. The driver's link step finds and includes the archive.
**Why this phase exists**: Decimal128 correctness is the load-bearing v0.1 promise. Shipping it as a standalone, separately-testable crate with a conformance gate eliminates the "we'll validate it once it's wired up" duct-tape pattern. If `ynz-numerics` doesn't pass IEEE 754 conformance, nothing downstream matters.

**Current-state anchors**:
- `design/numeric-types.md` (Implementation: Handwritten, Not Crates section).
- IEEE 754-2008 spec, section 5 (decimal arithmetic) — primary reference.
- Intel BID reference implementation (informational, NOT a dependency) — useful for cross-checking edge cases.

**Files (expected scope)**:
- `crates/ynz-numerics/Cargo.toml` + `crates/ynz-numerics/src/lib.rs`
- `crates/ynz-numerics/src/decimal128/bits.rs` (BID encoding: sign, combination field, exponent, coefficient — bit layout per IEEE 754)
- `crates/ynz-numerics/src/decimal128/ops.rs` (add, sub, mul, div, neg, abs, compare; rounding helpers; subnormal/special-value handling for ±0, ±Infinity, NaN, sNaN)
- `crates/ynz-numerics/src/decimal128/parse.rs` (string → decimal128, exact)
- `crates/ynz-numerics/src/decimal128/format.rs` (decimal128 → string, round-trip safe)
- `crates/ynz-numerics/tests/conformance/` (IEEE 754-2008 test vector loader + assertions)
- `crates/ynz-numerics/tests/differential.rs` (`proptest` against Python `decimal` via subprocess — 10k random tuples per CI run)
- `crates/ynz-numerics/tests/properties.rs` (commutativity, associativity-where-applicable, distributivity, round-trip identity)
- `crates/ynz-runtime/Cargo.toml` (`crate-type = ["staticlib"]`, depends on `ynz-numerics`)
- `crates/ynz-runtime/src/lib.rs` (`#[no_mangle] extern "C"` shims; `ynz_panic_overflow`, `ynz_panic_div_by_zero` stubs that write a three-part message to stderr and `abort(3)`)
- `crates/ynz-runtime/src/decimal_shims.rs` (`ynz_decimal_add(*const, *const, *mut)`, etc.)
- `crates/ynz-runtime/src/format_shims.rs` (the three caller-buffer to-string shims: `ynz_int_to_string`, `ynz_float_to_string`, `ynz_decimal_to_string` — signatures per the buffer-ABI decision above)
- `crates/ynz-driver/build.rs` (NEW — walks from `OUT_DIR` up to workspace `target/`, joins `PROFILE`, emits `cargo:rustc-env=YNZ_RT_LIB_DIR={path}` plus `cargo:rerun-if-changed=build.rs`)
- `crates/ynz-driver/src/build.rs` (read `env!("YNZ_RT_LIB_DIR")` at runtime; pass `-L{dir} -lynz_rt` to `cc`)

**Deviation rule**: No language-surface changes in this phase. No new tokens, no new AST nodes, no new typeck rules. If something downstream needs a hook, document it for the next phase; don't reach into the compiler from here.

**IEEE 754 conformance — concrete sourcing**:
- Test vectors: use the IBM Hursley decimal-test corpus (publicly-licensed, ships as `.decTest` files). Pin a specific corpus tarball SHA-256 in `crates/ynz-numerics/tests/conformance/CORPUS.sha256` so a future corpus update is a deliberate, reviewed change. Loader parses `.decTest` syntax into `(operands, expected, rounding_mode)` triples.
- Conformance subset for M2: `dqAdd`, `dqSubtract`, `dqMultiply`, `dqDivide`, `dqCompare`, `dqAbs`, `dqMinus`, `dqPlus`. The `dq` prefix is the corpus's name for "decimal quad" = 128-bit. Loader skips test cases tagged for ops we don't ship in M2 (sqrt, fma, exp, ln, etc.) — explicitly logged as skipped, not silently ignored.
- Rounding modes: M2 implements round-half-even (IEEE 754 default) only. Other modes (half-up, half-down, ceiling, floor, half-toward-zero) skipped with `// FUTURE: M8 polish or math-module work`.

**Differential test**:
- `proptest` generates pairs of decimal128 values (uniform random over the representable range, plus targeted edge cases: ±0, near-infinity, near-subnormal, denormalized inputs).
- For each `(a, b, op)`, compute `result_ynz` via our implementation AND `result_python` by shelling out to `python3 -c "from decimal import *; getcontext().prec=34; getcontext().rounding=ROUND_HALF_EVEN; print(...)"`.
- Assert byte-for-byte equality on the formatted output. Mismatch = test fails, prints both sides, fails CI.
- Test runs 10k iterations per CI invocation. Seed deterministic per CI run; surface the seed in the failure message so failures reproduce.
- Skipped on the CI runner if Python ≥ 3.8 isn't available (CI installs it; the skip path is for contributor laptops without Python — log loud, don't silently pass).

**Property tests**:
- Commutativity: `a + b == b + a` and `a * b == b * a` for all finite `a`, `b`.
- Identity: `a + 0 == a`, `a * 1 == a`, `a / 1 == a`, `a - a == 0`, `a / a == 1` (when `a != 0`).
- Sign: `a - b == -(b - a)`, `abs(a) >= 0`, `neg(neg(a)) == a`.
- Round-trip: `format(parse(s)) == s` for any well-formed decimal literal that fits in 34 digits.
- Distributivity is NOT a strict property in IEEE 754 decimal arithmetic (rounding intervenes); the test asserts the weaker "close to within one ULP" form and is documented as such.

**Steps**:
1. Scaffold the two crates per the files list. Add them to the workspace `Cargo.toml`. CI builds them as part of `cargo build --workspace`.
2. Implement `Decimal128 { bits: u128 }` with BID encoding helpers in `bits.rs`. Includes packed-decimal coefficient decode (per IEEE 754 §3.5.2). Test via known-value snapshots: `0.0`, `1.0`, `0.1`, `-1.5`, `Infinity`, `NaN`.
3. Implement `add`/`sub` first (sub = add with sign flip). Run conformance corpus subset for `dqAdd`/`dqSubtract` continuously while developing.
4. Implement `mul`/`div`. Division is the hardest — Newton-Raphson on the reciprocal vs. long-division loop is the strategy choice. Pick long-division (simpler, easier to validate) and revisit if performance is a problem at v0.4 lint pass.
5. Implement `compare`, `abs`, `neg`.
6. Implement `parse` (string → Decimal128) and `format` (Decimal128 → string). These are the same algorithms IEEE 754 specifies for conversion. Round-trip property test gates correctness.
7. Hook up the conformance corpus loader. Run `cargo test --release -p ynz-numerics conformance::` — must pass clean on Linux and macOS.
8. Hook up the differential test. Run 10k iterations on CI; pass.
9. Scaffold `ynz-runtime`. Implement `#[no_mangle] extern "C" ynz_decimal_add` and friends. Each is ~3 lines: unsafe-deref the input pointers, call into `ynz-numerics`, write the result via the output pointer.
10. Implement the three caller-buffer to-string shims per the locked ABI: `ynz_int_to_string`, `ynz_float_to_string`, `ynz_decimal_to_string`. Each writes formatted bytes + null terminator into the caller's buffer; returns the byte count (excluding the null). Each asserts `buf_len >= MIN_FOR_THIS_TYPE` and panics with a three-part diagnostic on undersize (paranoia — codegen always allocates correctly, but the assertion is cheap and catches future ABI drift).
11. `cargo build -p ynz-runtime --release` produces `target/release/libynz_rt.a`. Verify with `nm`: contains all six decimal shims AND all three format shims.
12. Add `crates/ynz-driver/build.rs` per the locked discovery mechanism. Verify `cargo build -p ynz-driver` succeeds and `YNZ_RT_LIB_DIR` is set to the expected workspace `target/{profile}/` path (assert via a unit test that reads `env!("YNZ_RT_LIB_DIR")` and checks it ends with `/debug` or `/release` and contains `libynz_rt.a`).
13. Update `ynz-driver`'s `build` flow to consume `env!("YNZ_RT_LIB_DIR")` and pass `-L{dir} -lynz_rt` to `cc`. Existing M1 `hello.ynz` integration test still links and runs (M2 hasn't touched the language surface, so M1's output is unchanged — the runtime is linked but no symbols from it are referenced yet).
14. WHY-comments on every conformance + differential + property test stating what invariant is protected.

**Acceptance criteria**:
- [ ] `cargo test -p ynz-numerics` passes 100% of the M2-subset IEEE 754 corpus.
- [ ] Differential test runs 10k tuples against Python `decimal` and asserts bit-identical output.
- [ ] All property tests (commutativity, identity, sign, round-trip) pass.
- [ ] `cargo build -p ynz-runtime --release` produces `libynz_rt.a` containing all six C-ABI shims.
- [ ] M1's `ynz run hello.ynz` integration test still passes (driver links `libynz_rt.a` but doesn't call into it yet).
- [ ] `crates/ynz-driver/build.rs` resolves `YNZ_RT_LIB_DIR` to the workspace `target/{profile}/` directory; verified by a unit test reading `env!("YNZ_RT_LIB_DIR")`.
- [ ] All three format shims accept caller-owned buffers per the locked ABI (`ptr %out_buf, i64 %buf_len`), write formatted bytes + null terminator, return byte count, panic three-part on undersize buffer.
- [ ] The corpus tarball SHA-256 is pinned in `CORPUS.sha256`; updating it requires a deliberate edit with a justification commit message.
- [ ] All `unsafe` blocks in `ynz-runtime` carry a `// SAFETY:` comment explaining the pointer invariants the caller (codegen) must uphold.

**Quality gate**:
- [ ] No `unwrap()` outside test code in either crate.
- [ ] `tests/jargon_audit.rs` (from M1 P2) stays green — `ynz-numerics` panic messages and `ynz-runtime` shim error messages don't contain banned jargon.
- [ ] No external crates pulled in for decimal arithmetic itself (per the no-crates session decision). Acceptable deps: `proptest` (dev-only), corpus-file parsing utilities (no actual decimal math).
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] `cc` invocation in the driver uses `Command::new("cc").arg(...)` per M1's P7 quoting discipline; no shell strings.

**Verification**: `cargo test --workspace` passes. `nm target/release/libynz_rt.a | grep ynz_decimal_add` returns a defined symbol. `./target/debug/ynz run crates/ynz-driver/tests/fixtures/hello.ynz` still prints `hello, yinz`.

---

### Phase 2: Lexer extension (M2 token set)
**PR scope**: Extend `ynz-parser::lex` to recognize the M2 token set: `let`, `const`, `true`, `false` keywords; integer / float / decimal literals (with hex / binary / scientific / underscore-separator forms); arithmetic / comparison / boolean / bitwise operators; `=` and `:` punctuation; type-position identifiers (handled later — lexer just produces `Identifier`).
**Branch**: `feat/m2-lexer`
**Flag**: N/A
**Est. lines**: ~400
**Objective**: Lexing a representative M2 source produces the expected token stream (snapshot-asserted). Malformed literals (`1.2.3`, `0xZZ`, `0b22`, `1__000` with adjacent underscores) produce three-part diagnostics; lexer continues.

**Spec correction landing in this phase**: add `%` to the arithmetic operator list in `spec/operators.md` (currently missing). Same PR.

**Current-state anchors**:
- `spec/variables.md`, `spec/numeric-types.md`, `spec/operators.md` (with the `%` addition above).
- M1 P3 lexer code — the byte-iterator pattern, span tracking, salsa query plumbing all extend, not get rewritten.

**Files (expected scope)**:
- `crates/ynz-parser/src/token.rs` (extend `Token` enum)
- `crates/ynz-parser/src/lexer.rs` (extend lexer logic)
- `crates/ynz-parser/tests/lex.rs` + snapshots
- `spec/operators.md` (add `%` to arithmetic list)

**New `Token` variants (M2)**:
- Keywords: `Let`, `Const`, `True`, `False`
- Literals: `IntLit(i64)`, `FloatLit(f64)`, `NumberLit(String)` — the string form is INTENTIONAL: parser does the lossless decimal128 decoding via `ynz-numerics::parse` in P3, the lexer just preserves the source bytes.
- Operators: `Plus`, `Minus`, `Star`, `Slash`, `Percent`, `EqEq`, `NotEq`, `Lt`, `LtEq`, `Gt`, `GtEq`, `AmpAmp`, `PipePipe`, `Bang`, `Amp`, `Pipe`, `Caret`, `Tilde`, `LtLt`, `GtGt`
- Punctuation: `Eq` (assignment), `Colon`

**Deviation rule**: Only the variants above. No `Plus_eq` (compound assignment is banned), no `PlusPlus` (increment is banned), no `Question` (no ternary). Variant-count test gets bumped from M1's count to the new M2 count with an explicit `// test-ratchet: M2 adds {N} for literals + operators` marker.

**Decimal-vs-int literal classification rule (locked here)**:
- A numeric literal containing `.` or `e`/`E` is a **number** literal → `NumberLit(String)`.
- A numeric literal with no `.` and no exponent is an **int** literal → `IntLit(i64)`. Overflow at parse time = three-part diagnostic ("literal does not fit in int / use number for values beyond ±9.2e18").
- A hex / binary literal is **always int** → `IntLit(i64)`. No `0x1.fp3` style hex floats (deferred indefinitely; spec doesn't promise them).
- `42.0` is a **number** literal, not an int — the `.0` distinguishes intent. **There is no `float` literal form.** `float` values come from explicit annotation (`let x: float = 1.0`) — the lexer produces `NumberLit("1.0")` and typeck retypes it under the annotation. Documented in `spec/numeric-types.md` Type inference section; same PR adds the explanation.

**Steps**:
1. Extend `Token` enum with the variants above. Update the variant-count test from M1's count → M2's count; add the `// test-ratchet:` marker per the rule.
2. Extend lexer: add keyword recognition (compare identifiers against the keyword set after lex), number-literal recognition (scan digits + `_` + optional `.` digits + optional `e[+-]digits`; hex `0x[0-9A-Fa-f_]+`; binary `0b[01_]+`).
3. Punctuation/operator recognition: add the two-char operators (`==`, `!=`, `<=`, `>=`, `&&`, `||`, `<<`, `>>`) before the single-char fallbacks. The order matters — `==` must beat `=`, `<=` must beat `<`.
4. Reject `1.2.3` (two dots), `1__0` (adjacent underscores), `_1` (leading underscore), `1_` (trailing underscore), `0xZZ` (non-hex char in hex literal), `0b22` (non-binary char in binary literal). Each emits a three-part diagnostic; lexer recovers to next whitespace.
5. Recovery from compound-assignment / increment attempts: when lexer sees `+=`, `++`, `-=`, `--`, `*=`, `/=`, `%=` — emit a three-part diagnostic ("compound assignment / increment is not supported in Yinz / use `x = x + n`") and continue past the `=` or second char. **The diagnostic message is the user-facing surface for the banned feature — it must teach.**
6. Update snapshot tests: add M2 source token-stream snapshots. Add negative snapshots for the malformed literals + banned-operator cases. Each test gets a `// WHY:` comment.
7. Same PR: `spec/operators.md` — add `%` to the arithmetic list (line ~9). One-line spec edit, no rationale needed (modulo is universal in arithmetic).

**Acceptance criteria**:
- [ ] M2 source token-stream snapshot matches.
- [ ] All seven malformed-literal cases produce three-part diagnostics with correct spans.
- [ ] All seven banned-operator cases (`+=`, `-=`, `*=`, `/=`, `%=`, `++`, `--`) produce three-part teaching diagnostics.
- [ ] Hex / binary / scientific / underscore-separator forms all lex to `IntLit` / `NumberLit` as the rule specifies.
- [ ] `m2_token_variant_count_locked` test pins the new count with the `// test-ratchet:` marker.
- [ ] `spec/operators.md` mentions `%`.

**Quality gate**:
- [ ] No `unwrap()` in lexer changes.
- [ ] `tests/jargon_audit.rs` stays green on the new diagnostics.
- [ ] No regression on M1's lexer tests.

**Verification**: `cargo test -p ynz-parser lex::` passes.

---

### Phase 3: AST + parser extension (M2 surface)
**PR scope**: Extend `ynz-ast::nodes` with `Stmt::Let { mutability, name, ty, value }`, `Stmt::Assign { target, value }`, `Expr::IntLit(i64)`, `Expr::FloatLit(f64)`, `Expr::NumberLit(String)`, `Expr::BoolLit(bool)`, `Expr::BinOp { op, lhs, rhs }`, `Expr::UnaryOp { op, expr }`, `Expr::MethodCall { receiver, method, args }`. Add `Type::Int`, `Type::Float`, `Type::Number { precision: u32 }`, `Type::Bool` to the type-position parser. Implement Pratt-style precedence climbing for binary operators per `spec/operators.md` precedence table.
**Branch**: `feat/m2-parser`
**Flag**: N/A
**Est. lines**: ~700
**Objective**: M2 source parses to the snapshot AST with zero diagnostics. Malformed expressions (`let x = 1 +`, `let : int = 5`, `let x: int = 1.5`, `let x = }`, `print(1, 2, 3)`) produce three-part diagnostics; parser recovers per the strategy from M1 P4.

**Current-state anchors**:
- M1's parser style: recursive descent with explicit recovery to next `}` or EOF.
- `spec/operators.md` (with the `%` addition from P2) — precedence table is canonical.
- `spec/variables.md` — `let` / `const` shape.

**Files (expected scope)**:
- `crates/ynz-ast/src/nodes.rs` (extend `Stmt`, `Expr`, `Type`)
- `crates/ynz-parser/src/parser.rs` (add precedence climber + statement parser)
- `crates/ynz-parser/tests/parse.rs` + snapshots

**New AST variants (M2)**:
- `Stmt::Let { is_const: bool, name: Spanned<Ident>, ty: Option<Spanned<Type>>, value: Spanned<Expr> }` — single statement covers `let` and `const`; the `is_const` field controls mutability check in typeck.
- `Stmt::Assign { target: Spanned<Ident>, value: Spanned<Expr> }` — reassignment.
- `Expr::IntLit(i64)`, `Expr::FloatLit(f64)`, `Expr::NumberLit(String)`, `Expr::BoolLit(bool)`.
- `Expr::BinOp { op: BinOpKind, lhs: Box<Spanned<Expr>>, rhs: Box<Spanned<Expr>> }` with `BinOpKind` covering all arithmetic / comparison / boolean / bitwise operators.
- `Expr::UnaryOp { op: UnaryOpKind, expr: Box<Spanned<Expr>> }` with `UnaryOpKind { Neg, Not, BitNot }`.
- `Expr::MethodCall { receiver: Box<Spanned<Expr>>, method: Spanned<Ident>, args: Vec<Spanned<Expr>> }` — `.toString()`, `.toNumber()`, etc.
- `Type::Int`, `Type::Float`, `Type::Number { precision: u32 }`, `Type::Bool`.

**Deviation rule**: Only the variants above. **NO** `Expr::Index` (collections — M5), **NO** `Expr::FieldAccess` (user types — M4), **NO** `Stmt::If` (control flow — M3), **NO** `Stmt::Return` (control flow — M3). Variant-count test gets bumped with `// test-ratchet: M2 adds {N} for variables + arithmetic`.

**Precedence-climbing strategy (locked here)**:
- Pratt-style precedence climber. Each token has a left binding power; the parser recursively gathers operands as long as the next operator's left BP exceeds the current minimum.
- Precedence table is hardcoded against `spec/operators.md` precedence list (PEMDAS extended). Test-ratchet: a dedicated `parser_precedence_table_matches_spec` test parses the spec table at runtime (markdown table → array of (op, level)) and asserts it matches the hardcoded table. Drift between code and spec fails the test.
- Unary operators (`-`, `!`, `~`) are right-associative prefix; binding power is `12` (higher than any binary op, lower than method-call dot).
- Method-call `.` and call-parens `(...)` are left-associative postfix at binding power `13`.

**Steps**:
1. Extend AST nodes. Update variant-count tests for `Stmt`, `Expr`, `Type` with `// test-ratchet:` markers per M2.
2. Add `parse_let_or_const` (top-level decision based on first token). Both desugar to `Stmt::Let { is_const, ... }`.
3. Add `parse_assign_or_expr_stmt` — peek for `Identifier =` shape; if so, parse `Stmt::Assign`; else fall through to expression statement.
4. Add `parse_expr` as Pratt climber (`parse_expr(min_bp: u8)`). Calls `parse_atom`, then loops over postfix / binary operators by BP.
5. `parse_atom` handles: literal (int/float/number/bool/string), parenthesized expr, identifier (then possible postfix chain), unary-prefix expr.
6. `parse_postfix_chain` handles `.method(args)` and `(args)` (the latter is `print(...)` and a few other reserved-name calls — M1's `Expr::Call` is reused, just exposed via the postfix chain now).
7. `parse_type` recognizes type-position identifiers: `int`, `float`, `number`, `number[N]` (with `N` parsed as an int-literal bracketed token), `bool`, `string`, `nothing`. Reject unknown type identifiers with a three-part diagnostic ("unknown type / did you mean: <closest valid type via Levenshtein>"). Levenshtein-suggestion is a polish nicety — if it adds complexity, drop to just listing the M2 type set.
8. Negative parse tests (each with `// WHY:` comments):
   - `let x = 1 +` — incomplete RHS; parser emits a three-part diagnostic, inserts `Expr::Error` as the RHS, recovers to next statement.
   - `let : int = 5` — missing identifier; parser emits a three-part diagnostic.
   - `let x: int = 1.5` — parses fine (type-checking the literal type happens in P4); produces an AST with `Type::Int` annotation + `NumberLit("1.5")` value.
   - `let x = }` — unexpected close-brace where atom expected; parser emits a three-part diagnostic, recovers.
   - `print(1, 2, 3)` — M2 print is single-arg; **but** the parser doesn't enforce arity (typeck does). So this parses fine; the arity error fires in P4.
   - `let x = 1 < 2 < 3` — comparison chaining. Parses as `(1 < 2) < 3` per left-associativity. Typeck rejects in P4 (`bool < int` is a type error).
9. WHY-comments per testing rules.

**Acceptance criteria**:
- [ ] M2 representative source parses to the snapshot AST.
- [ ] All five negative cases produce three-part diagnostics with correct spans and a non-`None` recovered AST.
- [ ] Precedence-climber tests cover every operator pair at the spec's precedence boundary (`1 + 2 * 3` → `1 + (2 * 3)`; `1 < 2 && 3 > 4` → `(1 < 2) && (3 > 4)`; etc.).
- [ ] `parser_precedence_table_matches_spec` test passes (binds the hardcoded table to the spec table).
- [ ] Variant-count tests pin the M2 counts with `// test-ratchet:` markers.
- [ ] M1 parser tests still pass.

**Quality gate**:
- [ ] No `unwrap()` in parser changes.
- [ ] `tests/jargon_audit.rs` green on new diagnostic strings.
- [ ] Recovery strategy comment at the top of `parser.rs` updated to cover the new statement / expression cases.

**Verification**: `cargo test -p ynz-parser parse::` passes.

---

### Phase 4: Typeck extension + primitive intrinsic table + spec corrections
**PR scope**: Extend the type system with `Type::{Int, Float, Number, Bool}` (alongside M1's `String`, `Nothing`, `Error`). Type-check the M2 expression set: arithmetic / comparison / boolean / bitwise operator return-type rules; literal type inference; type-annotation enforcement; const-reassignment check; mixed-type compile error with three-part diagnostic suggesting `.toX()` methods; `number[N]` with N != 34 produces the bignum-deferral compile error pointing to M8. Add the primitive intrinsic dispatch table: `print` polymorphic over primitives; conversion methods (`.toNumber`, `.toFloat`, `.toString`) resolved against a hardcoded `PrimitiveIntrinsicTable`.
**Branch**: `feat/m2-typeck`
**Flag**: N/A
**Est. lines**: ~900
**Objective**: M2 source type-checks clean. The full matrix of typeck failures (mismatched types, const reassignment, undefined variable, wrong arity on `print`, `number[N]` for N != 34, etc.) produces three-part diagnostics with actionable suggestions.

**Spec corrections landing in this phase** (same PR as the implementation):
- `spec/variables.md:48` → change `// compiler knows: number` to `// compiler knows: int`.
- `spec/numeric-types.md:211` → rewrite "Mixed-type expressions promote to the most capable type in the expression." to describe the compile-error behavior with an example three-part diagnostic.

**Current-state anchors**:
- M1's `check.rs` shape: walks AST, emits diagnostics, gates on parse-error placeholders.
- `spec/numeric-types.md` (with the correction above) — the canonical truth for literal-typing and mixed-type rules.
- `design/numeric-types.md` — overflow semantics, mixed-precision rules (latter not yet exercised in M2 since N=34 only).
- `design/compiler-errors.md` — three-part format, banned jargon.

**Files (expected scope)**:
- `crates/ynz-typeck/src/types.rs` (extend `Type` enum)
- **`crates/ynz-typeck/src/builtins.rs` → `crates/ynz-typeck/src/intrinsics.rs`** (RENAMED — git mv, not a new file). `BuiltinTable` → `PrimitiveIntrinsicTable`. M1's `print(string) -> nothing` becomes the seed row in the new table, alongside the polymorphic `print` overloads (one per primitive type) and the conversion methods. **Single source of truth — there is exactly one intrinsic table in the compiler. No `BuiltinTable` survives M2.** The `#[cfg(test)]` `with_test_builtin` helper from M1's P5 carries over with the rename (`with_test_intrinsic`).
- `crates/ynz-typeck/src/check.rs` (extend the check pass; replace `BuiltinTable` references with `PrimitiveIntrinsicTable`)
- `crates/ynz-typeck/src/scope.rs` (new — block-scoped variable environment with `is_const` tracking)
- `crates/ynz-typeck/tests/check.rs` + snapshots (M1's tests that referenced `BuiltinTable` get renamed in the same PR; behavior unchanged)
- `spec/variables.md` and `spec/numeric-types.md` (corrections)

**BuiltinTable→PrimitiveIntrinsicTable migration mechanics (locked)**:
- The rename is `git mv crates/ynz-typeck/src/builtins.rs crates/ynz-typeck/src/intrinsics.rs` + symbol rename throughout the workspace. Done in a single commit so blame stays clean.
- M1's `print(string) -> nothing` becomes ONE row in the polymorphic-print entry of the new table: the entry is `print` with an arms-list of signatures (`string -> nothing`, `int -> nothing`, `float -> nothing`, `number -> nothing`, `bool -> nothing`). Resolution looks up `(method_name, receiver_type)`; the `print` entry matches any of the listed types.
- Test-only helper migrates: `BuiltinTable::with_test_builtin(name, sig)` → `PrimitiveIntrinsicTable::with_test_intrinsic(name, receiver_ty, sig)`. The shape gains a receiver-type slot because intrinsics are now method-receiver-dispatched (except `print`, which is a free-standing call). The test from M1 P5 that exercises the type-mismatch path keeps working under the new name.
- The variant-count test for `Type` from M1 gets renamed if its file moved; otherwise unchanged.

**Type rules (locked here)**:

| Op | Operand types | Result type | Notes |
|---|---|---|---|
| `+ - * /` | `int, int` | `int` | overflow → runtime panic (codegen wires) |
| `+ - * /` | `float, float` | `float` | IEEE binary64 semantics |
| `+ - * /` | `number, number` | `number` | IEEE decimal128 via runtime |
| `%` | `int, int` | `int` | truncating; LLVM `srem` |
| `%` | `float, float` | `float` | LLVM `frem` (NaN on zero) |
| `%` | `number, number` | **compile error** | pointing at v0.7 `math` module |
| `< <= > >= == !=` | `T, T` (same numeric T) | `bool` | |
| `== !=` | `bool, bool` | `bool` | |
| `== !=` | `string, string` | `bool` | byte-equality in M2 (Unicode-aware comparison is M7) |
| `&& \|\|` | `bool, bool` | `bool` | short-circuit (codegen) |
| `!` | `bool` | `bool` | |
| `& \| ^` | `int, int` | `int` | |
| `<< >>` | `int, int` | `int` | `>>` is arithmetic (sign-extending); per spec/operators.md |
| `~` | `int` | `int` | |
| `-` (unary) | `int` / `float` / `number` | same | |
| Any binary | mismatched types | **compile error** | three-part: WHAT (types differ) / WHAT-INSTEAD (call `.toX()` on one side) / WHY (no implicit numeric coercion in Yinz) |

**Literal-type-inference rules**:
- `IntLit` → `int` unless the binding context annotates `number` or `float` (then re-typed as that, parsed losslessly via the appropriate runtime helper).
- `NumberLit` → `number` unless the binding context annotates `float` (then re-typed as `float`, with possible binary-rounding loss documented at the parse site via an info-tier diagnostic — info tier is reserved-but-unused until v0.4; for M2 just type-correct, no diagnostic).
- `BoolLit` → `bool`.
- `StringLit` → `string`.

**Const + scope rules**:
- `const` bindings: emit diagnostic on any `Stmt::Assign` targeting the const name. Three-part: WHAT (const cannot be reassigned), WHAT-INSTEAD (use `let` if you need mutation), WHY (const expresses an intent that the binding doesn't change — catches bugs where the wrong name was assigned).
- Block-scoped: each `{ ... }` introduces a new scope. Shadowing is allowed (re-declaring the same name in a nested scope is fine).
- Undefined-variable error: three-part with a Levenshtein "did you mean" suggestion if a similar name is in scope.

**Primitive intrinsic table**:
- `print` is polymorphic over `int | float | number | bool | string`. Resolution happens in the call-site type check.
- Conversion methods (each is `fn(receiver) -> result_type`):
  - `int.toNumber() -> number`
  - `int.toFloat() -> float`
  - `int.toString() -> string`
  - `number.toFloat() -> float`
  - `number.toString() -> string`
  - `float.toNumber() -> number`
  - `float.toString() -> string`
  - `bool.toString() -> string`
- Method-call on a non-intrinsic name (e.g., `1.unknownMethod()`) → three-part diagnostic listing the available intrinsic methods on that primitive type. **No general method dispatch in M2** — the table is the universe.

**Deferral compile error for `number[N]` with N != 34**:
- Diagnostic body: WHAT ("number[N] for N != 34 is not yet implemented"), WHAT-INSTEAD ("use plain `number` (= `number[34]`) for now; if you genuinely need higher precision, see M8 in the v0.1 roadmap"), WHY ("Yinz's exact-decimal promise covers `number[N]` up to 4096 digits — the implementation lands in v0.1 milestone 8 before public v0.1 release").
- Source location: the `[N]` bracket. Span points at the integer N.
- This diagnostic is non-banned-jargon-clean — it's the one place "M8" appears in user-facing output, which is fine because it's a roadmap reference, not jargon.

**Steps**:
1. Extend `Type` enum with the M2 types. Update typeck's type-equality function. Variant-count test for `Type` with `// test-ratchet:` marker.
2. Implement `Scope` as a stack of `HashMap<Ident, (Type, is_const, source_span)>`. `push_scope` / `pop_scope` around blocks. `lookup(name)` walks the stack.
3. `check_let`: walks RHS, infers result type, applies annotation rules (annotation widens/narrows literal-typed values), inserts into scope.
4. `check_assign`: looks up `target` in scope. If const → three-part error. If not found → three-part undefined-variable error. If found → check RHS type against the bound type; error on mismatch.
5. `check_expr` (recursive): handle each `Expr` variant. For `BinOp` and `UnaryOp`, look up the operator's type rule from a table. For `MethodCall`, look up `(receiver_type, method_name)` in `PrimitiveIntrinsicTable`.
6. Implement the `PrimitiveIntrinsicTable` as a const lookup table — `&[(Type, &str, fn(args: &[Type]) -> Result<Type, Diagnostic>)]`. `print` is the polymorphic entry; conversion methods are the rest.
7. Wire `number[N != 34]` diagnostic: when parser produces `Type::Number { precision }` with precision != 34, typeck emits the deferral diagnostic at the binding site.
8. Mixed-type-arithmetic diagnostic: when operands of a binary op have different numeric types, emit the three-part error with the specific `.toX()` suggestion. The suggestion picks the WIDER of the two types: `int + number` → suggest `lhs.toNumber()` (widen int, lose nothing); `int + float` → suggest `lhs.toFloat()`; `number + float` → suggest `rhs.toNumber()` OR `lhs.toFloat()` (both lose precision in different ways; the diagnostic lists BOTH and explains the tradeoff — this is a teaching opportunity).
9. Spec corrections (`spec/variables.md:48`, `spec/numeric-types.md:211`) — same PR.
10. Tests (matrix coverage):
    - Happy path: M2 representative source type-checks clean.
    - Mismatched types: every cell of the mismatch matrix above produces a three-part diagnostic. Specifically test:
      - `let x: int = 1; let y: number = 2; let z = x + y` — error with `.toNumber()` suggestion.
      - `let x: int = 1; let y: float = 2.0; let z = x + y` — error with `.toFloat()` suggestion.
      - `let x: number = 1; let y: float = 2.0; let z = x + y` — error mentions BOTH conversions, explains tradeoff.
    - Const reassignment: `const x = 1; x = 2` — error.
    - Undefined variable: `let y = x` (x not in scope) — error with Levenshtein suggestion if similar name exists.
    - `number[5]` annotation → bignum-deferral error.
    - `number[5000]` annotation → above-4096 cap error (per `spec/numeric-types.md`).
    - `1.unknownMethod()` → error listing valid intrinsics.
    - `print(1, 2)` → arity error (print is single-arg in M2).
    - `1 < 2 < 3` → second comparison is `bool < int` — error.
    - `let x: int = 1.5` → type-mismatch error (number literal can't fit int annotation).
    - Const-vs-let parse-error gate: a function with a parse-error body produces NO typeck diagnostics for that body (M1's gate carries forward).
11. WHY-comments on every test.

**Acceptance criteria**:
- [ ] All mismatch-matrix cells covered by negative tests producing three-part diagnostics.
- [ ] Each diagnostic suggests the correct `.toX()` method per the rules above.
- [ ] Const-reassignment, undefined-variable, arity, and deferral errors all produce three-part diagnostics.
- [ ] `spec/variables.md` and `spec/numeric-types.md` corrections committed in the same PR.
- [ ] Variant-count test for `Type` pins M2 count.
- [ ] M1's typeck tests still pass.
- [ ] `tests/jargon_audit.rs` green on all new diagnostic strings.

**Quality gate**:
- [ ] No `unwrap()` in typeck changes.
- [ ] `PrimitiveIntrinsicTable` is a single source of truth — no scattered hardcoded method-name lists.
- [ ] Operator type-rule table is a single source of truth — no scattered hardcoded operator-result-type matches.
- [ ] The bignum-deferral diagnostic body is committed as a constant string, not synthesized at error time (makes it easy to grep and update when M8 removes the deferral).

**Verification**: `cargo test -p ynz-typeck` passes. `rg "promote to the most capable" spec/` returns empty (confirms the spec correction). `rg "compiler knows: number" spec/variables.md` returns empty (confirms the variables.md correction).

---

### Phase 5: Codegen extension (LLVM ops + runtime calls + overflow + short-circuit)
**PR scope**: Extend `ynz-codegen::emit_artifact` to lower the M2 typed AST. Emit LLVM IR for: stack-allocated locals (`alloca` + `store`/`load` per `let`/`const`/assign); int/float binary ops via native LLVM instructions; int overflow check via `llvm.sadd.with.overflow.i64` and friends, branching to a runtime panic stub on overflow; decimal ops via `call @ynz_decimal_add(...)`; short-circuit `&&` / `||` via basic-block branches with phi; comparison ops via native LLVM `icmp` / `fcmp` / runtime `ynz_decimal_compare`; bitwise ops native LLVM; conversion intrinsics (`.toNumber`, `.toFloat`, `.toString`) lowered to native LLVM casts or runtime calls; div-by-zero check on int and decimal (panic stub); float div-by-zero allowed to produce IEEE infinity per design.
**Branch**: `feat/m2-codegen`
**Flag**: N/A
**Est. lines**: ~1100
**Objective**: M2 smoke fixture compiles to a working binary that produces the expected stdout. Object-file SHA-256 is deterministic across runs (M1's reproducibility contract still holds). LLVM IR snapshot is regression-detected.

**Current-state anchors**:
- M1's `emit.rs` — the inkwell discipline (no inkwell types leak out of the module), the `CompiledArtifact` shape, the SHA-256 contract.
- `ynz-runtime` (from P1) — provides `ynz_decimal_*` C-ABI symbols.

**Files (expected scope)**:
- `crates/ynz-codegen/src/emit.rs` (extend lowering)
- `crates/ynz-codegen/src/runtime_decls.rs` (new — extern C declarations for runtime symbols)
- `crates/ynz-codegen/tests/golden.rs` + `__golden__/m2_smoke.{triple}.sha256` + `__snapshots__/m2_smoke.ll.snap`

**Lowering rules (locked here)**:

| Yinz construct | LLVM IR |
|---|---|
| `let x = expr` | `%x = alloca <ty>; store <ty> <expr-value>, ptr %x` |
| `x = expr` | `store <ty> <expr-value>, ptr %x` (target ptr from scope lookup) |
| Read `x` | `%tmp = load <ty>, ptr %x` |
| `int + int` | `%t = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %a, i64 %b); %r = extractvalue ... 0; %ov = extractvalue ... 1; br i1 %ov, label %panic, label %ok` |
| `int - int`, `int * int` | `llvm.ssub.with.overflow.i64`, `llvm.smul.with.overflow.i64` — same pattern |
| `int / int` | div-by-zero check + `sdiv i64` |
| `int % int` | div-by-zero check + `srem i64` |
| `float + float`, etc. | native `fadd`, `fsub`, `fmul`, `fdiv` (`float % float` → `frem`) |
| `number + number` | `call void @ynz_decimal_add(ptr %a, ptr %b, ptr %out)` |
| `number / number` | div-by-zero check (compare against decimal zero via `@ynz_decimal_compare`) + `call void @ynz_decimal_div(...)` |
| `int == int`, etc. | `icmp eq i64` |
| `float == float`, etc. | `fcmp oeq float` (ordered — NaN compares unequal, matches IEEE 754) |
| `number == number`, etc. | `call i32 @ynz_decimal_compare(ptr %a, ptr %b)` returning -1/0/1; then `icmp eq i32 %r, 0` |
| `a && b` | `entry: br i1 %a, label %rhs, label %short; rhs: ... br label %merge; short: br label %merge; merge: %r = phi i1 [%b, %rhs], [false, %short]` |
| `a \|\| b` | mirror of `&&` with `[true, %short]` |
| `int.toNumber()` | `call void @ynz_decimal_from_int(i64 %x, ptr %out)` |
| `int.toFloat()` | `sitofp i64 %x to double` |
| `int.toString()` | `%buf = alloca [24 x i8]; %n = call i64 @ynz_int_to_string(i64 %x, ptr %buf, i64 24)` — result ptr is `%buf`; caller-owned per the locked buffer-ABI |
| `number.toString()` | `%buf = alloca [48 x i8]; %n = call i64 @ynz_decimal_to_string(ptr %x, ptr %buf, i64 48)` |
| `float.toString()` | `%buf = alloca [32 x i8]; %n = call i64 @ynz_float_to_string(double %x, ptr %buf, i64 32)` |
| `bool.toString()` | select between two global constants (`@.str.true`, `@.str.false`) — no runtime call, no buffer needed |
| `print(<primitive>)` | the right `.toString` first (which lowers per above and produces a `ptr` to the caller's `alloca` buffer), then `call i32 @puts(ptr %s)` |

**Runtime extern declarations** (added to `runtime_decls.rs`, declared once per `Module`; signatures match the locked buffer-ABI from the M2 architectural-decisions block):
```llvm
declare void @ynz_decimal_add(ptr, ptr, ptr)
declare void @ynz_decimal_sub(ptr, ptr, ptr)
declare void @ynz_decimal_mul(ptr, ptr, ptr)
declare void @ynz_decimal_div(ptr, ptr, ptr)
declare i32  @ynz_decimal_compare(ptr, ptr)
declare void @ynz_decimal_from_int(i64, ptr)
declare i64  @ynz_decimal_to_string(ptr, ptr, i64)   ; (value, out_buf, buf_len) -> bytes_written
declare i64  @ynz_int_to_string(i64, ptr, i64)        ; same shape
declare i64  @ynz_float_to_string(double, ptr, i64)   ; same shape
declare void @ynz_panic_overflow(ptr)         ; ptr to a static C string describing the op
declare void @ynz_panic_div_by_zero(ptr)
```

**Memory model for M2 decimal locals**: `alloca` of `[16 x i8]` for each `number` local — exactly enough for one decimal128. The runtime reads/writes 16 bytes through the pointer. No heap allocation in M2 — every `number` is a stack value. (Heap allocation is M4's problem when user types land.)

**Steps**:
1. Extend `emit_artifact` to walk the M2 typed AST. Maintain a `HashMap<Ident, BasicValueEnum>` of in-scope variables (mapping name → alloca ptr for `let` / `const`, since both are stack-allocated; const-ness is enforced upstream at typeck).
2. Declare runtime externs once per Module.
3. Lower each operator per the table. Each lowering helper takes `(BasicValueEnum, BasicValueEnum) -> Result<BasicValueEnum, CodegenError>`.
4. Implement int-overflow check pattern: `extractvalue` from intrinsic result, branch on the i1, panic block calls `@ynz_panic_overflow(<op-descriptor-global-string>)` then `unreachable`, ok block is the new insertion point.
5. Implement div-by-zero pattern: compare denominator against zero, branch to panic block on true. For `int`: native `icmp`. For `number`: call `@ynz_decimal_compare` against a global decimal-zero constant.
6. Implement short-circuit `&&` / `||` with basic-block branching + phi at merge. Pattern is documented inline.
7. Implement `print` polymorphic intrinsic: for `string` operand, lower to `puts` directly (M1 pattern); for other primitives, lower the appropriate `.toString` first.
8. Wire SHA-256 reproducibility: M2 fixture's object bytes must be deterministic across runs on the same target triple. Module identifier explicitly set per M1's contract.
9. Tests:
   - Object SHA-256 golden for `m2_smoke.ynz` on each target triple.
   - IR text snapshot (informational) — useful for debugging codegen regressions; failure here is loud but doesn't gate.
   - Reproducibility test: codegen the same source twice in the same process; assert SHA-256 byte-identical.
   - End-to-end execution: compile + link + run the produced binary; capture stdout; assert exact match.
   - Integer-overflow runtime test: a fixture `let x: int = int.max ... ` — but **wait, `int.max` is M4-deferred per the catch-up list above**. So overflow runtime test uses a literal `9223372036854775807 + 1`. **Spec note**: int-literal overflow at parse-time would catch the `+1` half if both terms were literals; this test phrases it as `let x: int = 9223372036854775807; let y = x + 1` so the overflow happens at runtime, not parse.
   - Div-by-zero runtime test for int and number; assert non-zero exit code and the three-part panic message on stderr.
   - Decimal128 exactness: `let x = 0.1 + 0.2; print(x)` produces `0.3\n` exactly. **This is the load-bearing M2 demo.**
10. WHY-comments per testing rules.

**Acceptance criteria**:
- [ ] `m2_smoke.ynz` compiles, links, runs, prints `0.3\n1763\ntrue\n`.
- [ ] SHA-256 golden for `m2_smoke` committed per target triple.
- [ ] IR text snapshot committed (informational).
- [ ] Reproducibility test passes: same source, same SHA-256.
- [ ] Int-overflow runtime test exits non-zero with the three-part panic message.
- [ ] Int- and number-div-by-zero runtime tests exit non-zero with three-part panic messages.
- [ ] Float-div-by-zero produces `inf` or `-inf` (no panic).
- [ ] `let x = 0.1 + 0.2; print(x)` outputs exactly `0.3\n` (the load-bearing decimal exactness test).
- [ ] No `inkwell::Module` / `inkwell::Context` leaks outside `emit.rs` (grep-asserted).
- [ ] `codegen` salsa query depends on `check`, returns `Arc<CompiledArtifact>` (M1's shape preserved).

**Quality gate**:
- [ ] No `unwrap()` in codegen changes outside `verify()`-style sanity checks.
- [ ] Every runtime-extern declaration uses a single helper that prevents duplicate declarations across functions.
- [ ] Each overflow / div-by-zero panic block writes a static-string descriptor of the operation (e.g., `"int overflow in '+' at line N"`) so the runtime panic stub can render a useful message — the descriptor strings are subject to the jargon audit.
- [ ] All `unsafe` blocks (raw pointer manipulation for decimal allocas) carry `// SAFETY:` comments.

**Verification**: `cargo test -p ynz-codegen` passes. `./target/debug/ynz run crates/ynz-driver/tests/fixtures/m2_smoke.ynz` prints the expected stdout.

---

### Phase 6: Driver integration + polymorphic-print integration + M2 fixture suite
**PR scope**: Wire P1–P5 through the driver. Add M2 integration tests covering the full surface: smoke fixture, mixed-type compile-error fixtures, overflow panic fixtures, const-reassignment fixtures, deferral-error fixtures (number[N != 34]), banned-syntax fixtures (compound assignment, ternary attempt). Each negative fixture has a committed stderr snapshot per M1's byte-for-byte discipline.
**Branch**: `feat/m2-driver`
**Flag**: N/A
**Est. lines**: ~500 (driver glue is mostly mechanical; bulk is fixtures + stderr snapshots)
**Objective**: `cargo test -p ynz-driver integration::m2` covers every M2 happy-path and failure-mode permutation that the planning surface promises.

**Current-state anchors**:
- M1's `tests/integration.rs` and `tests/fixtures/` patterns. M2 extends them.
- M2 catch-up list above — each deferred feature has a fixture that documents the current (deferral) behavior so M4/M6/M8 know when they've completed the catch-up.

**Files (expected scope)**:
- `crates/ynz-driver/tests/fixtures/m2_smoke.ynz` (the headline integration test)
- `crates/ynz-driver/tests/fixtures/m2_mixed_int_number.ynz` (mixed-type compile error)
- `crates/ynz-driver/tests/fixtures/m2_mixed_int_float.ynz`
- `crates/ynz-driver/tests/fixtures/m2_mixed_number_float.ynz`
- `crates/ynz-driver/tests/fixtures/m2_const_reassign.ynz`
- `crates/ynz-driver/tests/fixtures/m2_int_overflow.ynz` (runtime panic)
- `crates/ynz-driver/tests/fixtures/m2_int_div_by_zero.ynz` (runtime panic)
- `crates/ynz-driver/tests/fixtures/m2_number_div_by_zero.ynz` (runtime panic)
- `crates/ynz-driver/tests/fixtures/m2_float_div_by_zero.ynz` (produces `inf`, exits 0)
- `crates/ynz-driver/tests/fixtures/m2_bignum_deferral.ynz` (`let x: number[100] = 1.0` → deferral error)
- `crates/ynz-driver/tests/fixtures/m2_compound_assign.ynz` (`x += 1` → teaching diagnostic)
- `crates/ynz-driver/tests/fixtures/m2_ternary_attempt.ynz` (`let y = (a > b) ? a : b` → teaching diagnostic)
- `crates/ynz-driver/tests/fixtures/m2_decimal_exactness.ynz` (the `0.1 + 0.2 == 0.3` headline)
- `crates/ynz-driver/tests/__snapshots__/m2_*.{stdout,stderr}.snap` (per fixture, both streams; snapshots for the failure-mode fixtures committed byte-for-byte)

**Catch-up fixtures (commented `// CATCH-UP <milestone>: ...` markers)**:
- `m2_int_max_deferred.ynz` — `let x = int.max` → "type-attached constants not yet implemented; M4 must add `int.max`."
- `m2_wrapping_add_deferred.ynz` — `x.wrappingAdd(1)` → "overflow methods not yet implemented; M4."
- `m2_string_parse_deferred.ynz` — `"42".toInt()` → "fallible conversions need maybe; M6."
- These fixtures are committed in M2 with the stderr snapshot showing the current diagnostic, and the snapshot is updated (with `// test-ratchet:` justification) when the catching-up milestone removes the deferral.

**Steps**:
1. Wire the driver to handle M2 sources. Most of the work is in P1–P5; this phase just adds fixtures and snapshots.
2. Run each fixture through the build/run pipeline; capture stdout/stderr; commit snapshots.
3. For each negative fixture, the stderr snapshot is the contract — the test asserts exact match.
4. For each catch-up fixture, the stderr snapshot captures the CURRENT deferral error. M4/M6/M8 will update these as they close the catch-up entries.
5. WHY-comments on every test.

**Acceptance criteria**:
- [ ] Every M2 fixture produces the expected stdout / stderr (committed snapshots).
- [ ] M2_smoke and m2_decimal_exactness exit 0 with the expected stdout.
- [ ] Every negative fixture exits non-zero with the expected stderr.
- [ ] Catch-up fixtures are clearly marked (`// CATCH-UP M4:`, etc.).
- [ ] M1's `hello.ynz` integration test still passes.
- [ ] Banned-jargon audit passes on every diagnostic emitted by these fixtures.

**Quality gate**:
- [ ] No `unwrap()` in driver code.
- [ ] Every stderr snapshot was reviewed for three-part WHAT/WHAT-INSTEAD/WHY structure during PR review (recorded in the PR description).

**Verification**: `cargo test -p ynz-driver integration::m2` passes on Linux and macOS.

---

### Phase 7: M2 verification sweep + tag `v0.1.0-m2`
**PR scope**: No new features. Verification sweep mirroring M1 P8. TODO sweep. M2 explicit-non-goals audit. Catch-up list audit (every deferred feature has a fixture + a clear forward owner in M4 / M6 / M8). CHANGELOG entry. Tag.
**Branch**: `chore/m2-verification`
**Flag**: N/A
**Est. lines**: ~80
**Objective**: M2 can be tagged without regret. Catch-up entries are unambiguous so downstream milestones cannot accidentally orphan them.

**Steps**:
1. **Broad TODO sweep** (same grep as M1):
   `rg -i 'TODO|FIXME|HACK|XXX|TEMP|PLACEHOLDER|acceptable for now|works in current state|fine until|we.?ll revisit|for now|good enough for the MVP|executor will figure' crates/`
   Migrate findings to the plan file (M3+ work), or `.claude/todos.md` (cross-milestone), or delete (no-longer-relevant comments). Zero results required to proceed.
2. **Catch-up list audit**:
   - For every entry in the M2 catch-up list, verify (a) a fixture exists that exercises the current deferral, (b) a stderr snapshot exists capturing the deferral diagnostic, (c) the owning milestone (M4 / M6 / M8) is unambiguously named in the diagnostic body AND in this plan file.
   - Tabulate the catch-up entries in this plan file (already done above in the "Catch-Up list for downstream milestones" section — verify the table is in sync with the fixtures).
3. **M2 explicitly-NOT list audit**: confirm nothing slipped in. Variant-count tests for `Token`, `Stmt`, `Expr`, `Type` confirm mechanically; this step is a sanity audit.
4. **Spec-correction verification**: `rg "promote to the most capable" spec/` returns empty; `rg "compiler knows: number" spec/variables.md` returns empty.
5. **`number[N]` deferral verification**: the deferral compile error mentions "M8" and the M8 roadmap entry mentions the catch-up obligation.
6. Run the full quality checklist below (M2-extended).
7. Add `CHANGELOG.md` entry for M2.
8. Tag `v0.1.0-m2` after merge.

**Acceptance criteria**:
- [ ] TODO sweep returns zero matches.
- [ ] Catch-up list audit passes — every deferred feature has fixture + snapshot + named owner.
- [ ] M2 "explicitly NOT" list audited; no slips.
- [ ] Spec corrections verified.
- [ ] Quality-checklist items below ticked with evidence.
- [ ] CHANGELOG entry committed.
- [ ] Git tag created.

**Verification**: The greps above return empty. `git tag -l v0.1.0-m2` returns the tag.

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

## Quality Checklist Addendum (verify at completion of M2)

M2 inherits every item above. Additional M2-specific items:

- [ ] `ynz-numerics` passes the M2-subset IEEE 754 decimal128 conformance corpus (Hursley `.decTest`, pinned SHA-256 in `CORPUS.sha256`)
- [ ] `ynz-numerics` differential test against Python `decimal` passes 10k random tuples on CI
- [ ] `ynz-numerics` property tests (commutativity, identity, sign, round-trip) all pass
- [ ] `libynz_rt.a` builds clean on Linux and macOS; symbol export verified via `nm`
- [ ] Generated binaries link `libynz_rt.a` and resolve all `ynz_*` extern symbols
- [ ] M2 smoke fixture (`m2_smoke.ynz`) runs end-to-end and produces the expected stdout
- [ ] M2 decimal-exactness fixture (`0.1 + 0.2 → 0.3` exact) passes
- [ ] M2 integer-overflow runtime test produces a three-part panic on stderr and exits non-zero
- [ ] M2 div-by-zero runtime tests (int + number) panic; float div-by-zero produces `inf` and exits 0
- [ ] Mixed-type arithmetic compile errors point at the correct `.toX()` method per the type-rule matrix
- [ ] `number[N]` for N != 34 produces the M8-deferral compile error (verified per the catch-up fixture)
- [ ] Spec corrections committed: `spec/variables.md` says `int` for `let x = 42`; `spec/numeric-types.md` describes mixed-type as a compile error (not promotion)
- [ ] `spec/operators.md` lists `%` in the arithmetic section
- [ ] Catch-up fixtures committed with current-state stderr snapshots; each names its owning milestone (M4 / M6 / M8) in the diagnostic body
- [ ] M2-extended variant-count tests (`Token`, `Stmt`, `Expr`, `Type`) pin their new counts with `// test-ratchet:` markers
- [ ] Object-file SHA-256 reproducibility contract still holds (per M1 P6 — M2 fixtures get their own golden hashes)

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each of the 8 phases is one PR with one branch. No phase mashes itself into "I'll split it later" — the branch name and est-lines are written down here, and each PR's scope is bounded by its phase block.
- **Shadow main branches**: every phase merges to `main` before the next starts. No long-lived `m1` umbrella branch. The integration risk of merging often (each PR may temporarily break the M1 end-to-end test until P7) is the smaller risk than the alternative.
- **Building the engine before shipping value**: M1 itself is the walking-skeleton answer to this anti-pattern. After M1, we have a real binary. After M2, we have variables. Every milestone produces a usable artifact, not a layer. **Honest disclosure**: P2 (diagnostics) is infrastructure ahead of value — it ships before any user-runnable output. We accept this because Golden Rule 11 (compiler is a teacher) is load-bearing for every later milestone; shipping a parser before diagnostics infrastructure is the duct-tape framing `no-duct-tape.md` prohibits. P2 isn't dressed up as value — it's explicitly infra-first, and we're calling it that.
- **Hotfix that isn't**: N/A — no production users. Will revisit when v0.1 actually ships and someone outside the team uses it.
- **Abandoned branches**: each phase is single-session-scoped. Branches that go stale (no merge within the session that opened them) get either merged or deleted at session end — they don't accumulate.
- **Flag graveyards**: N/A — the compiler doesn't use feature flags. `--release` is a user-facing build mode, not a feature flag. If we add an experimental flag for compiler-internal A/B (none planned), it gets a removal trigger documented at the time of introduction.

---

## Out-of-Scope For This Plan (per-milestone guardrails)

Each section is a final guardrail against scope creep — explicit redundancy with the per-milestone "explicitly NOT" lists, restated here so a contributor scanning the bottom of the plan sees the boundaries.

### Out-of-scope for M1 (historical — kept for reference)

- Variables (`let`, `const`) — M2 ✓ (now in progress)
- Arithmetic and operators — M2 ✓
- Numeric types (`int`, `float`, `number`, decimal128) — M2 ✓ (bignum `number[N>34]` deferred to M8)
- User-defined functions beyond `main` — M3
- Control flow — M3
- User-defined types — M4
- Ownership analysis — M4
- Generics, collections — M5
- Options, unions, maybe — M6
- Strings beyond ASCII bytes — M7
- The `errors` keyword and cascades — M7
- Iterables — M7
- Modules — M8
- Doc comments — M8
- Sensitive type modifier — M8
- Concurrency keywords parsing — M8

### Out-of-scope for M2 (CURRENT — do NOT slip these in)

- Bignum `number[N]` for N > 34 — M8 (the load-bearing carry; reserved syntax, compile-error pointer in M2)
- Overflow escape methods (`.wrappingAdd`, `.saturatingAdd`, etc.) — M4
- Type-attached constants (`int.max`, `int.min`, `number.epsilon`) — M4
- Fallible conversions (`.toInt`, string-to-numeric parsing) — M6 (needs `maybe`)
- General method dispatch — M4 (M2 uses an intrinsic table for the specific primitive methods it ships)
- User-defined functions beyond `main` — M3
- Control flow — M3
- User-defined types — M4
- Ownership analysis — M4
- Generics, collections — M5
- Options, unions, maybe — M6
- Strings beyond M1's UTF-8 byte arrays — M7
- The `errors` keyword and cascades — M7
- Iterables — M7
- Modules — M8
- `ynz watch`, `ynz fmt`, LSP — v0.2 (separate plan)
- `ynz test`, the test runner — v0.13 (separate plan)

If you find yourself adding code that touches any item above, STOP and either re-plan this milestone or escalate the work to its proper milestone.

---

## Reviewer Disputes

(none yet — populated during Step 7 review iterations if/when planner pushes back)
