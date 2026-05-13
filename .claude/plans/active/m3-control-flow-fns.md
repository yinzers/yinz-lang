---
slug: m3-control-flow-fns
owner: patrick
status: active
files:
  - crates/ynz-parser/**
  - crates/ynz-ast/**
  - crates/ynz-typeck/**
  - crates/ynz-codegen/**
  - crates/ynz-driver/**
  - spec/control-flow.md
  - spec/functions.md
created: 2026-05-13
last_updated: 2026-05-13-r1
depends_on: v0-1-compiler
---

# Plan: M3 — Control flow + user functions

Created: 2026-05-13
Status: active

**Parent milestone**: `.claude/plans/active/v0-1-compiler.md` (v0.1 roadmap umbrella).

---

## Context & Why

**Goal.** Implement M3 of the v0.1 compiler: `if`, multi-case `if`, `while`, `for x in range(...)`, early `return`, user-defined functions with parameters and return types, block scoping. After M3, Yinz programs can branch, loop, and decompose into multiple named functions — the first milestone where the language can express a non-trivial algorithm (recursion, accumulators, branching logic).

**Why now.** M2 just shipped (commit `c39fe8a`, tag `v0.1.0-m2`). M2 gave the language variables, arithmetic, decimal128, and the polymorphic primitive intrinsic table. M3 is the next step on the v0.1 critical path per the roadmap. Every later milestone (M4 types, M5 generics, M6 options, M7 errors, M8 modules) needs user-defined functions and control flow to be testable — without M3, none of them can be demoed end-to-end.

**Background.** Yinz spec defines control flow tersely (`spec/control-flow.md`): two forms of `if` (simple + multi-case with `=>` arrows), `while`, `for x in collection`. **There is no standalone `else { }` block** — the spec is explicit: "Two patterns [early return + pre-assignment], zero `else` blocks." The roadmap entry's "if/else" phrasing is shorthand for "if + multi-case `if`-with-`else =>`-catch-all" — not C-style `if/else`. This plan implements per spec, not per roadmap shorthand.

**Constraints.**
- Inherits every v0.1 architectural lock: salsa-from-day-1, three-part diagnostics, banned-jargon audit, object-file SHA-256 reproducibility, `inkwell` types confined to `emit.rs`, etc. See `.claude/plans/active/v0-1-compiler.md` "Locked Decisions" section.
- M3 ships **no ownership annotations** on parameters. `share`/`lend`/`give` are M4. M3 parameters are read-only (assignment to a parameter is a compile error). This sidesteps every ownership question and keeps M3 scope honest.
- M3 ships **only value-form multi-case `if`** (`200 => ...`) plus the `else =>` catch-all. The `is Type` form (`is Circle =>`) needs unions (M6). The `option_variant =>` form needs `options` (M6). M3's parser/typeck must emit a clear three-part deferral diagnostic if a user writes either form — pointing to M6, not silently accepting.
- M3 ships **`range` as a temporary builtin** for `for x in range(...)`. Spec calls this out: "with a temporary `range` builtin until proper iterables in M7." This is a documented deferral per `~/.claude/rules/no-duct-tape.md` — the what/why/cost/trigger are all spelled out below.
- M3 ships **no exhaustiveness checking for value-form multi-case** (per spec — value form requires `else =>` catch-all; exhaustiveness applies to options/unions which are M6).

**Success criteria for M3 (this milestone's contract):**

A multi-function fibonacci program compiles, runs, and produces the right output:

```yinz
function fib(n: int) -> int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

function main() -> nothing {
  let result = fib(10)
  print(result)
}
```

This compiles, runs, and prints `55\n`. Captured as `crates/ynz-driver/tests/fixtures/m3_fib.ynz`.

Additionally:
- `while` and `for x in range(...)` loops compile and run correctly.
- Multi-case `if` with value matching + `else =>` catch-all compiles and runs.
- A function whose paths don't all return (unless `-> nothing`) is a compile error with a three-part diagnostic naming the dropping path.
- Assignment to a parameter is a compile error with a three-part diagnostic pointing at M4 (when `lend` lands).
- `is Type` and options-variant multi-case forms produce a three-part deferral diagnostic pointing at M6.
- Object-file SHA-256 reproducibility contract still holds.
- M1 + M2 integration tests still pass.

---

## Research Findings

- **AST shape is already partially M3-ready.** `crates/ynz-ast/src/nodes.rs` has `Module { items: Vec<Item> }` and `Item::Function(FunctionDecl)` — multi-function modules supported. `FunctionDecl` currently has no `params` field; M3 adds it. `Block { stmts: Vec<Stmt> }` is reusable across function bodies, if-bodies, while-bodies, for-bodies.
- **Scope already supports push/pop.** `crates/ynz-typeck/src/scope.rs` has `Scope::push()` / `Scope::pop()` / `lookup()` / `all_names()`. M3 just calls push/pop around new control-flow blocks and function bodies.
- **`expr_types` keying bug already fixed.** M2 P5 changed the codegen `expr_types` HashMap key from `usize` (span.start) to `(usize, usize)` (start, end) — was needed because BinOp's start equals its leftmost child's start. M3's nested control flow doesn't reintroduce the issue.
- **Short-circuit codegen pattern exists.** M2's `&&`/`||` lowering uses basic-block branching + phi at merge. M3's `if`/`while`/`for` lowering reuses this exact pattern (branch + merge basic block; for loops use a back-edge instead of phi).
- **`alloca` + `mem2reg` for variable mutation across branches.** Every Yinz local lives in an `alloca`; both branches of an `if` `store` to the same alloca, and reads after the merge `load` the current value. LLVM's `mem2reg` pass (run by default in `OptLevel::None` for our debug pipeline? — verify in P4) promotes to SSA. **Risk note**: if `mem2reg` is NOT enabled at our optimization level, the IR is correct but unoptimized; that's fine for M3 since M2 already accepts unoptimized IR for the debug pipeline.
- **inkwell 0.9 basic-block API** is unchanged from M1/M2 usage. `function.append_basic_block(ctx, "name")`, `builder.position_at_end(bb)`, `builder.build_conditional_branch(cond, then_bb, else_bb)`, `builder.build_unconditional_branch(target_bb)`. Every basic block must end with exactly one terminator (`br`, `ret`, `unreachable`). M3 codegen must enforce this invariant — leaving a basic block without a terminator is a verify-time crash, not a compile-time error from inkwell.
- **Spec/control-flow.md is unambiguous on no-`else`-block.** Re-reading the spec confirms: simple `if` has no `else` clause; the alternation patterns are early-return and pre-assignment. Multi-case `if` uses `=>` arrows inside the block with optional `else =>` catch-all.
- **`for (item in items)` and `if (cond)` parens are required.** Per spec — both forms wrap their head in parens. This matches M1's `function main() -> nothing { ... }` paren-required style. Parser rejects `if cond { }` without parens.
- **Curly braces are required everywhere.** Spec: "Curly braces always required." `if (cond) stmt` (no braces) is a parse error with a teaching diagnostic.
- **Project structure has no `mod` declarations** — single-file compilation only until M8. M3 multi-function modules all live in one file.
- **`range` semantics**: Python convention. `range(end)` = 0..end (exclusive). `range(start, end)` = start..end (exclusive). No step parameter in M3 (defer). Type: `range(int) -> Range`, `range(int, int) -> Range`. `Range` is an internal type only — users cannot store it in a `let` binding or pass it as a function arg in M3. **Why this restriction**: `Range` as a first-class type leaks an iterable concept that M7 will replace; restricting it to `for x in range(...)` syntax (no other position) makes the M7 migration mechanical (rip out the special-case, plug in the iterable protocol).

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Two-pass typeck (collect signatures, then check bodies) breaks salsa incremental story | Medium | Future LSP integration suffers; recompiling one function invalidates all bodies | P3 wires the signature pre-pass as its own salsa query (`module_signatures(source_id) -> Arc<SignatureTable>`). Body-check depends on it, but only the body of the changed function gets re-checked when the body alone changes; signature changes cascade to all callers (correct behavior). Salsa handles the dependency graph. |
| Reachability / return-coverage analysis is non-trivial — easy to get wrong on multi-case + early returns | High | Functions that don't actually return on every path compile (silent bug at runtime) OR functions that DO return everywhere fail to compile (false-positive friction) | P3 builds a small CFG per function body. `analyze_return_paths(block) -> ReturnAnalysis { all_paths_return: bool, dead_code: Vec<SourceSpan> }` is a separate function with its own unit tests covering: empty function, single return, return in one if-arm only, return in both if-arms, return in multi-case + else, return in while body (does NOT count — while may not execute), return in for body (same — for may not execute), nested if-returns, `panic` and infinite loops (not in M3 — out of scope; but the analysis must still be correct for what M3 ships). Each case gets a dedicated test with a `// WHY:` comment. |
| Parser disambiguation between simple `if` and multi-case `if` | Medium | Multi-case parses as simple `if` with weird body, or vice versa | Locked rule: parser commits to multi-case form on seeing `<atom> => ...` or `else => ...` as the **first** statement in the block. Once committed, every subsequent statement must be an arm (a non-arm statement after committing produces a three-part diagnostic). Simple `if` has zero arms — the first statement is anything except an arm. A block starting with `else =>` is multi-case (no arm key, just the catch-all). Tested with adversarial inputs: `if (x) { foo() }` (simple), `if (x) { 1 => bar(); 2 => baz() }` (multi-case value), `if (x) { else => qux() }` (multi-case all-catch-all), `if (x) { 1 => foo(); bar() }` (multi-case then non-arm → diagnostic). |
| LLVM basic block terminator invariant — easy to forget terminator on a branch's tail | High | inkwell `module.verify()` fails at codegen time; failure mode is loud but blocks M3 entirely until fixed | P4 codegen for every control-flow construct ends with an explicit terminator-emission step. A unit test asserts that the verify pass succeeds on every M3 fixture. Adversarial cases tested: function whose last statement is `return` (single terminator), function whose `if` body returns on both arms (merge block is unreachable — emit `unreachable` terminator on merge), function whose `while` body never breaks out (loop has no exit edge — should not happen since condition check creates the exit edge). |
| `range` as builtin leaks into AST/typeck/codegen as a special case that's hard to remove in M7 | Medium | M7 iterables refactor becomes a full rewrite instead of a swap | `range` is implemented as a **single special-case in three places only**: (1) typeck `check_for_loop` recognizes `range(...)` as the iterable expression and types it specially; (2) codegen `lower_for_loop` recognizes `range(...)` and lowers to a counter loop; (3) `Type::Range { ... }` exists ONLY for type-checking the for-loop iterable position — not exposed in `let` bindings or function signatures (compile error if you try). M7 removes all three special-cases simultaneously by introducing `Iterable[T]` protocol. The deferral comment block in each location names M7 as the trigger and "swap for protocol dispatch" as the action. |
| Variable mutation across branches via `alloca` requires careful re-load at every basic block boundary | Medium | Stale value used in branch B after branch A wrote to the variable | Locked codegen pattern: every variable read emits `load <ty>, ptr %x` at the **point of use**, not cached at the function entry. M2's pattern already does this (verified in `crates/ynz-codegen/src/emit.rs`). M3 control-flow doesn't change this rule. Adversarial test: `let x = 1; if (cond) { x = 2 }; print(x)` — must print 2 if cond was true, 1 otherwise. The unit test asserts both paths via two fixtures with different `cond` values. |
| Mutual recursion stops working if signature collection order matters | Low (CFG-based pre-pass should prevent) | `foo() calls bar(); bar() calls foo()` fails to compile | P3's `collect_signatures` is a pure top-down walk over `Module.items` that builds a `SignatureTable` BEFORE any body is checked. Signature collection does NOT look at bodies, only declarations. Body check phase has the full signature table available. Mutual recursion test fixture: `function ping(n: int) -> int { if (n <= 0) { return 0 } return pong(n - 1) }; function pong(n: int) -> int { if (n <= 0) { return 0 } return ping(n - 1) }; function main() -> nothing { print(ping(5)) }`. Must compile and print `0`. |
| `main` signature check from M1 needs updating to coexist with user functions | Low | `main` validation breaks when other functions are defined | M1's check ("there's exactly one item, it's a function called main with `() -> nothing`") gets replaced by: (a) find the `main` function in the module — error if missing, (b) verify its signature is `() -> nothing` — error if not, (c) other functions get their own typeck per their declared signatures. The M1 test that asserts "missing main is an error" still passes because the new logic preserves that behavior. The M1 "wrong main return type" and "main with parameters" tests also still pass. |
| Variant-count test ratchet bumps could be forgotten | Medium | Silent scope creep — adding a variant without updating the lock test | M2 already locks variant counts via `// test-ratchet: <reason>` markers. Every M3 variant addition (Token, Stmt, Expr, Type) bumps the counter with a marker. The immutable-test-check.sh PreToolUse hook globally enforces the marker on Edit/Write. The M2 pattern is mechanical — M3 inherits it directly. |
| Object-file SHA-256 reproducibility regression | Low | M3 fixtures' golden hashes drift between runs | M1/M2's reproducibility contract is unchanged — explicit module identifier, fixed target triple, no debug info, deterministic LLVM options. M3 fixtures get their own per-triple golden hashes. The reproducibility test (codegen-twice asserts identical SHA-256) runs on every M3 fixture. |
| Banned-jargon audit fails on new diagnostic strings | Medium | CI red, blocks merge | Every new diagnostic in P1-P5 written following `design/compiler-errors.md` three-part format. The jargon audit runs in CI on every PR. Test-driven: write the diagnostic, then run `cargo test --workspace` to confirm clean. |
| Spec/code drift: spec says one thing, parser does another | Medium | User confusion; design intent lost | M3 ships with a spec-correction note in P3 if any drift is found during typeck implementation. Same PR as the implementation, no separate fix PR. |
| Comment rules sweep regression | Low | Section banners or "what" comments re-introduced | M2 P7 stripped 148 banners and "what" comments. M3 P6 runs the same sweep — `rg '// ──' crates/` must return empty, `rg '// [A-Z][a-z]+ part' crates/` returns near-empty. Comment rules from `~/.claude/rules/comments.md` apply: Tier 1 default no-comment, durable-not-changelog. |

---

## Questions

- **Should the unreachable-code detection be a warning or a suggestion?** Per `design/compiler-errors.md`, the three severity tiers exist (Error / Warning / Suggestion). Suggestion tier is reserved-but-unused until v0.4. M3 will emit unreachable-code as a **Warning** (compile succeeds, message rendered). If patrick prefers Suggestion (silent in M3, surfaced in v0.4), flip the tier in P3 — single-line change. **Default in this plan: Warning.**

- **Multi-case `if` on `string` values**: should `if (s) { "yes" => ...; "no" => ...; else => ... }` work in M3? Spec example only shows int values, but the value form's design clearly extends to strings. **Default in this plan: yes — string multi-case ships in M3** because strings already exist (M1) and the codegen is straightforward (`strcmp` against each case literal until a match; LLVM optimizes to a jump table or hash for many cases). Push back if you want to defer strings.

- **`return` without a value in a non-`nothing` function**: compile error or accepted? Spec is silent. **Default in this plan: compile error with three-part diagnostic** — `return` alone is only valid in `-> nothing` functions; everywhere else the value is required. Aligns with Yinz's "return types are always required" rule.

- **`continue` and `break`?** Spec doesn't mention them. **Default in this plan: deferred to M3.5 or M6 (out of M3 scope).** No `continue`/`break` keywords reserved in M3 — they're plain identifiers. If a user writes `break` thinking it works, they get an "undefined identifier" diagnostic. Trigger to revisit: a real Yinz program that genuinely needs early-loop-exit and can't refactor to early-return-from-function.

---

## Risk Assessment & Rollout Strategy

**Risk level: LOW (production) / MEDIUM (architectural debt).**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | — |
| Touches auth/permissions | No | — |
| Raw SQL / literals | No | — |
| Modifies existing data | No | Greenfield additions to compiler crates |
| Third-party integration | Yes | `inkwell` (LLVM), `salsa` — existing integrations, no new ones |
| Changes existing endpoints | No | — |
| Wrong foundational choice cascades | Yes | Two-pass typeck design, control-flow basic-block emission patterns — both will be reused in M4+ for type methods, M6 for narrowing, M7 for iterables |

**Mitigations applied:**
- Two-pass typeck implemented as its own salsa query → keeps incremental story intact, can be optimized later without breaking M3 contracts.
- `range` is a documented deferral with named M7 trigger → not silent debt.
- Reachability analysis is its own module with dedicated unit tests covering adversarial cases.
- Every M3 fixture gets per-triple SHA-256 golden hashes → reproducibility contract enforced mechanically.

**Rollout plan:** N/A. The compiler is a development tool with no users yet. M3 merges to main with the standard milestone tagging (`v0.1.0-m3`).

---

## Phases

Each phase is one PR. Branch merges to `main` before the next phase starts. Each phase ships via `/pr`.

### Phase 1: Lexer extension (M3 keywords + `=>`)
**PR scope**: Extend `ynz-parser::lex` to recognize the M3 keywords: `if`, `else`, `while`, `for`, `in`, `return`. Add the `=>` (FatArrow) token. Reject `match` and `switch` keywords with three-part teaching diagnostics (Yinz uses multi-case `if` instead).
**Branch**: `feat/m3-lexer`
**Est. lines**: ~150
**Ships via**: `/pr`
**Objective**: Lexing M3 source produces the expected token stream (snapshot-asserted). Banned keywords (`match`, `switch`) produce teaching diagnostics. `=>` is correctly disambiguated from `=` and `>`.
**Why this phase exists**: M3 grammar needs new keywords; lexer extension follows the M1/M2 pattern. The `match`/`switch` teaching diagnostics enforce the design decision (multi-case `if` replaces both) at the user surface — Golden Rule 11.
**Current-state anchors**:
- `crates/ynz-parser/src/token.rs` — `Token` enum, current count 42 (locked by `m2_token_variant_count_locked` test).
- `crates/ynz-parser/src/lexer.rs` — keyword recognition pattern (compare identifiers against keyword set after lex).
**Files (expected scope)**:
- `crates/ynz-parser/src/token.rs` (add variants)
- `crates/ynz-parser/src/lexer.rs` (extend keyword set, two-char operator `=>`)
- `crates/ynz-parser/tests/lex.rs` + snapshots
**Deviation rule**: Only the variants below. No `Match`, no `Switch`, no `Break`, no `Continue`. Variant-count test bumps from 42 → 49 (+7) with `// test-ratchet: M3 adds 7 for control flow + FatArrow` marker.

**New `Token` variants (M3)**:
- Keywords: `If`, `Else`, `While`, `For`, `In`, `Return` (6)
- Punctuation: `FatArrow` (`=>`) (1)
- Total new: 7. New count: 49.

**Banned-keyword diagnostics (P1 teaching surface)**:
- `match` → three-part: WHAT ("match is not a Yinz keyword"), WHAT-INSTEAD ("use multi-case `if` with `=>` arrows: `if (value) { 1 => ...; 2 => ...; else => ... }`"), WHY ("Yinz uses one `if` keyword for branching, branching-by-match, and branching-by-type. One concept = one keyword.").
- `switch` → same shape, "switch is not a Yinz keyword".

**Steps**:
1. Add the 7 new `Token` variants. Update `m2_token_variant_count_locked` → `m3_token_variant_count_locked` (rename) with the test-ratchet marker.
2. Extend keyword recognition: add the 6 new keywords to the keyword table.
3. Add `=>` lexing: after seeing `=`, peek for `>`. If present, emit `FatArrow`. Else fall back to `Eq` (M2 behavior).
4. Banned keywords (`match`, `switch`): when the lexer recognizes an identifier that matches either, emit the teaching diagnostic at the identifier's span. **Continue lexing** — produce an `Identifier(name)` token so the parser can recover (the user might have meant to define a function named `match`; M3 doesn't reserve them, just teaches against their canonical sense).
5. Snapshot tests: M3 source token-stream snapshot. Negative snapshots for `match`/`switch` teaching diagnostics. Each test has a `// WHY:` comment.

**Acceptance criteria**:
- [x] M3 source token-stream snapshot matches.
- [x] `match` and `switch` produce three-part teaching diagnostics.
- [x] `=>` lexes as `FatArrow`, distinct from `=` and `>`.
- [x] `m3_token_variant_count_locked` test pins the new count (49) with `// test-ratchet:` marker.
- [x] M1 + M2 lex tests still pass.

**Quality gate**:
- [x] No `unwrap()` in lexer changes.
- [x] `tests/jargon_audit.rs` green on new diagnostic strings.
- [x] No regression on M1 + M2 lex tests.

**Verification**: `cargo test -p ynz-parser --test lex` passes (45/45).

---

### Phase 2: AST + parser extension (M3 statements + parameters)
**PR scope**: Extend `ynz-ast::nodes` with `Stmt::If` (carrying optional multi-case arms), `Stmt::While`, `Stmt::For`, `Stmt::Return`. Extend `FunctionDecl` with a `params: Vec<Param>` field. Implement the parser side: parse function parameters (`name: Type` pairs, comma-separated, no ownership annotations); parse simple `if (cond) { block }`; parse multi-case `if (value) { arms }` with disambiguation; parse `while (cond) { block }`; parse `for (name in expr) { block }`; parse `return [expr]`.
**Branch**: `feat/m3-parser`
**Est. lines**: ~700
**Ships via**: `/pr`
**Objective**: M3 source parses to the snapshot AST with zero diagnostics. Malformed cases produce three-part diagnostics with correct spans; parser recovers per M1's strategy.
**Why this phase exists**: Establishes the AST shape and parser patterns for control flow. Multi-case disambiguation locked here is reused by M6's narrowing work.
**Current-state anchors**:
- `crates/ynz-ast/src/nodes.rs` — current AST. `Stmt` variant count 3, `Expr` count 10, `Type` count 7 (all locked by tests).
- `crates/ynz-parser/src/parser.rs` — Pratt precedence climber + recovery strategy (`is_stmt_boundary()`).
- `spec/control-flow.md`, `spec/functions.md`, `spec/scope.md` — canonical syntax.
**Files (expected scope)**:
- `crates/ynz-ast/src/nodes.rs` (extend `Stmt`, extend `FunctionDecl`, add `Param`, add `MatchArm`)
- `crates/ynz-parser/src/parser.rs` (add statement parsers + parameter parser)
- `crates/ynz-parser/tests/parse.rs` + snapshots

**New AST variants (M3)**:
- `Stmt::If { cond: Expr, body: Block, arms: Vec<MatchArm>, span: SourceSpan }` — `arms` empty means simple `if`; non-empty means multi-case. `body` and `arms` are mutually exclusive in practice (committed at parse time), but represented as a single struct for codegen ergonomics (simple if = no arms, multi-case if = arms only and empty body).

  **Refinement (locked here)**: split into two variants instead — `Stmt::If { cond, body, span }` (simple) and `Stmt::Match { scrutinee, arms, else_arm, span }` (multi-case). **Why split**: prevents a malformed-state where both `body` and `arms` are populated. The AST shape encodes the parser's disambiguation. Variant count is 4 + 2 (If, Match) instead of 4 + 1 (If) — accept the +1 for type safety.

- `Stmt::Match { scrutinee: Expr, arms: Vec<MatchArm>, else_arm: Option<Block>, span: SourceSpan }`
- `MatchArm { pattern: MatchPattern, body: Block, arrow_span: SourceSpan }`
- `MatchPattern { kind: MatchPatternKind, span: SourceSpan }`
- `MatchPatternKind` enum: `Value(Expr)` for value-form match; `IsType(String)` and `Variant(String)` reserved-but-rejected variants — parser accepts them but emits a three-part deferral diagnostic pointing at M6. **Why include them at parser level**: prevents users from getting a confusing "unexpected `is`" error; instead they get a clear "this form is M6" message. Variant-count test for `MatchPatternKind` is locked at 3 (Value, IsType, Variant) with the M6 reservation noted in `// test-ratchet:` comment. The `String` payload on `IsType` is a parser-level stand-in for M3's deferral surface; M6 widens it to a full `TypePath` once unions land — note this on the variant definition (`// REPLACE-AT M6: widen String to TypePath for narrowing`).
- `Stmt::While { cond: Expr, body: Block, span: SourceSpan }`
- `Stmt::For { var: String, var_span: SourceSpan, iter: Expr, body: Block, span: SourceSpan }`
- `Stmt::Return { value: Option<Expr>, span: SourceSpan }`
- `FunctionDecl` gains `pub params: Vec<Param>`.
- `Param { name: String, name_span: SourceSpan, ty: Type, ty_span: SourceSpan, span: SourceSpan }`. No ownership annotation field (M4 adds it).

**Deviation rule**: Only the variants above. No `Stmt::Break`, `Stmt::Continue`. No `MatchPatternKind::Range` (`1..10 =>`), no `MatchPatternKind::Or` (`1 | 2 =>`) — those are deferred indefinitely or to a future milestone if requested. Variant-count tests:
- `Stmt`: M2 = 3 (Expr, Let, Assign). M3 adds 5 (If, Match, While, For, Return) → 8. Test-ratchet: `M3 adds 5 for control flow`.
- `Expr`: M2 = 10, unchanged in M3.
- `Type`: M2 = 7, M3 adds 1 (`Type::Range` for the for-loop iterable position) → 8. Test-ratchet: `M3 adds 1 for range builtin iterable type — restricted to for-loop iterable position only, full Iterable[T] protocol in M7`.
- `MatchPatternKind`: new enum, count = 3 (Value, IsType, Variant). Locked with `m3_match_pattern_kind_variant_count` test.

**Parser disambiguation rule (locked)**:
- `parse_if`: after consuming `if (cond)`, parse `{`. Peek the first token:
  - `}` → empty simple `if` (body is an empty block).
  - `Identifier`, `IntLit`, `NumberLit`, `StringLit`, `BoolLit`, `Else` followed by `=>` → multi-case `if`. Parse arms.
  - Anything else → simple `if`. Parse statements until `}`.
- Once committed to multi-case, every subsequent top-level element in the block must be an arm. Encountering a non-arm statement produces a three-part diagnostic ("inside a multi-case `if`, every entry must be a `pattern => body` arm") and parser recovers to the next `=>` or `}`.

**Parser rules for `for` loop**: `for (name in expr) { block }`. Parser does NOT special-case `range(...)` — `expr` is any expression. Typeck (P3) verifies the expression is `Type::Range`. **Why parser-agnostic**: M7 will allow any `Iterable[T]` expression here; keeping the parser general now means M7 changes only typeck/codegen, not parser.

**Parser rules for parameters**:
- `function name(p1: T1, p2: T2, ...) -> RT { body }` — zero or more params, comma-separated, optional trailing comma.
- Each param is `name: Type`. No ownership annotations in M3.
- If the user writes `share name: Type` (or `lend`/`give`), parser emits a three-part diagnostic: WHAT ("ownership annotations are not yet implemented"), WHAT-INSTEAD ("declare the parameter without an annotation: `name: Type`"), WHY ("Yinz's ownership system lands in v0.1 milestone 4. Until then, parameters are read-only by value or by reference per type."). Parser recovers by skipping the annotation token and parsing the rest of the param normally.
- Duplicate parameter names within the same function are a three-part error (parser-level, since it's a structural concern).

**Parser rules for `return`**:
- `return` alone → `Stmt::Return { value: None }`.
- `return expr` → `Stmt::Return { value: Some(expr) }`.
- Parser doesn't enforce the function's return type — that's typeck (P3).

**Steps**:
1. Extend AST nodes per the spec above. Bump variant-count tests with `// test-ratchet:` markers.
2. Extend `FunctionDecl` parser to accept parameters. Update existing M1/M2 parse tests that construct `FunctionDecl` (snapshot updates only).
3. Add `parse_if`, `parse_while`, `parse_for`, `parse_return`. Dispatched from `parse_stmt`.
4. Add `parse_match_arm`. Reject `is Type` and `variant =>` forms with the M6 deferral diagnostic.
5. Recovery patterns documented at the top of `parser.rs`. Each new statement-parser follows: emit diagnostic on error, scan to next `;`/`}`/statement-boundary, continue.
6. Negative parse tests (each with `// WHY:` comments):
   - `if cond { }` (missing parens) → three-part diagnostic.
   - `if (cond) stmt` (missing braces) → three-part diagnostic.
   - `if (cond) { 1 => foo; bar() }` (multi-case then non-arm) → three-part diagnostic, recovery.
   - `if (shape) { is Circle => foo() }` → three-part M6 deferral diagnostic.
   - `function foo(share name: int) { }` → three-part M4 deferral diagnostic for `share`, parser recovers, param is accepted as `name: int`.
   - `function foo(a: int, a: int) -> int { return 0 }` → duplicate param diagnostic.
   - `return` at module level (outside any function) → already covered by parser, but verify the error message is the three-part shape.
   - Trailing comma in params: `function foo(a: int,) -> int { return a }` accepts.
   - Adversarial: nested `if` inside multi-case arm body: `if (x) { 1 => { if (y) { return 1 } return 2 } }` parses cleanly.
   - Adversarial: `for (i in range(0, 10)) { for (j in range(0, 10)) { print(i + j) } }` nested loops parse cleanly.
7. WHY-comments per the testing principles.

**Acceptance criteria**:
- [x] M3 representative source parses to snapshot AST.
- [x] All negative cases produce three-part diagnostics with correct spans and recovered AST.
- [x] `is Type` and options-variant multi-case forms produce M6 deferral diagnostics.
- [x] `share`/`lend`/`give` annotations on params produce M4 deferral diagnostics.
- [x] Variant-count tests pin M3 counts with `// test-ratchet:` markers.
- [x] M1 + M2 parser tests still pass.
- [x] Multi-case disambiguation tests cover the locked rule (8+ cases per the parser table above).

**Quality gate**:
- [x] No `unwrap()` in parser changes.
- [x] `tests/jargon_audit.rs` green on new diagnostic strings.
- [x] No `Stmt::If::arms` and `Stmt::If::body` ambiguity — the split into `Stmt::If` and `Stmt::Match` is preserved.

**Verification**: `cargo test -p ynz-parser --test parse` passes (49/49). Full workspace 236/236.

---

### Phase 3: Typeck extension — signature pre-pass, function bodies, control flow, return-path analysis
**PR scope**: Two-pass typeck. Pass 1 (`module_signatures` salsa query): walk `Module.items`, collect every `FunctionDecl`'s `(name, params, return_type)` into a `SignatureTable`. Reject duplicate function names. Reject `main` with non-`() -> nothing` signature. Pass 2 (extends existing `check_query`): walk each function body with the signature table available, push a fresh scope per function, register parameters as read-only `let` bindings (with a `is_param: true` flag to reject reassignment), check statements including new control-flow forms. Implement `analyze_return_paths` (CFG-based): every non-`-> nothing` function must have all paths return; unreachable-code after a definite-return emits a warning. Add `range` to the intrinsic table as a free-standing function with arity-1 (returns `Type::Range`) and arity-2 forms. Reject any `Type::Range` value in `let`/param/return position with a three-part M7 deferral message.
**Branch**: `feat/m3-typeck`
**Est. lines**: ~900
**Ships via**: `/pr`
**Objective**: M3 source type-checks clean (including mutual recursion). All typeck failures produce three-part diagnostics with actionable suggestions. `range` works in for-loop iterable position only; using `Type::Range` anywhere else is a compile error pointing to M7.
**Why this phase exists**: First milestone with multi-function modules + control flow. The two-pass design establishes the pattern that M4 (methods), M5 (generics), M6 (narrowing) will all extend.

**Architectural decisions locked in this phase**:
- **Two-pass typeck = separate salsa queries.** `module_signatures(source_id) -> Arc<SignatureTable>` runs first; `check(source_id)` depends on it. Why separate queries: a body-only change re-runs only the body check for that function (other bodies are still cached); a signature change cascades correctly to all callers via salsa's dependency tracking. **Alternative considered & rejected**: single query with internal two-pass — works but loses per-function incremental granularity, which v0.2 LSP needs.
- **Parameter scoping = function-body scope with `is_param: true` flag.** `Scope::insert` extended to accept a marker. `Stmt::Assign` typeck checks the flag: if `is_param`, emit three-part diagnostic ("parameters are read-only / parameter mutation lands in v0.1 milestone 4 with the `lend` ownership modifier / Yinz separates 'this function reads' from 'this function writes' to make code review answerable at a glance"). M4 will allow mutation when the parameter is declared `lend`.
- **Return-path analysis = recursive CFG walk over the body.** `fn analyze_return_paths(block: &Block, expected_ret: &Type) -> ReturnAnalysis` returns `{ all_paths_return: bool, dead_code: Vec<SourceSpan>, wrong_value_returns: Vec<(SourceSpan, Type, Type)> }`. The walk uses these rules:
  - `Stmt::Return { value: Some(e) }`: this path returns. Type-check `e` against `expected_ret`; if mismatch, accumulate. Mark subsequent statements in the same block as dead.
  - `Stmt::Return { value: None }`: this path returns IF `expected_ret == Nothing`; otherwise a "return without value in non-nothing function" diagnostic.
  - `Stmt::If`: a path returns if simple if-body returns AND control falls through to after the if (which doesn't return). Conservative: a simple `if` does NOT guarantee return-on-all-paths even if the body returns, because the false branch falls through.
  - `Stmt::Match` (multi-case): all paths return only if every arm AND the `else_arm` (if present) all return. Without `else_arm`, the multi-case has a fall-through (the scrutinee didn't match any arm — runtime panic? or fall-through silently? **Locked decision**: value-form multi-case WITHOUT `else =>` falls through silently; the multi-case is a non-exhaustive switch. This is the spec's stated semantics ("For value matching (numbers, strings), exhaustiveness isn't enforced — use `else =>` as a catch-all"). The return-path analysis treats no-`else_arm` as a fall-through path that doesn't return.
  - `Stmt::While`, `Stmt::For`: the loop body may never execute (zero iterations); does NOT count as a return-guaranteeing construct.
  - `Stmt::Expr` containing `panic(...)` — N/A in M3; `panic` is a runtime concept, not a typeck-special-case. Reaches via runtime, not via the type system.
- **`range` typeck rules**: `range(end)` and `range(start, end)` are accepted in the for-loop iterable position. Both args must be `int`. `range` returns `Type::Range { start_inclusive: bool /*always true*/, end_inclusive: bool /*always false in M3*/, element: Box<Type> /*always Int in M3*/ }` — the type carries metadata for the codegen pass. `range(...)` in any other position (RHS of `let`, function arg, function return type, multi-case scrutinee) is a three-part diagnostic pointing at M7.
- **Dead-code reporting = warning, not error.** Per the open question above; flip if patrick prefers Suggestion tier.
- **`main` signature check moves into `module_signatures` pass.** When the signature pre-pass collects signatures, it also verifies `main` exists with `() -> nothing`. M1's existing missing-`main`, wrong-return-type, and has-parameters tests are preserved — same diagnostics, just emitted from the new location.

**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs` — current check pass.
- `crates/ynz-typeck/src/scope.rs` — push/pop/lookup already exists.
- `crates/ynz-typeck/src/intrinsics.rs` — `PrimitiveIntrinsicTable` (M2); `range` is added as a free-standing function (not method).
- `crates/ynz-typeck/src/types.rs` — current `Type` enum (7 variants).

**Files (expected scope)**:
- `crates/ynz-typeck/src/types.rs` (add `Type::Range`)
- `crates/ynz-typeck/src/signatures.rs` (NEW — `SignatureTable`, `collect_signatures` function, `module_signatures` salsa query)
- `crates/ynz-typeck/src/check.rs` (extend with: parameter handling, control-flow statement checks, two-pass orchestration, return-path call)
- `crates/ynz-typeck/src/return_paths.rs` (NEW — `analyze_return_paths` + `ReturnAnalysis`)
- `crates/ynz-typeck/src/intrinsics.rs` (add `range` free-fn entries; introduce a `FreeFnSig` table parallel to `methods` table)
- `crates/ynz-typeck/src/queries.rs` (add `module_signatures` query, extend `check` to depend on it)
- `crates/ynz-typeck/tests/check.rs` + snapshots
- `design/decisions.md` (record the loop-var-const decision)

**Steps**:
1. Add `Type::Range { ... }` variant. Update `m3_type_variant_count_locked` test with marker. Update `Type::display` / `PartialEq` impls.
2. Create `signatures.rs`. Define `SignatureTable { fns: HashMap<String, FunctionSig> }` and `FunctionSig { params: Vec<Type>, ret: Type, decl_span: SourceSpan }`. Implement `collect_signatures(module: &Module) -> (SignatureTable, Vec<Diagnostic>)`:
   - Walk `module.items`.
   - For each `Item::Function`, record signature. If name already present, emit three-part duplicate-function diagnostic with both spans.
   - After walking: check for `main`. Missing → three-part diagnostic. Wrong signature → three-part diagnostic.
3. Wire `module_signatures` salsa query. Returns `Arc<(SignatureTable, Vec<Diagnostic>)>`.
4. Extend `check` query to: (a) call `module_signatures`, (b) for each `Item::Function`, push a fresh function-body scope, insert params as `(name → (Type, is_param=true))`, walk body. The existing parse-error gate from M1 still applies per-function.
5. Implement statement checkers:
   - `check_stmt_if(stmt)`: type-check condition (must be `bool`), push scope, walk body, pop scope.
   - `check_stmt_match(stmt)`: type-check scrutinee, for each arm verify pattern type matches scrutinee type and arm body has no parse errors. Push/pop scope per arm. Reject `IsType` and `Variant` patterns with M6 deferral.
   - `check_stmt_while(stmt)`: condition must be `bool`, push/pop scope, walk body.
   - `check_stmt_for(stmt)`: iterable must be `Type::Range` (with int element); push scope; insert loop var as `(int, is_param=false, is_const=true)`. **Design decision locked in P3**: the loop variable is **const** (immutable inside the body); assignment to the loop var is a three-part diagnostic. Rationale: spec/scope.md models loop-locals as fresh bindings each iteration; allowing mutation would make `i = 10` inside `for (i in range(0, 5))` ambiguous (skip ahead? rebind for the remainder? both are bad answers). Decision recorded in `design/decisions.md` in the same PR as P3 implementation (P3 ships the spec/design edit, not just the code).
   - `check_stmt_return(stmt)`: type-check value against function's expected return type. If value is `None`, function must be `-> nothing`. If value is `Some` and function is `-> nothing`, three-part error.
6. Implement `analyze_return_paths` in `return_paths.rs`. Pure function over `&Block`. Returns `ReturnAnalysis`. Unit-tested with adversarial cases (see Risks table).
7. After each function body check, call `analyze_return_paths`. Emit warnings for dead code, errors for missing returns.
8. Extend `intrinsics.rs`: add a `free_fns` table parallel to `methods`. Add `range` entries: `range(int) -> Range { element: Int, end_exclusive: true }` and `range(int, int) -> Range { ... }`. Method-call typeck unchanged (still uses `methods` table).
9. Extend call-site typeck (`check_call`):
   - First check `PrimitiveIntrinsicTable.free_fns` (covers `print`, `range`).
   - Then check `SignatureTable.fns` (user functions).
   - If neither has the name → three-part undefined-function diagnostic with Levenshtein suggestion.
   - For user-function calls: verify arity, then type-check each arg against the param type.
10. Reject `Type::Range` in non-for-loop-iterable positions:
    - `let x: Range = ...` → M7 deferral diagnostic.
    - `function foo(r: Range) -> nothing` → M7 deferral diagnostic.
    - `function foo() -> Range` → M7 deferral diagnostic.
    - Multi-case scrutinee of `Range` → M7 deferral.
11. Tests:
    - **Happy path**: M3 representative source with multiple functions, control flow, range loops, recursion — type-checks clean.
    - **Mutual recursion**: ping/pong fixture from the Risks table.
    - **Duplicate function name**: two functions named `foo` → three-part with both spans.
    - **Missing `main`**: empty module → three-part.
    - **`main` with wrong signature**: `function main(x: int) -> nothing { }` → three-part.
    - **Parameter mutation**: `function foo(x: int) -> int { x = 5; return x }` → three-part M4 deferral.
    - **Loop var mutation**: `for (i in range(0, 10)) { i = 5 }` → three-part error.
    - **Wrong return type**: `function foo() -> int { return "hi" }` → three-part.
    - **Missing return**: `function foo() -> int { print(1) }` → three-part with the dropping path's span.
    - **`return` without value in non-nothing**: `function foo() -> int { return }` → three-part.
    - **Value-return in nothing**: `function foo() -> nothing { return 1 }` → three-part.
    - **Dead-code warning**: `function foo() -> int { return 1; print(2) }` → warning at `print(2)`'s span.
    - **Multi-case fall-through allowed**: `function foo(x: int) -> nothing { if (x) { 1 => print(1); 2 => print(2) } print("done") }` — type-checks clean (fall-through is fine for nothing-return).
    - **Multi-case non-exhaustive in non-nothing**: `function foo(x: int) -> int { if (x) { 1 => return 1; 2 => return 2 } }` → three-part missing-return diagnostic (no `else =>`, fall-through doesn't return).
    - **Multi-case exhaustive with else**: `function foo(x: int) -> int { if (x) { 1 => return 1; else => return 0 } }` — type-checks clean.
    - **`is Circle =>`**: three-part M6 deferral.
    - **`Range` outside for-loop**: `let r = range(0, 10)` → three-part M7 deferral.
    - **`range` arity error**: `range(1, 2, 3)` → three-part arity diagnostic.
    - **`range` wrong arg type**: `range("hi")` → three-part type mismatch.
    - **Undefined function**: `function main() -> nothing { unknownFn() }` → three-part with Levenshtein.
    - **Function arg type mismatch**: `function foo(x: int) -> int { return x }; function main() -> nothing { foo("hi") }` → three-part.
    - **String multi-case**: `function foo(s: string) -> nothing { if (s) { "a" => print(1); "b" => print(2); else => print(0) } }` — clean.
    - **Fibonacci**: the M3 contract fixture type-checks clean.
    - **Parse-error gate**: a function with a parse-error body produces NO typeck diagnostics for that body (M1's gate carries forward).
    - **Adversarial: multi-case with else but value-arms don't return**: `function foo(x: int) -> int { if (x) { 1 => print("oops"); else => return 0 } }` — value-arm doesn't return; fall-through path doesn't return → three-part missing-return diagnostic. Distinct from the "non-exhaustive without else" case above; this one HAS the else and STILL fails because not every arm body returns.
    - **Adversarial: `while (true)`** with no `break` (and M3 has no `break`) — does `function foo() -> int { while (true) { } }` compile or error? **Locked decision in P3**: error with three-part missing-return diagnostic. Rationale: the typeck doesn't do constant-folding of conditions, so "while true" looks identical to "while cond" where cond might evaluate false at runtime. Conservative analysis = treat every loop as may-not-execute. If a user wants "this loop runs forever," they write `return` inside the body. The diagnostic body explicitly mentions this guidance.
    - **Adversarial: nested call expressions**: `function add(a: int, b: int) -> int { return a + b }; function main() -> nothing { print(add(add(1, 2), add(3, 4))) }` — exercises nested call type-check (each call arg is itself a call). Must type-check and emit `10\n`.
    - **Adversarial: empty function body, non-nothing return**: `function foo() -> int { }` — zero-statement block, no return → three-part missing-return diagnostic. Edge case for return-path analyzer (empty Block input).
    - **Adversarial: dead code interaction with multi-case**: `function foo(x: int) -> int { if (x) { 1 => { return 1; print(2) }; else => return 0 } }` — dead-code warning on `print(2)` AND multi-case still counts as returning on all paths (arm-1 returns via dead-code-after-return; else returns; no fall-through). Tests the analyzer composes correctly.
    - **Adversarial: empty / reversed range**: `function main() -> nothing { for (i in range(5, 5)) { print(i) } }` (empty range, zero iterations) — type-checks clean, runs to completion with no output. Similarly `range(5, 3)` (start > end) — locked behavior: empty range, zero iterations, no error. The codegen-side check confirms zero-iteration safety in P4.
12. WHY-comments on every test.

**Acceptance criteria**:
- [x] All test cases above pass (75/75 typeck tests).
- [x] Mutual recursion compiles and type-checks clean.
- [x] `analyze_return_paths` is unit-tested in `return_paths.rs` (7 dedicated unit tests).
- [x] `module_signatures_query` is a separate salsa query; `check_query` depends on it.
- [x] `range` is in the intrinsics free-fn table, callable from for-loop iterable position.
- [x] `Type::Range` in let-binding position produces an M7 deferral.
- [x] Variant-count test for typeck `Type` pins M3 count (8).
- [x] M1 + M2 typeck tests still pass.

**Quality gate**:
- [x] No `unwrap()` in typeck changes.
- [x] `tests/jargon_audit.rs` green on all new diagnostic strings.
- [x] `SignatureTable` is separate from `PrimitiveIntrinsicTable`.
- [x] Return-path analysis is its own module (`return_paths.rs`), not inlined into `check.rs`.

**Verification**: `cargo test -p ynz-typeck` passes (75/75). Full workspace 282/282.

---

### Phase 4: Codegen extension — control flow lowering + multi-function modules + range loops
**PR scope**: Extend `ynz-codegen::emit_artifact` to walk a multi-function `TypedModule`. Emit one LLVM function per `FunctionDecl` with proper C ABI (parameters lowered per their type, return value boxed for the runtime where needed). Lower `Stmt::If` to conditional branch + merge basic block. Lower `Stmt::Match` to a chain of conditional branches (value comparison per arm + else-arm fallthrough). Lower `Stmt::While` to pre-header + body + back-edge. Lower `Stmt::For` (with `range(...)` iterable) to a counter loop. Lower `Stmt::Return` to LLVM `ret`. Maintain the basic-block terminator invariant — every block ends with exactly one terminator.
**Branch**: `feat/m3-codegen`
**Est. lines**: ~900
**Ships via**: `/pr`
**Objective**: M3 fixtures compile to working binaries that produce the expected stdout. Object-file SHA-256 is deterministic across runs. LLVM `module.verify()` succeeds on every M3 fixture.
**Why this phase exists**: First time codegen handles multi-function modules and non-straight-line control flow. The basic-block emission patterns established here are reused by M4 (method dispatch), M6 (narrowing), M7 (iterables).

**Architectural decisions locked in this phase**:
- **Multi-function emission**: walk `TypedModule.items`. For each function, generate the LLVM function signature once (forward declaration for all functions before any body is emitted — supports mutual recursion at the LLVM level), then walk each body. Same pattern as M1's `puts` extern declaration.
- **Parameter ABI in M3**: scalars (`int`, `float`, `bool`) passed by value (LLVM `i64`, `double`, `i1`). `string` passed as `ptr` (pointer to UTF-8 bytes; length is null-terminator-implied per M1's strings). `number` passed as `ptr` (pointer to 16-byte decimal128). On the callee side: scalar params live in their value-typed LLVM args; pointer params get an `alloca` of the corresponding type and a `load`-then-`store` to materialize a local copy (since M3 params are read-only, the local copy is for uniformity with `let` bindings and is dead-code-eliminated by LLVM in release builds). **Why this and not "params live in their original storage"**: keeping the variable model uniform (every name = an alloca) means the existing M2 `Stmt::Let` lowering pattern works unchanged. The extra copy for pointer params is paid once per call and is trivially elided by LLVM.
- **`Stmt::Return` lowering**: emit `ret <ty> <value>` for value returns, `ret void` for nothing returns. After emitting the `ret`, the current basic block is terminated; subsequent statements in the same block are dead code (typeck already warned in P3, but codegen must still skip emission to avoid generating IR after a terminator — which is an inkwell verify error).
- **`Stmt::If` simple lowering**: emit `then_bb`, `merge_bb`. Conditional branch from current bb: cond ? then_bb : merge_bb. Position at `then_bb`, emit body. If body's last stmt is not a terminator, emit `br merge_bb`. Position at `merge_bb`. Continue.
- **`Stmt::Match` lowering**: chain of conditional branches. For each arm: emit a comparison (`icmp` for int, `fcmp` for float, `strcmp` runtime call for string, `ynz_decimal_compare` for number, `icmp` for bool), branch to `arm_body_bb` on equality OR `next_check_bb` on inequality. After arms, the `else_arm` body (or just the fall-through merge if no else). **Locked**: string multi-case uses `strncmp` from libc OR a new `ynz_string_eq(ptr, ptr) -> i1` runtime shim. **Decision**: add `ynz_string_eq` to `ynz-runtime` as a thin wrapper around `strcmp` to keep the C-ABI consistent with the decimal compare pattern. New runtime shim in `ynz-runtime/src/string_shims.rs`, lowering test verifies it.
- **`Stmt::While` lowering**: emit `header_bb`, `body_bb`, `exit_bb`. Unconditional branch from current bb to header_bb. At `header_bb`, evaluate condition; conditional branch to body_bb or exit_bb. At `body_bb`, emit body, then unconditional branch back to header_bb (back-edge). Position at `exit_bb`. Continue.
- **`Stmt::For` with `range(...)` lowering**: rewrite to an equivalent while-loop at codegen time. Pattern:
  - Allocate `%counter = alloca i64`. Store the range's start (0 for `range(end)`, `start` for `range(start, end)`).
  - Allocate `%end = alloca i64`. Store the range's end.
  - Header: load `%counter`, load `%end`, `icmp slt` (signed less-than, since end-exclusive), conditional branch.
  - Body: bind the loop variable to the current counter value (alloca the loop var, store the counter's loaded value into it), emit the body.
  - Body tail: increment `%counter` by 1, branch back to header.
  - Exit: continue.
  - **Why bind loop var via alloca+store instead of using `%counter` directly**: keeps "every Yinz name = an alloca" invariant. LLVM mem2reg elides the redundant copy in release builds.
- **Multi-function call lowering**: existing M2 call-site pattern (lookup the LLVM function by name via `module.get_function(name)`, build_call) works. For user-defined functions, the call resolves to the LLVM function emitted from the corresponding `FunctionDecl`. Forward-declare all M3 module functions before emitting bodies, so mutual recursion's call sites can resolve.
- **`main` change**: M1's hardcoded `main` emission generalizes to "the function named `main` gets emitted with C ABI returning `i32`, body returns 0 implicitly if its `-> nothing` body doesn't have an explicit `return`." Other functions get their declared return ABI.
- **Reproducibility contract preserved**: M3 fixture object bytes are deterministic. Each fixture gets a per-triple SHA-256 golden.

**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs` — M2 codegen, especially the short-circuit `&&`/`||` basic-block pattern that M3 generalizes.
- `crates/ynz-codegen/src/runtime_decls.rs` — extern declarations for runtime symbols.

**Files (expected scope)**:
- `crates/ynz-codegen/src/emit.rs` (extend with multi-function emission + control-flow lowering)
- `crates/ynz-codegen/src/runtime_decls.rs` (add `ynz_string_eq` extern)
- `crates/ynz-runtime/src/string_shims.rs` (NEW — `ynz_string_eq` impl)
- `crates/ynz-runtime/src/lib.rs` (export `string_shims`)
- `crates/ynz-codegen/tests/golden.rs` + `__golden__/m3_*.{triple}.sha256` + `__snapshots__/m3_*.ll.snap`

**Steps**:
1. Add `ynz_string_eq` runtime shim. C-ABI: `extern "C" fn ynz_string_eq(*const u8, *const u8) -> i32` (1 if equal, 0 if not). Internally uses `libc::strcmp`. Add to `nm` symbol verification.
2. Extend `runtime_decls.rs` with the new extern.
3. Refactor `emit_artifact` to emit multiple LLVM functions. First pass: forward-declare each function's signature. Second pass: emit each body.
4. Add `lower_param` helper: scalar params load directly into a value-typed alloca; pointer params materialize a local alloca + load + store.
5. Add `lower_stmt_if`, `lower_stmt_match`, `lower_stmt_while`, `lower_stmt_for`, `lower_stmt_return` helpers. Each follows the locked patterns above.
6. Ensure every basic block ends with exactly one terminator. After lowering a body, if the current bb has no terminator AND the function is `-> nothing`, emit `ret void`. If the function is non-nothing, this is a typeck bug (P3 should have caught it) — emit `unreachable` and log a debug-only assertion.
7. Wire SHA-256 reproducibility for M3 fixtures: module identifier explicitly set per M1's contract.
8. Tests:
   - Object SHA-256 golden for each M3 fixture per target triple.
   - IR text snapshot per fixture (informational).
   - Reproducibility test: codegen the same source twice, assert SHA-256 identical.
   - End-to-end execution: compile + link + run each fixture; capture stdout; assert exact match.
   - **Fibonacci fixture**: prints `55\n`.
   - **Mutual recursion fixture (ping/pong)**: prints `0\n`.
   - **While-loop fixture**: countdown from 5 to 1, prints `5\n4\n3\n2\n1\n`.
   - **For-loop fixture**: `for (i in range(0, 5)) { print(i) }` prints `0\n1\n2\n3\n4\n`.
   - **Multi-case int fixture**: prints the right branch.
   - **Multi-case string fixture**: prints the right branch.
   - **Multi-case else fixture**: falls through to `else =>` on unmatched scrutinee.
   - **Nested control flow**: `for` inside `if` inside another function.
   - **Early return mid-function**: emits `ret`, dead-code skipped.
   - **Module verify passes** on every fixture: a dedicated test asserts `module.verify().is_ok()` for the M3 typed module.
9. WHY-comments per testing principles.

**Acceptance criteria**:
- [ ] `m3_fib.ynz` compiles, links, runs, prints `55\n`.
- [ ] All M3 fixtures listed above produce expected stdout.
- [ ] SHA-256 goldens committed per target triple.
- [ ] Reproducibility test passes.
- [ ] `module.verify()` succeeds on every M3 fixture.
- [ ] M1 + M2 codegen tests still pass.
- [ ] No `inkwell::Module` / `inkwell::Context` leaks outside `emit.rs` (grep-asserted).
- [ ] `codegen` salsa query depends on `check`, returns `Arc<CompiledArtifact>` (M1's shape preserved).
- [ ] `nm libynz_rt.a | grep ynz_string_eq` returns a defined symbol.

**Quality gate**:
- [ ] No `unwrap()` in codegen changes outside `verify()`-style sanity checks.
- [ ] Every basic block terminator-emit step is followed by a position-at-end of a fresh bb (no orphaned bbs).
- [ ] `lower_stmt_for` references the `range` builtin only in its dedicated handler — no other codegen path special-cases `Type::Range`.
- [ ] All `unsafe` blocks in the new runtime shim carry `// SAFETY:` comments per M2 P1's pattern.

**Verification**: `cargo test -p ynz-codegen` passes on Linux + macOS. `./target/debug/ynz run crates/ynz-driver/tests/fixtures/m3_fib.ynz` prints `55`.

---

### Phase 5: Driver integration + M3 fixture suite
**PR scope**: Wire P1–P4 through the driver. Add M3 integration tests covering the full surface: fibonacci, mutual recursion, while, for-with-range, multi-case (int + string + else), early-return, parameter-mutation error, missing-return error, M6 deferrals, M7 deferrals, undefined-function with Levenshtein. Each negative fixture has a committed stderr snapshot per M1/M2's byte-for-byte discipline.
**Branch**: `feat/m3-driver`
**Est. lines**: ~400
**Ships via**: `/pr`
**Objective**: `cargo test -p ynz-driver integration::m3` covers every M3 happy-path and failure-mode permutation that this plan promises.
**Why this phase exists**: First and most important end-to-end test for M3. P1–P4 are unit-tested in isolation; this phase proves they compose.

**Current-state anchors**:
- M2's `tests/integration.rs` and `tests/fixtures/` patterns. M3 extends them.

**Files (expected scope)**:
- `crates/ynz-driver/tests/fixtures/m3_fib.ynz` (headline)
- `crates/ynz-driver/tests/fixtures/m3_mutual_recursion.ynz` (ping/pong)
- `crates/ynz-driver/tests/fixtures/m3_while_countdown.ynz`
- `crates/ynz-driver/tests/fixtures/m3_for_range.ynz`
- `crates/ynz-driver/tests/fixtures/m3_for_nested.ynz` (range inside range)
- `crates/ynz-driver/tests/fixtures/m3_multicase_int.ynz`
- `crates/ynz-driver/tests/fixtures/m3_multicase_string.ynz`
- `crates/ynz-driver/tests/fixtures/m3_multicase_else.ynz`
- `crates/ynz-driver/tests/fixtures/m3_early_return.ynz` (return from inside if-body)
- `crates/ynz-driver/tests/fixtures/m3_param_mutation.ynz` (compile error)
- `crates/ynz-driver/tests/fixtures/m3_missing_return.ynz` (compile error)
- `crates/ynz-driver/tests/fixtures/m3_return_no_value_in_int.ynz` (compile error)
- `crates/ynz-driver/tests/fixtures/m3_return_value_in_nothing.ynz` (compile error)
- `crates/ynz-driver/tests/fixtures/m3_dead_code.ynz` (warning, exits 0)
- `crates/ynz-driver/tests/fixtures/m3_duplicate_function.ynz` (compile error)
- `crates/ynz-driver/tests/fixtures/m3_undefined_function.ynz` (compile error with Levenshtein)
- `crates/ynz-driver/tests/fixtures/m3_arg_type_mismatch.ynz` (compile error)
- `crates/ynz-driver/tests/fixtures/m3_arg_arity_mismatch.ynz` (compile error)
- `crates/ynz-driver/tests/fixtures/m3_loop_var_mutation.ynz` (compile error)
- `crates/ynz-driver/tests/fixtures/m3_is_type_deferral.ynz` (M6 deferral)
- `crates/ynz-driver/tests/fixtures/m3_share_param_deferral.ynz` (M4 deferral)
- `crates/ynz-driver/tests/fixtures/m3_range_outside_for_deferral.ynz` (M7 deferral)
- `crates/ynz-driver/tests/fixtures/m3_match_keyword_banned.ynz` (teaching diagnostic)
- `crates/ynz-driver/tests/fixtures/m3_switch_keyword_banned.ynz` (teaching diagnostic)
- `crates/ynz-driver/tests/__snapshots__/m3_*.{stdout,stderr}.snap` (per fixture, both streams)

**Catch-up fixtures (committed with `// CATCH-UP <milestone>` markers)**:
- `m3_is_type_deferral.ynz` — `is Circle =>` form → "narrowing on unions lands in M6"
- `m3_share_param_deferral.ynz` — `share player: Player` → "ownership annotations land in M4"
- `m3_range_outside_for_deferral.ynz` — `let r = range(0, 10)` → "Range as first-class iterable lands in M7"
- `m3_break_undefined.ynz` (optional) — `break` → "undefined identifier `break`" with Levenshtein. Documents that M3 doesn't reserve `break`/`continue`. Trigger to revisit: a real program needing them.

**Steps**:
1. Wire the driver to handle M3 sources. Most of the work is in P1–P4; this phase adds fixtures and snapshots.
2. Run each fixture through the build/run pipeline; capture stdout/stderr; commit snapshots.
3. For each negative fixture, snapshot stderr exactly (byte-for-byte modulo ANSI colors disabled per M1).
4. For each catch-up fixture, the stderr snapshot captures the CURRENT deferral error. M4/M6/M7 will update these as they close their catch-up entries.
5. WHY-comments on every test.

**Acceptance criteria**:
- [ ] Every M3 fixture produces the expected stdout / stderr.
- [ ] Headline fixtures (`m3_fib`, `m3_mutual_recursion`, `m3_while_countdown`, `m3_for_range`, multi-case happy paths) exit 0.
- [ ] Every negative fixture exits non-zero with the expected stderr.
- [ ] Catch-up fixtures are clearly marked.
- [ ] M1 + M2 integration tests still pass.
- [ ] Banned-jargon audit passes on every M3 diagnostic.

**Quality gate**:
- [ ] No `unwrap()` in driver changes.
- [ ] Every stderr snapshot was reviewed for three-part WHAT/WHAT-INSTEAD/WHY structure during PR review (recorded in the PR description).

**Verification**: `cargo test -p ynz-driver integration::m3` passes on Linux + macOS.

---

### Phase 6: M3 verification sweep + tag `v0.1.0-m3`
**PR scope**: No new features. TODO sweep. Comment-rules sweep. M3 explicit-non-goals audit. Catch-up list audit. Spec corrections (any that surfaced during P1–P5). CHANGELOG entry. Tag.
**Branch**: `chore/m3-verification`
**Est. lines**: ~80
**Ships via**: `/release`
**Objective**: M3 can be tagged without regret. Catch-up entries are unambiguous so M4 / M6 / M7 cannot accidentally orphan them.

**Steps**:
1. **Broad TODO sweep** (same grep as M1/M2):
   `rg -i 'TODO|FIXME|HACK|XXX|TEMP|PLACEHOLDER|acceptable for now|works in current state|fine until|we.?ll revisit|for now|good enough for the MVP|executor will figure' crates/`
   Migrate findings to plan files / `.claude/todos.md` / delete. Zero results required.
2. **Comment rules sweep** (per `~/.claude/rules/comments.md` Hard Rules):
   - `rg '// ──|// ───|// ════' crates/` returns empty (no section banners).
   - Spot-check for "what" comments describing the obvious; delete them.
   - Verify all `// SAFETY:` comments on `unsafe` blocks are intact.
   - `cargo clippy --workspace -- -D warnings` clean.
3. **Catch-up list audit**:
   - For each entry in the M3 catch-up list (M4 share/lend/give annotations, M6 is-type / options-variant narrowing, M7 range-as-iterable), verify (a) a fixture exists exercising the deferral, (b) a stderr snapshot captures the diagnostic, (c) the owning milestone is unambiguously named in the diagnostic body AND in this plan file AND in the v0-1-compiler umbrella's roadmap entry.
4. **M3 explicitly-NOT list audit**: confirm nothing slipped in. Variant-count tests for `Token`, `Stmt`, `Expr`, `Type`, `MatchPatternKind` confirm mechanically; this is a sanity audit.
5. **Spec-correction verification**: if P1–P5 surfaced any spec drift (e.g., functions.md's ownership-annotation examples vs. the M4 deferral), commit corrections in this PR. Document each correction in the CHANGELOG.
6. **Quality checklist verification**: run through the M3 checklist below; each item checked with evidence (file path, test name, or grep + result).
7. Add `CHANGELOG.md` entry for M3.
8. Bump `Cargo.toml` workspace version to `v0.1.0-m3` (per `/release` skill).
9. Tag `v0.1.0-m3` after merge.

**Acceptance criteria**:
- [ ] TODO sweep returns zero matches.
- [ ] Comment-rules sweep clean.
- [ ] Catch-up list audit passes — every deferred feature has fixture + snapshot + named owner.
- [ ] M3 "explicitly NOT" list audited; no slips.
- [ ] Spec corrections (if any) committed in this PR.
- [ ] Quality-checklist items below ticked with evidence.
- [ ] CHANGELOG entry committed.
- [ ] Git tag `v0.1.0-m3` created.

**Verification**: `git tag -l v0.1.0-m3` returns the tag. The TODO and comment-rules greps return empty.

---

## Quality Checklist (verify at completion of M3)

M3 inherits every item from `v0-1-compiler.md`'s "Shared Quality Checklist". Additional M3-specific items:

- [ ] M3 representative source (multi-function, control-flow, recursion) type-checks clean and runs.
- [ ] Fibonacci fixture prints `55`.
- [ ] Mutual recursion fixture compiles and runs.
- [ ] All control-flow fixtures (while, for-range, multi-case int/string/else) compile and produce expected output.
- [ ] Every M3 negative fixture exits non-zero with byte-for-byte-matching stderr.
- [ ] Reachability analysis catches: missing returns in non-nothing functions; dead code after returns; non-exhaustive multi-case fall-through in non-nothing.
- [ ] Parameter mutation (assignment to a param) is a compile error pointing at M4.
- [ ] `is Type` and options-variant multi-case forms emit M6 deferral diagnostics.
- [ ] `share`/`lend`/`give` parameter annotations emit M4 deferral diagnostics.
- [ ] `Type::Range` outside the for-loop iterable position emits M7 deferral.
- [ ] `match` and `switch` keywords emit teaching diagnostics.
- [ ] Two-pass typeck design: `module_signatures` is its own salsa query, `check` depends on it.
- [ ] Variant-count tests (`Token` → 49, `Stmt` → 8, `Type` → 8, `MatchPatternKind` → 3) all pinned with `// test-ratchet:` markers.
- [ ] Object-file SHA-256 reproducibility contract holds for every M3 fixture per target triple.
- [ ] `ynz_string_eq` runtime shim exported from `libynz_rt.a` (verified via `nm`).
- [ ] M3 catch-up fixtures committed with current-state stderr snapshots; each names its owning milestone (M4 / M6 / M7) in the diagnostic body.

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each phase is one PR on its named branch (`feat/m3-lexer`, `feat/m3-parser`, etc.). Phase scope is bounded by the phase block here.
- **Shadow main branches**: each phase merges to `main` before the next starts. No long-lived `m3` umbrella branch. Phases that go stale within a session get merged or deleted at session end — they don't accumulate.
- **Building the engine before shipping value**: every M3 phase produces a usable checkpoint. P1 alone is a lexer change with no value; P2 alone parses but doesn't run; P3 alone type-checks but doesn't run; P4 first produces runnable M3 binaries; P5 ships the integration suite. **Honest disclosure**: P1–P3 are infrastructure ahead of value (same pattern as M1 P2 and M2 P1–P4). Acceptable because the milestone's end-to-end value lands at P4 — three sessions of infrastructure is the cost of teaching-quality control-flow diagnostics done right.
- **Hotfix that isn't**: N/A — no production users.
- **Abandoned branches**: each phase is single-session-scoped. Stale branches get merged or deleted at session end.
- **Flag graveyards**: N/A — the compiler doesn't use feature flags. Deferrals (M3 → M4 / M6 / M7) are tracked via catch-up fixtures with named-milestone owners, not flags.

---

## Out-of-Scope For This Milestone (M3 guardrails)

Do NOT slip these in:

- **Ownership annotations** (`share`, `lend`, `give`) on parameters — M4. M3 ships read-only parameters only.
- **User-defined types** (`type Foo { ... }`), methods on user types — M4.
- **Type-attached constants** (`int.max`, `int.min`) — M4 (carried from M2).
- **Overflow escape methods** (`.wrappingAdd`, `.saturatingAdd`) — M4 (carried from M2).
- **General method dispatch** (any method beyond M2's hardcoded intrinsic table) — M4.
- **Generics**, **collections** (`array`, `fixed`, `map`) — M5.
- **`<>` generics syntax** in the parser — M5 (docs updated 2026-05-13).
- **`is Type` narrowing** in multi-case `if` — M6 (deferral diagnostic in M3).
- **`option_variant =>`** in multi-case `if` — M6 (deferral diagnostic in M3).
- **`options` declarations** — M6.
- **Union types** (`A or B`) — M6.
- **`maybe T`** — M6.
- **Fallible conversions** (`.toInt()` on number/float, string-to-numeric parsing) — M6 (carried from M2).
- **Full Unicode strings** (`.get`, `.byteAt`, `.graphemeAt`, interpolation) — M7. M3 strings remain UTF-8 byte arrays.
- **`Iterable[T]` protocol**, `FallibleIterable[T]`, full `for` over collections — M7 (M3 has only `for x in range(...)` via the temporary builtin).
- **`Range` as a first-class value** — M7. `Range` exists only in for-loop iterable position in M3.
- **The `errors` keyword and cascades** — M7.
- **Modules**, `import`/`export` — M8.
- **Doc comments** (`///`) parsed and preserved on signatures — M8.
- **Sensitive type modifier** — M8.
- **Concurrency keywords** (`wait`, `background`) — M8.
- **Bignum `number[N]` for N > 34** — M8 (carried from M2).
- **`break` and `continue`** in loops — deferred indefinitely; revisit when a real Yinz program needs them and can't refactor to early-return.
- **`ynz watch`, `ynz fmt`, LSP** — v0.2.
- **`ynz test`, the test runner** — v0.13.

If a phase below feels like it's drifting into any of the above, STOP and re-plan.

---

## M3 Catch-Up Obligations (recorded so downstream milestones don't orphan them)

- **M4 must catch up**:
  - Replace M3's "parameters are read-only" with `share` / `lend` / `give` ownership annotations.
  - Update `m3_share_param_deferral.ynz`'s stderr snapshot when `share` actually works.
  - Add overflow escape methods on `int` (carried from M2).
  - Add type-attached constants (`int.max`, etc.) (carried from M2).
  - Generalize `PrimitiveIntrinsicTable` to a general method-dispatch mechanism.

- **M6 must catch up**:
  - Implement `is Type` narrowing for multi-case `if` on union types.
  - Implement options-variant matching for multi-case `if` on `options` types.
  - Add exhaustiveness checking for options/unions multi-case (per spec: "the compiler verifies every case is handled").
  - Update `m3_is_type_deferral.ynz`'s stderr snapshot when `is Type` actually works.

- **M7 must catch up**:
  - Replace `range` builtin with the `Iterable[T]` protocol.
  - Allow `Range` (or whatever it becomes) as a first-class value: assignable, passable, returnable.
  - Generalize `for x in expr` to accept any `Iterable[T]`.
  - Update `m3_range_outside_for_deferral.ynz`'s stderr snapshot when `Range` becomes a first-class value.
  - Remove the M3 special-cases in typeck `check_for_loop` and codegen `lower_stmt_for`.
  - **Replace `ynz_string_eq` byte-equality with Unicode canonical equivalence** for multi-case string matching. M3 ships byte-equality via `strcmp`, which works for ASCII and for the current null-terminated UTF-8 byte model, but `"café"` (NFC) vs `"café"` (NFD) compare unequal under bytes / equal under canonical form. M7's full Unicode work owns this fix. No new fixture required in M3 (no M3 program produces NFD strings); M7 adds a fixture exercising the canonical-equivalence path.

---

## Reviewer Disputes

(none yet — populated during Step 7 review iterations if/when the reviewer pushes back)
