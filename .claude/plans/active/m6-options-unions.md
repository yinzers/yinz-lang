---
slug: m6-options-unions
owner: patrick
status: active
files:
  - crates/ynz-ast/**
  - crates/ynz-parser/**
  - crates/ynz-typeck/**
  - crates/ynz-codegen/**
  - crates/ynz-diagnostics/**
  - crates/ynz-driver/tests/fixtures/**
  - examples/basics/src/main.ynz
  - examples/errors/m6_errors.ynz
  - design/options.md
  - design/unions.md
  - design/narrowing.md
  - spec/options.md
  - spec/unions.md
  - spec/maybe.md
  - .claude/state.md
  - .claude/todos.md
  - .claude/plans/active/v0-1-compiler.md
  - Cargo.toml
created: 2026-05-18
last_updated: 2026-05-18-r2
depends_on: m5-generics
---

# Plan: M6 — Options + Unions + Narrowing

Created: 2026-05-18
Status: approved 2026-05-18 (r2 after plan-reviewer round 1 PASS)

## Context & Why

**Goal.** Ship M6 of the v0.1 compiler — the milestone that introduces type-driven discrimination. When M6 ships, users can declare `options Status { active, inactive, banned }` and use `Status.active` as a value; declare union types `shape Shape = Circle | Square | Triangle` and discriminate them with `if (x is Circle)` or in multi-case `if (x) { is Circle => ... }`; rely on exhaustive-multi-case checking for both options and unions; carry through the `.value` narrowing on `maybe<T>` past early returns and through short-circuit boolean operators; and convert numbers ↔ strings ↔ ints with `.toInt()` / `.toNumber()` / `.toFloat()` returning a safe `maybe<T>`.

**Why now.** M5 just shipped (`tag v0.1.0-m5`, 574 tests). The remaining v0.1 surface is options/unions/narrowing (M6), strings/errors/iterables (M7), and modules/polish (M8). M6 is the structural prerequisite for M7's `errors` keyword (which is a flow-sensitive narrowing analysis on a union with a designated "error" arm) and unblocks every stdlib module that needs `options` for configuration types or unions for variant returns.

**Background.** M3 reserved the `is Type =>` and `variant =>` multi-case arm forms with stand-in `String` payloads and a parser-level deferral diagnostic pointing to M6 (`MatchPatternKind::IsType(String)`, `MatchPatternKind::Variant(String)`). M5 shipped the narrow positive/negative/AND form of `.exists()` narrowing for `maybe<T>` but explicitly left early-return narrowing and `||` propagation to M6 (per `design/maybe.md` flow-sensitive rules table). M2 reserved fallible numeric conversions to land "when `maybe<T>` exists, which is M5+M6 work" (per the M2 catch-up list).

**Constraints.**
- Compiler implementation language: Rust stable.
- LLVM 18 via inkwell; no changes to runtime ABI of M5-shipped types.
- Diagnostics follow WHAT/WHAT-INSTEAD/WHY per `design/compiler-errors.md`.
- Internal Rust AST naming stays `MatchPattern` / `MatchPatternKind` / `MatchArm` (Patrick: "if it is internal it is fine as long as it's consistent"). The TWO existing stub variants get widened in place: `IsType(String) → Is(TypePath)` and `Variant(String) → OptionName(String)`. No surrounding rename.
- All new user-facing diagnostics audited against `crates/ynz-diagnostics/src/banned_jargon.rs`.
- Union LLVM layout follows a mechanical decision table (mirrors `design/maybe.md`).

**Success criteria for M6:**
- `examples/basics/src/main.ynz` extended with an M6 section showing: an `options` declaration + value use; a union type + `is` narrowing in a multi-case `if`; an options multi-case; fallible conversion (`"42".toInt()` → handled `maybe int`); early-return narrowing on `.exists()`. The whole file still runs end-to-end.
- `examples/errors/m6_errors.ynz` created and intentionally triggers every new compile-error class M6 introduces (non-exhaustive options multi-case, non-exhaustive union multi-case, `is` on a non-union scrutinee, accessing `.value` after a falsy early return without negation narrowing — actually, that ONE works now; etc.).
- `m3_is_type_deferral.ynz` updated: deferral diagnostic gone; the fixture is now a runnable union example with stdout snapshot.
- `m2_*_parse_deferred.ynz` fixtures updated: deferral diagnostic gone; runnable `.toInt()` example with stdout snapshot.
- All M5 tests still green (574+ from M5 baseline, plus M6 additions).
- `Cargo.toml` bumped to `0.1.0-m6` and `v0.1.0-m6` tag created at end of M6.

---

## Research Findings

**Master plan scope for M6** (from `.claude/plans/active/v0-1-compiler.md:190-194` via /peek):

> Milestone 6 (M6): Options + unions + narrowing — multi-session
> `options Status { ... }` declarations, union types `A | B`, `if (x is Type)` pattern narrowing as a flow-sensitive analysis. (`maybe<T>` moved to M5 — see master plan note above.) Early-return narrowing for `.value` on `maybe<T>` (deferred from M5) lands here too.
> Depends on: M5

**M5's explicit deferral to M6** (from `design/maybe.md:87`):

> `if (m.exists()) { return ... } m.value` | NO — early-return narrowing is M6 | Teaching error points to M6.

**M3's REPLACE-AT markers** (from `crates/ynz-ast/src/nodes.rs:213-217`):

```rust
/// `is TypeName =>` — type-narrowing form, deferred to M6.
// REPLACE-AT M6: widen String to TypePath for narrowing
IsType(String),
/// `variant_name =>` — options-variant form, deferred to M6.
// REPLACE-AT M6: widen String to VariantPath for options exhaustiveness
Variant(String),
```

**M2's catch-up obligation** (from `.claude/plans/done/m2-literals-arithmetic.md:62`):

> M6 must catch up: `.toInt()` on number/float (returns `maybe int`); `string.toInt()` / `string.toNumber()` / `string.toFloat()` (return `maybe T`); compile-error suggestions for mixed-type arithmetic involving these fallible directions.

**Existing lexer state** (from `crates/ynz-parser/src/lexer.rs:303-360`):

- `match` and `switch` already banned with three-part hint
- `enum` banned with hint pointing to `options` (line 354)
- `none` is already `Token::None` (M5 shipped this)
- `options` and `is` are NOT yet keywords — they parse as `Token::Identifier`
- The `|` operator already exists for bitwise OR (M2); reused in type position for unions (parser context disambiguates)

**Existing token set**: 57 tokens shipped through M4, plus M5's additions. M6 adds 2 new tokens (`Options`, `Is`). The `|` is already lexed; union-type usage is a parser-context distinction.

**Vocabulary rule audit** (from `.claude/rules/vocabulary.md`): user-facing diagnostics use "options" / "variant" / "union" / "narrows to / narrowed". Banned in error text: `enum`, `match`, `switch`, `tag`, `discriminant`, `tagged union`. Internal AST naming exempt.

**Auto-promotion analysis** (per `.claude/rules/auto-promotion.md`, mandatory):

1. **Union layout — pointer-niche vs tagged struct**: YES, auto-promote. Compiler picks per concrete variant set: if all variants are heap-allocated shapes (no value-type variants, no `none` variant in the form `T | none` which is just `maybe<T>`), use pointer-niche on the data slot (mirrors `design/maybe.md` rule for heap shapes). Otherwise tagged struct `{ i8 tag, [maxSize x i8] payload }`. **Codegen surface only in M6** (no user-typeable opt-in — layout is non-observable, like SSO threshold). No muted hint, no Tier 3 lint. Documented in `design/unions.md` as the locked decision table.
2. **Options as i8 tag**: trivial — every options type ≤256 variants gets `i8`. No auto-promotion opportunity beyond the obvious. >256 variants → typeck compile error suggesting "split into multiple options or model as int"; >256 is almost certainly a code smell.
3. **`is`-narrowing on union with single concrete type**: if a `maybe<Shape>` is narrowed by `if (m.exists()) { ... }` and `Shape` is a concrete (non-union) type, no codegen difference vs the M5 `.value` path. No auto-promotion opportunity.
4. **Exhaustive multi-case → jump table**: per `design/control-flow.md:68`, the compiler already lowers multi-case `if` over `int` to LLVM `switch`. M6 extends this for: options (tag is i8, switch is dense — natural jump table); unions (tag is i8, payload-extract per arm). No auto-promotion to flag — the optimal codegen IS the only codegen.
5. **Narrowing reach**: positive `is`, negative `is`, early-return narrowing, `&&` propagation, `||` partial-propagation, reassignment-invalidation. These are correctness, not auto-promotion. The teaching surface is the diagnostic when narrowing fails: every failure points to the specific rule that wasn't met. No lint needed in M6.

**Override-direction analysis for union layout**: no user-typeable override syntax. Pointer-niche vs tagged struct is implementation-detail; users see only `A | B | C`. Adding `dense<A | B>` / `tagged<A | B>` opt-in surface would be duct tape. Deliberate no-override, documented in `design/unions.md`.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `|` operator overload (bitwise-OR vs union-type) creates parser ambiguity | Medium | Wrong AST for user code; subtle bug class | Parser context: `|` is union-type ONLY in Type position (after `=` in `shape Name = ...` and in type annotations). In expression position, `|` stays bitwise-OR. Adversarial fixture: `let x: int = a | b` (bitwise) vs `shape S = A | B` (union) vs `function f(x: A | B) -> int { return x as_int }` (param-type union). |
| Narrowing flag carries across `||` incorrectly when only ONE branch proves the condition | High | `.value` allowed where it could be `none` → runtime crash | Locked semantics table in `design/narrowing.md`: `||` propagates the flag ONLY when BOTH operands prove the same narrowing (rare in practice). Adversarial fixture asserts `(m.exists() || other)` does NOT narrow `m`; only `(m.exists() && other)` does. |
| Early-return narrowing miscomputes the "rest of the block" scope | High | False positive (rejects valid code) OR false negative (allows `none` access) | Implementation walks the AST AFTER the early-return statement within the SAME scope; narrowing flag invalidated by ANY reassignment, function call that takes `lend self`, or scope exit. Fixtures cover: simple early-return, nested-if early-return, loop with early-return, return-from-multi-case-arm. |
| Exhaustiveness check on union with `extends` chain (e.g., `Admin extends User` in `Admin \| User`) misclassifies | Medium | Wrong "case not handled" or wrong "unreachable arm" diagnostic | Per `spec/unions.md:39-55`: in unions, `is` is exact-type match (no subtype). Exhaustiveness checks each declared variant by name (not by subtype). Fixture: `shape AnyUser = Admin \| Guest \| User` with all three `is` arms is exhaustive; missing one is an error; `Admin extends User` does NOT make `is User` cover `Admin`. |
| `OptionName(String)` AST variant resolved against wrong options type when scrutinee type is union of two options types | Low | Confused diagnostic; wrong arm picked | Typeck rule: `OptionName` arms allowed ONLY when scrutinee is a single options type. If scrutinee is `OptionsA \| OptionsB`, user must use the fully-qualified `OptionsA.foo =>` form (an `Is` arm with the variant path), OR refactor to two nested multi-cases. Diagnostic explains both paths. |
| Shorthand options resolution (`desc` → `SortOrder.desc`) collides when two visible options types both define `desc` | Medium | "Ambiguous" diagnostic better than wrong silent pick | Typeck rejects with three-part error citing both candidate types and requiring qualification. Fixture: declare two options types with the same variant name in scope; assert the diagnostic. |
| Tagged-struct payload alignment wrong on heterogeneous variants | High | UB, crashes on certain variant sequences | Layout rule: payload size = max(sizeof variant) rounded up to alignof(largest variant). Alignment = max(alignof variant). LLVM struct emitted with explicit padding. IR-snapshot test for `Circle \| Triangle` where `Triangle { base: number, height: number }` is larger and 16-byte aligned. |
| Pointer-niche layout broken when one variant could legitimately be a null pointer (e.g., `Shape \| none` where Shape is a heap shape) | High | "None" interpreted as a valid Shape pointer → segfault | The pointer-niche encoding for unions is ONLY used when no variant is `none` itself; `T \| none` is exactly `maybe<T>` (already handled by M5's lowering table). Compiler refuses to apply niche when any variant is `none`. Snapshot test asserts. |
| M5's `m5_maybe.ynz` test using `.value` inside positive `if (m.exists())` blocks regresses when early-return narrowing lands | Medium | Existing fixtures fail | Run full test suite after each P3/P4 phase; M5 fixtures must stay green; new fixtures added for the additional narrowing forms M6 enables. |
| Fallible `.toInt()` semantics on float/number out-of-range, NaN, or fractional input | High | Silent-wrong-output: codegen using `fptosi.sat` returns `some(i64::MAX)` for `1e30` (spec: `none`) and `some(0)` for NaN (spec: `none`) | LOCKED RULE (P0 doc — `design/numeric-types.md` extension): `(float).toInt() -> maybe<int>` returns `none` for NaN, ±Inf, or magnitude > `i64::MAX` as a real; otherwise returns the truncated-toward-zero integer. `(number).toInt() -> maybe<int>` same rule against decimal128 → i64. `.toInt()` does NOT truncate the fractional part silently on string input but DOES on float/number input (spec asymmetry, lock with rationale: float/number values are already-numeric so truncation is intuitive; string `"42.5".toInt()` is a parse failure because the user wrote a non-integer literal — see string row below). MANDATORY codegen sequence (P4 must emit verbatim, asserted via IR snapshot): (1) `is_nan = fcmp uno x, x` → if true, return `{has_value: 0}`; (2) compare `x` to `i64::MAX_AS_F64` (= 9.223372036854776e18) and `i64::MIN_AS_F64` (= -9.223372036854776e18) → if out of range, return `{has_value: 0}`; (3) `result = fptosi x to i64` (RAW, NOT `fptosi.sat`, because we already proved in-range); (4) return `{has_value: 1, value: result}`. Test vectors: `(2.5).toInt() == some(2)`, `(-2.5).toInt() == some(-2)`, `(0.5).toInt() == some(0)`, `(-0.5).toInt() == some(0)`, `(1e30).toInt() == none`, `(-1e30).toInt() == none`, `(NaN).toInt() == none`, `(+Inf).toInt() == none`, `(-Inf).toInt() == none`. |
| String → numeric conversion semantics under-specified and contradicting Rust's `str::parse` | High | `"  +42  ".toInt()` rejected by `str::parse::<i64>` but spec wants `some(42)` → silent disagreement between two parts of the plan | LOCKED RULE (P0 doc — `design/numeric-types.md` extension, "String-to-numeric parsing"): runtime functions explicitly implement the steps (NOT `str::parse` blindly): (a) strip ONLY ASCII whitespace characters [0x20 space, 0x09 tab, 0x0A LF, 0x0D CR] from both ends of the byte slice (NOT Unicode whitespace — keeps behavior platform-independent and predictable); (b) accept optional leading `+` or `-` (single character, no other prefix); (c) reject empty string after whitespace strip; reject lone sign with no digits; (d) `.toInt()` requires the remainder be `[0-9]+` ONLY — no `0x`, `0o`, `0b` prefix; no decimal point; no scientific notation; out-of-range → `none`; (e) `.toFloat()` and `.toNumber()` additionally accept `[+-]?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?` — decimal point + scientific notation OK; `0x`/`0o`/`0b` still rejected; (f) any input failing the above → `none`. Implementation in `ynz-runtime` does this step-by-step (not via `str::parse`), and tests assert against a fixed test-vector table. Test vectors: `"42".toInt() == some(42)`, `"  +42  ".toInt() == some(42)`, `"-42".toInt() == some(-42)`, `"".toInt() == none`, `"  ".toInt() == none`, `"+".toInt() == none`, `"-".toInt() == none`, `"0x1A".toInt() == none`, `"42 hello".toInt() == none`, `"42.5".toInt() == none`, `"1e3".toInt() == none`, `"99999999999999999999".toInt() == none` (out-of-range), `"\u{00A0}42".toInt() == none` (non-breaking space is multi-byte UTF-8; NOT in the ASCII-whitespace strip set; correctly rejected), `"\t42\n".toInt() == some(42)` (tab + LF ARE in the ASCII-whitespace strip set). For `.toFloat()`: `"1.5e2".toFloat() == some(150.0)`, `"  -1.5  ".toFloat() == some(-1.5)`, `"abc".toFloat() == none`, `"1.5.5".toFloat() == none`. |
| Adding two new tokens (`Options`, `Is`) breaks user identifiers `options` / `is` if they appeared in any test fixture or example | Low | Compile error in unrelated test | Pre-check: grep all `.ynz` files for identifiers named `options` or `is`; rename if found before P1 lands. |
| `Token::Is` collides with the `In` keyword in lexer-time longest-match (both 2-char) | Low | Lexer ambiguity | Both are exact-match keywords keyed on the full identifier string; no longest-match conflict (`is` and `in` differ at position 1). No mitigation needed. |
| Designing `design/options.md`, `design/unions.md`, `design/narrowing.md` opens debates that drag P0 longer than the codegen work | Medium | Schedule risk; design churn during implementation | P0 ships ONLY the locked decisions from this plan (LLVM layouts, narrowing rules table, exhaustiveness rules, fallible-conversion semantics). Anything that needs new debate is a future-doc TODO, not a P0 blocker. |

---

## Risk Assessment & Rollout Strategy

**Risk level: COMPILER-CORRECTNESS-CRITICAL** (Tier A per plan-reviewer rubric). The "no production traffic" framing doesn't apply — the compiler IS the product, and wrong codegen ships to every user. Specific silent-wrong-output failure modes M6 must defend against:

- **Narrowing miscompute** → `.value` allowed where binding is actually `none` → segfault when LLVM dereferences a null payload pointer (or unsafe-read of an uninitialized tagged-union slot).
- **Pointer-niche layout misapplied** to a union where one variant could legitimately have a null-pointer payload → "none" tag interpreted as a valid `*Shape` → segfault on field access.
- **`.toInt()` truncation direction wrong** (saturating vs toward-zero) → silently-different integer in user code; out-of-range inputs that should produce `none` produce `some(i64::MAX)` instead. Class is invisible to happy-path tests.
- **NaN handling in `(float).toInt()`** if codegen relies on raw `fptosi.sat` → `NaN.toInt() = some(0)` (LLVM-defined behavior) instead of `none` (spec-required).
- **String→numeric semantics divergence** (whitespace stripping, sign prefix, scientific notation, empty input) → parser accepts inputs the spec rejects or rejects inputs the spec accepts; bug only surfaces when a user passes the edge case.

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler-only |
| Touches auth/permissions | No | — |
| Raw SQL / literals | No | — |
| Modifies existing data | No | New tokens + new typeck rules + new codegen paths; no migrations |
| Third-party integration | No | — |
| Changes existing endpoints | Yes (minor) | M3's `MatchPatternKind::IsType(String)` and `Variant(String)` widen in place. M3 fixtures depending on deferral diagnostics update. M5's `.value` narrowing rules table expands. All in-tree; no external API surface. |

**Mitigations applied:**
- Comprehensive fixture coverage (positive + negative + adversarial) for each new feature; adversarial set explicitly enumerated in `### Adversarial Test Cases` below.
- IR-snapshot tests for union layout (tagged-struct + pointer-niche cases), exhaustiveness lowering, AND the exact codegen sequence for `.toInt()` (range-check + NaN-check + `fptosi` raw — see P4 step 9).
- Test vectors for numeric/string conversion locked in `design/numeric-types.md` extension at P0 and tested both via `cargo test` (Rust-level) and via runnable fixtures (end-to-end).
- Existing M1–M5 test suites stay green throughout (run full suite after each phase).
- Three-part teaching diagnostics for every new compile-error class; exact diagnostic text for the most counter-intuitive rules (`||` non-propagation, ambiguous-shorthand) locked in P0.

**Rollout plan:** No feature flag (compiler binary; users opt in by installing). Tag `v0.1.0-m6` after all phases pass full test suite + examples run end-to-end + jargon audit clean.

### Adversarial Test Cases (M6-specific, Tier A required)

Fixtures must cover each — P3 + P4 own these per the per-phase Demo & Error Gallery obligation:

1. `shape Circle { ... }; let Circle = something; if (x) { is Circle => ... }` — `is X` namespace resolution. Locked rule (P0 docs): `is X` resolves X in the types-only namespace; same-name bindings are NOT shadowed. The shadowing binding `Circle` is legal per Yinz (no rule against capital-letter bindings) but cannot be used in an `is` arm. Fixture: `m6_neg_is_namespace_shadow.ynz` asserts compile error if `is Circle` could ambiguously refer to either; or succeeds cleanly when only the type meaning is valid (which it always is here).
2. `shape S = Foo | Bar | Foo` — non-adjacent duplicate variant. Fixture: `m6_neg_union_duplicate_nonadjacent.ynz` asserts compile error citing both positions of `Foo`.
3. `function desc(x: int) -> string { ... }; players.sort(p => p.health, desc)` — options shorthand vs same-name function. Locked rule (P0 docs): when a bare identifier resolves to BOTH a function and an options variant via shorthand, the function wins (functions are first-class names; options shorthand is a context-driven fallback). Fixture: `m6_neg_options_shorthand_vs_fn.ynz` asserts that the function resolution wins and produces a type error at the `SortOrder`-expecting call site, with diagnostic naming both candidates.
4. `(NaN).toInt()` — confirm codegen emits explicit `fcmp uno` BEFORE `fptosi`. Fixture: `m6_float_to_int_nan.ynz` asserts `(0.0/0.0).toInt().exists() == false`.
5. `(1e30).toInt()` and `(-1e30).toInt()` — confirm range check returns `none`. Fixture: `m6_float_to_int_out_of_range.ynz` asserts both directions return `none`, not `some(i64::MAX/MIN)`.
6. `"  +42  ".toInt()` — confirm runtime function strips ASCII whitespace AND accepts `+` sign. Fixture: `m6_string_to_int_whitespace_sign.ynz` asserts `some(42)`. Plus `"".toInt() == none`, `"+".toInt() == none`, `"-".toInt() == none`, `"0x1A".toInt() == none`, `"42 hello".toInt() == none`, `"42.5".toInt() == none` (`.toInt()` rejects fractional; user must `.toFloat().or(0.0).toInt()`).
7. `if (!(x is Admin))` where `shape Admin extends User; shape S = Admin | User` — negative narrowing respects exact-type-match consistently with positive form. Fixture: `m6_neg_narrow_extends_negation.ynz` asserts narrowed type is `User` (only) inside the then-block, NOT `User | Admin` (which would defeat the purpose).
8. `if (x is Foo) { ... }` where `x: Foo` already (not union) — redundant-narrowing diagnostic. Locked rule (P0 docs): emit a Tier 3 lint-style suggestion ("the `is Foo` check is always true; the binding's static type is already Foo"). For M6, since lints are v0.4+, the suggestion is emitted as an INFO-level diagnostic (non-blocking) — when v0.4 ships the lint tier, this gets re-categorized. Fixture: `m6_info_is_redundant.ynz` asserts the INFO diagnostic AND that compilation succeeds.
9. `if (m.exists()) { if (someFn(m.value)) { return ... } } m.value` — early-return narrowing through NESTED if's return path. Locked rule (P0 docs): early-return analysis walks the outermost if statement only; nested return paths do NOT propagate narrowing facts out of the inner block to the outer block's tail (the outer if's narrowing fact for `m` IS preserved into the outer tail per the positive `if (m.exists())` form, but the inner if doesn't ADD a fact). Fixture: `m6_narrow_nested_early_return.ynz` proves the documented behavior.
10. Nested multi-case where one arm's body contains another multi-case on a different scrutinee — confirm scoping. Fixture: `m6_narrow_nested_multicase.ynz` proves inner scrutinee's narrowing doesn't leak.

---

## What M6 explicitly is NOT (deferred to later milestones)

- **`Iterable<T>` contract** — M7. M6 does NOT touch iterable protocols.
- **Full Unicode strings (`.byteAt`, `.graphemeAt`, interpolation)** — M7. Strings stay as M5's UTF-8 byte-level type.
- **`errors` keyword with flow-sensitive auto-propagation ("cascades")** — M7. The narrowing infrastructure M6 builds is the prerequisite; M7 specializes it for the error union.
- **Modules (`import`/`export`)** — M8. M6 still requires all options/union declarations to live in the same file as their use sites.
- **Doc comments (`///`)** — M8.
- **Sensitive type modifier** — M8.
- **Concurrency (`wait`, `background`)** — M8 (parse + typeck only; sequential execution).
- **Bignum `number<N>` for N > 34** — M8.
- **Custom user-defined options/union LLVM layout** — never. Layout is implementation-detail; users do not opt in.
- **Closures carrying narrowing flags** — v0.3+ (closures themselves are v0.3+).
- **`is` narrowing through method calls on the narrowed binding** (e.g., `if (x is Foo) { y = transform(x); y.fooMethod() }` where `y` should also be narrowed) — v0.2+ (requires alias analysis the LSP work owns).
- **Custom user types satisfying `follows Iterable<T>`** — v1.0.

---

## Catch-Up Obligations from Earlier Milestones (closed in M6)

**Disk-reality note**: a `grep` over `crates/ynz-driver/tests/fixtures/m2_*.ynz` shows the M2 deferral fixtures that EXIST are `m2_int_max_deferred.ynz`, `m2_wrapping_add_deferred.ynz`, `m2_bignum_deferral.ynz` — NONE of which are string-parse deferrals. The string-parse "catch-up" fixtures the M2 plan called out (`m2_string_parse_deferred.ynz` etc.) were never created. Therefore P5's work is to CREATE the runnable fixtures from scratch (NOT rename existing ones), now that the underlying feature (fallible string conversions) is implemented by P3+P4. The intrinsics table (`crates/ynz-typeck/src/intrinsics.rs:57-68`) confirms NO `.toInt()` entry exists today for any source type — P3 adds them.

| Obligation | Origin | Closed by phase |
|---|---|---|
| `.toInt()` on `int` → identity | M2 plan §62 | P3 adds to intrinsics; P5 ships fixture |
| `.toInt()` on `float` → `maybe<int>` with NaN+OOR rules | M2 plan §62 | P3 adds to intrinsics; P4 emits the locked codegen sequence; P5 ships fixture + test vectors |
| `.toInt()` on `number` (decimal128) → `maybe<int>` | M2 plan §62 | P3 adds to intrinsics; P4 wraps existing ynz-numerics decimal→i64 in maybe; P5 ships fixture |
| `string.toInt()` / `.toNumber()` / `.toFloat()` → `maybe<T>` | M2 plan §62 | P3 adds to intrinsics; P4 implements `ynz_string_to_int/number/float` in `ynz-runtime` per the locked parsing rule; P5 ships fixtures with the locked test-vector table |
| `is Type =>` in multi-case `if` (close M3 deferral fixture) | M3 plan §523, REPLACE-AT M6 marker | P2 widens AST; P3b implements typeck; P5 refreshes `m3_is_type_deferral.ynz` to a runnable fixture |
| Options-variant form (`active =>`) in multi-case `if` | M3 plan §523, REPLACE-AT M6 marker | P2 widens AST; P3a implements typeck for options-arm form |
| Exhaustiveness for options/unions in multi-case | M3 plan §38, design/control-flow.md§58 | P3a (options) + P3b (unions) |
| Early-return narrowing for `.value` on `maybe<T>` | M5 design/maybe.md§87 | P3b implements analysis; P5 ships fixture |
| `\|\|` propagation rule for narrowing (M5 deferred for being M6 work) | M5 plan §1051 | P3b implements; P5 ships negative fixture |

---

## Invariants This Milestone Must Preserve

Per `.claude/rules/plan-invariants.md`. Each subsection lists testable assertions.

### Safety

- An `options` value cannot be constructed except via `OptionsName.variantName` literal or a typed function returning one. No coerce-from-int, no coerce-from-string.
- A union value cannot be accessed as a specific variant without flow-sensitive proof from `is`. The compiler rejects `(x: A | B).foo` outside an `if (x is A)` block.
- Exhaustiveness on options/unions in multi-case `if` is required: missing a variant is a compile error citing the missing variant by name AND offering to add the `else =>` catch-all.
- Narrowing flag invalidated by: reassignment to the binding, scope exit (then-block end), function call that takes `lend self` on the binding, or any path that mutates a field of the binding.
- **Recognized-exit set for early-return narrowing** (LOCKED in `design/narrowing.md`): only `return <expr>`, `return` (in `nothing`-returning function), `panic(msg)`, and `loop { /* no break */ }` (infinite loop — purely forward-defensive; loops are M3-shipped but `break`/`continue` are deferred per M3 plan §117). Any other statement (including `print()`, `someFn()` returning `nothing`, or a call to a `nothing`-returning user function) does NOT count as a recognized exit even if the user "knows" the function diverges — the typeck can't prove it. Diagnostic when user expects narrowing from a non-exit: WHAT/WHAT-INSTEAD/WHY pointing to the recognized-exit list and suggesting the workaround (`if (m.exists()) { use(m.value) } else { /* recognized exit */ }`).
- `.value` on a `maybe<T>` still rejects when narrowing not proven — M5's compile-error rule survives untouched; M6 only ADDs proven paths.
- `OptionName` arm names resolve only against the scrutinee's declared options type; ambiguous shorthand (two visible options types defining the same variant name) is a compile error.
- **Single-variant options is REJECTED** (parallel to single-variant union rejection): `options Foo { only_one }` produces a compile error suggesting either adding a second variant OR using a `const FOO: int = 0` if the user genuinely wants a single named constant. Symmetry with `shape S = A` (single-variant union also rejected) is the rationale — both forms are degenerate and almost always indicate the wrong tool. Locked in `design/options.md`.
- **`is X` namespace resolution**: `is X` looks up `X` in the types-only namespace. A same-name binding in the values namespace (e.g., `let Circle = 5`) does NOT shadow the type lookup. The same-name binding remains usable in expression position; the `is` arm sees only the type.
- **Function vs options-shorthand priority**: when a bare identifier could resolve to either an in-scope function OR an options variant via context-driven shorthand, the function wins. Locked in `design/options.md`.

### Performance

- Options values lower to `i8` (or smallest type that fits the variant count); arithmetic-free comparison via LLVM `icmp eq`. No vtable, no string compare.
- Multi-case `if` over an options type emits LLVM `switch` (dense jump table — same codegen as M3's `int` multi-case).
- Union layout decision (mechanical table — locked in P0):

| Variant set | LLVM type | Why |
|---|---|---|
| All variants are heap-allocated shapes (`ynz_alloc`-backed) AND no `none` variant | Pointer-niche on `{ i8 tag, *T data }` where tag identifies which `*T` to interpret data as | Tag-only layout; no max-payload waste. Used when every payload is a single pointer. |
| Mixed value-type and heap variants, OR any variant has a value-type payload >1 pointer | Tagged struct `{ i8 tag, [maxPayloadSize x i8] payload, padding }` aligned to max(alignof) | Single representation; payload area sized for largest variant; alignment correct for all. |
| `T \| none` (any T) | NOT a union per M6 codegen — this IS `maybe<T>` and uses M5's lowering | Avoids double-encoding |
| Single-variant "union" (`shape S = A`) | REJECTED at typeck — degenerate | Compile error suggests `shape S = A` is just an alias; remove the union |
| 2 to 255 variants | Tag is `i8` | One byte covers up to 256 distinct variants |
| 256+ variants | Tag is `i16`, but COMPILE ERROR with hint suggesting "this is probably a code smell; consider refactoring" | Avoid 256-variant unions in v0.1; revisit if real demand emerges |

  IR-snapshot test in `crates/ynz-codegen/tests/snapshots.rs` asserts each row produces the expected LLVM type.

- `is Type` check on a union lowers to: load tag byte → compare to expected tag constant. ~2 instructions. No method dispatch.
- Narrowed `.foo` access on a union variant inside a proven `is` block lowers to: extract payload at known offset, cast to variant type, field-access. Inline-able; LLVM will fold consecutive narrowed accesses.

#### Auto-Promotion Analysis (mandatory subsection per `.claude/rules/auto-promotion.md`)

1. **Union layout — pointer-niche vs tagged struct**
   - Stricter form fits when: all variants are heap shapes AND no `none` variant.
   - Codegen-promote? YES — when proven, use pointer-niche (1 byte tag + 1 pointer = 16 bytes on 64-bit, vs N-byte tagged struct).
   - Muted hint? NO — layout is implementation-detail (no user-typeable opt-in; same precedent as SSO threshold, auto-SoA).
   - Tier 3 lint? NO — same rationale.
   - Documented in `design/unions.md` as deliberate no-user-override.

2. **Options tag size — `i8` vs `i16`**
   - Stricter form: `i8` when variants ≤ 256.
   - Codegen-promote? YES — always pick smallest fitting tag.
   - Muted hint? NO — no user-typeable opt-in.
   - Tier 3 lint? NO.

3. **Multi-case `if` lowering to `switch` table**
   - Stricter form: dense jump table over tag.
   - Codegen-promote? YES — M3 already does this for `int`; extend to options + union tags.
   - Muted hint / lint? NO — pure codegen choice.

4. **Narrowing reach (`is`, early-return, `&&`, `||`)**
   - Not an auto-promotion; it's correctness analysis. Teaching diagnostic on failure (already part of the compiler-is-teacher mission).

5. **Override-direction analysis (per `.claude/rules/auto-promotion.md` "Override Patterns — Consider Both Directions")**
   - Force pointer-niche when not provable: NO — would unsafely reinterpret payload. Deliberate omission.
   - Force tagged-struct when niche would work: NO — would waste memory for no benefit. Deliberate omission.
   - Force smaller/larger options tag: NO — no use case; ABI-stable but not user-observable.
   - Naming consistency: N/A (no overrides shipped).

**No new auto-promotion candidates beyond layout and tag-size.** This explicit declaration satisfies the "state explicitly that it was considered, not forgotten" requirement.

### Teaching

- Every new compile-error class follows WHAT/WHAT-INSTEAD/WHY (audited by `crates/ynz-diagnostics/src/banned_jargon.rs`):
  - Non-exhaustive multi-case (options): names every missing variant; suggests both completing them and adding `else =>`.
  - Non-exhaustive multi-case (union): names every missing type variant; suggests both completing them and adding `else =>`.
  - `is Type` on non-union scrutinee: explains that `is` discriminates a union; suggests `shape S = A | B | C` if user wants discrimination.
  - `is Type` with Type not in union: lists the actual union's variants.
  - Accessing `.foo` on a union variable without `is` narrowing: cites union type; suggests `if (x is Foo) { x.foo }`.
  - `OptionName` arm form against non-options-or-union scrutinee: explains the form is for `options` / union types; cites scrutinee type.
  - Ambiguous shorthand options (two visible types with same variant name): cites both candidates; requires qualified form.
  - Function-vs-options-shorthand collision: when a bare identifier resolves to BOTH a function and an options variant, the function wins; if the function's type doesn't match the expected parameter type, the resulting type error cites both candidates and instructs the user to use the qualified form (`OptionsName.variant`) to disambiguate.
  - Early-return narrowing where the "return" is not in the recognized-exit set: explains which constructs ARE recognized (`return`, `panic`, infinite `loop`) and which the user's code uses instead.
  - `.toInt()` / `.toNumber()` / `.toFloat()` runtime parse-failure cases (empty string, out-of-range, non-numeric): all return `none` at runtime, not a compile error; the compile error is using `.value` on the result without checking.
  - `.toInt()` on `bool`: compile error (no fallible conversion from bool); suggests `if (b) { 1 } else { 0 }`.
  - **`is Foo` on a binding whose static type is already `Foo`**: INFO-level diagnostic (non-blocking; precursor to v0.4 Tier 3 lint per `design/linting.md`). WHAT: "this `is Foo` check is always true; `x`'s type is already `Foo`." WHAT-INSTEAD: "remove the `is` check OR remove the surrounding `if` if the body would always run." WHY: "the check adds runtime cost without narrowing; the compiler proves it's redundant." Diagnostic deferred re-categorization to v0.4 lint tier.

- **`||` non-propagation diagnostic text** (LOCKED — exact wording required for P3b implementation):

  > **WHAT**: `.value` is not safe here. The narrowing on `m` from `m.exists()` doesn't carry into this block because `||` only narrows when BOTH operands prove the same fact.
  >
  > **WHAT INSTEAD**: Pick one of:
  >   1. `if (m.exists()) { ... use(m.value) ... }` — narrow before the body.
  >   2. `let safe = m.or(defaultValue); use(safe)` — handle the `none` case with a fallback.
  >   3. If the other condition is a separate concern, split into two `if`s.
  >
  > **WHY**: `||` is true when EITHER operand is true. If `other` is true and `m.exists()` is false, the body still runs but `m` is `none` — accessing `m.value` would crash. The compiler enforces this even when you "know" both will be true together, because the safety check costs nothing and the bug class costs a lot.

  This is the only narrowing rule users get wrong, and the diagnostic IS the teaching surface for it.

- **Ambiguous-shorthand options diagnostic text** (LOCKED):

  > **WHAT**: `desc` is ambiguous — it's a variant of both `SortOrder` and `Direction` in scope here.
  >
  > **WHAT INSTEAD**: Use the qualified form: `SortOrder.desc` or `Direction.desc`.
  >
  > **WHY**: Shorthand resolution requires a unique match against the expected type. When two visible options types define the same variant name, the compiler refuses to guess.

- M3's `m3_is_type_deferral.ynz` deferral diagnostic GONE; the fixture compiles and runs (deferral text replaced with stdout snapshot).
- M2's string-parse "catch-up" fixtures are CREATED IN P5 (not renamed — they don't exist today; see Catch-Up Obligations table). After P5 ships: `m2_string_to_int_basic.ynz`, `m2_string_to_int_vectors.ynz` etc. exist as runnable demonstrations.
- IDE muted hints for narrowing (categorization per `.claude/rules/inference.md`):
  - Inside `if (x is Foo) { ... }`, hover on `x` shows `narrowed to Foo (because of is-check on line N)` — INFORMATIONAL category (no typeable equivalent; comment-style annotation; deferred to v0.2 LSP work).
  - Inside `if (m.exists()) { return ... } m.value`, hover on the auto-propagated narrowing shows `narrowed via early-return on line N` — INFORMATIONAL.
  - No new muted hints ship in M6 (LSP work is v0.2); only the typeck infrastructure that v0.2 will surface. P0 documents the v0.2 surface obligation in `design/narrowing.md`.

### Runtime Dependencies

- `options` declarations and values: NONE (compile-time enum lowered to `i8` constants).
- Union typeck + codegen: NONE (compile-time analysis + LLVM tagged-struct/pointer-niche emission).
- Narrowing analysis: NONE (compile-time flow analysis).
- Fallible conversions `.toInt()` on `number`/`float`/`int`/`string`:
  - `(int).toInt()` → identity; NONE.
  - `(float).toInt()` → uses LLVM `fptosi.sat` with overflow check; NONE additional.
  - `(number).toInt()` → uses ynz-numerics decimal128 → i64 conversion (already shipped in M2); NONE additional.
  - `(string).toInt() / .toNumber() / .toFloat()` → requires runtime string-parsing functions: `ynz_string_to_int`, `ynz_string_to_number`, `ynz_string_to_float`. These compile into `ynz-runtime` as `extern "C"` functions called via codegen. Each returns a `{ has_value: i1, value: T }` struct (matches `maybe<T>` lowering for primitives per `design/maybe.md`). Implementation in safe Rust using `str::parse::<i64>` / `parse::<f64>` and a decimal128 parser from `ynz-numerics`. No new dependencies beyond what M2 + M5 already pulled in.

### Kernel-Mode Behavior

- `options` declarations and values: always work in `--kernel` mode (no allocator, no heap; tags are stack-resident `i8`s).
- Union typeck: always works in `--kernel` mode (compile-time only).
- Union codegen:
  - Tagged-struct layout: always works in `--kernel` mode (stack-resident structs).
  - Pointer-niche layout: requires the heap shapes' allocator to be available. Per M4's kernel-mode rule (heap shapes need user-provided allocator in kernel mode), this is the user's responsibility; M6 introduces no new allocator dependency.
- Narrowing analysis: always works in `--kernel` mode (compile-time only).
- Fallible conversions:
  - `(int/float/number).toInt()` etc.: always work in `--kernel` mode (no heap).
  - `(string).toInt()` etc.: requires the `string` runtime functions. Strings are heap-allocated, so kernel-mode users must already have provided an allocator (M5 obligation). The `ynz_string_to_*` runtime functions perform NO additional heap allocation (parse the bytes already on stack). Always works in `--kernel` mode given the M5 allocator obligation.

### Demo & Error Gallery

Per `.claude/rules/plan-invariants.md` `### Demo & Error Gallery` requirement.

**examples/basics/src/main.ynz** — extended progressively across P3a, P3b, P4, P6 (per the per-phase obligation). Final M6 section covers:
- Declare a small `options Status { pending, active, banned }` and use `Status.active` as a value.
- Print via `Status.active.toString()` (which lowers to a stdlib codegen path returning the variant name).
- Multi-case `if (status) { Status.active => ..., Status.pending => ..., Status.banned => ... }` showing exhaustiveness in practice.
- Declare `shape AccountResult = ActiveAccount | BannedAccount | PendingAccount` (three small heap shapes).
- A `function classify(account: AccountResult) -> string { if (account) { is ActiveAccount => return "OK", is BannedAccount => return "DENIED", is PendingAccount => return "WAIT" } }` showing union narrowing.
- Early-return narrowing using a CONCRETE-and-shipped function: take an existing M5 collection method that returns `maybe<T>` (e.g., `.first()` on `array<int>`):
  ```ynz
  function firstOr(nums: share array<int>) -> int {
    let r = nums.first()       // maybe<int> per M5 collection-method signatures
    if (!r.exists()) { return -1 }   // recognized exit
    return r.value             // M6 early-return narrowing proves this safe
  }
  ```
  (No invented `lookup` function — `.first()` is shipped in M5 per `crates/ynz-typeck/src/intrinsics.rs` collection-method registration.)
- Fallible conversion: `let parsed = "42".toInt(); print(parsed.or(0))`.

**examples/errors/m6_errors.ynz** — create in P6, intentionally trigger:
- Non-exhaustive options multi-case (missing variant)
- Non-exhaustive union multi-case (missing variant type)
- `is Type` on a non-union scrutinee
- `is Type` with Type not in scrutinee's union variant list
- `.foo` access on a union variable without `is` narrowing
- `OptionName` arm form against a non-options scrutinee
- Ambiguous shorthand options (declares two options types with `desc` variant in scope, then attempts to use `desc` unqualified)
- "Early-return" where the early-return path doesn't actually return (just prints) — must reject
- `||` form where narrowing is claimed but `||` doesn't propagate the flag
- 256+ variant options type (compile error)
- Single-variant union `shape S = A` (compile error suggesting remove the union)
- Comparison between two different options types (`Status.active == OtherOptions.first`) — typeck error
- `.toInt()` on something the spec doesn't allow (e.g., a bool)

Each error in the file has a `// WHY:` comment naming the diagnostic class. The file is run via the existing `examples/errors/m{N}_errors.ynz` snapshot harness in `crates/ynz-driver/tests`.

**Per-phase obligation**: every phase that adds executable surface AND/OR new error classes extends BOTH files in that phase's acceptance criteria (not deferred to P6 alone).

---

## Phases

**Per-phase shipping action** (detected per `/plan` Step 4a): project-local `/pr` skill exists at `.claude/skills/pr/` (Yinz-specific). All phases ship via `/pr` (drafts a milestone-aware PR).

**Per-milestone shipping action**: project-local `/release` skill exists at `.claude/skills/release/`. Milestone wrap-up uses `/release` to bump `Cargo.toml`, generate CHANGELOG section, commit, tag, push.

---

### Phase 0: Doc Lockdown
**PR scope**: New `design/options.md`, `design/unions.md`, `design/narrowing.md` capturing the locked decisions from this plan (LLVM layouts, narrowing rules table, exhaustiveness rules, fallible-conversion semantics). Update `spec/options.md`, `spec/unions.md`, `spec/maybe.md` to match new surface; update master plan's M6 paragraph to mark "in progress." Update `todos.md` per state shifts.
**Branch**: `chore/m6-doc-lockdown`
**Flag**: N/A
**Est. lines**: ~600 (mostly new design doc text + small spec edits)
**Ships via**: `/pr`
**Objective**: Lock M6 design surface before any code lands. Future contributors reading any design doc see the correct M6 scope; the implementation phases reference these locked decisions instead of re-debating them.
**Why this phase exists**: per `no-duct-tape.md` — implementing first and writing design docs second is how design drift creeps in. Lock decisions in design docs first; implementation phases cite them.
**Current-state anchors**:
- `design/maybe.md` — exemplar for the "decision-table-first" design style; M6 design docs mirror this format
- `design/type-system.md:273-321` — currently the ONLY home for union/options/maybe brief notes; M6 splits to dedicated files
- `.claude/plans/active/v0-1-compiler.md:190-194` — M6 master-plan paragraph
- `spec/options.md`, `spec/unions.md`, `spec/maybe.md` — user-spec entry points
- `design/control-flow.md:58-65` — exhaustiveness rule lives here; M6 expands and cross-refs
**Files (expected scope)**:
- NEW: `design/options.md`
- NEW: `design/unions.md`
- NEW: `design/narrowing.md`
- EDIT: `spec/options.md` (clarify exhaustiveness, ambiguous-shorthand rule)
- EDIT: `spec/unions.md` (clarify exhaustiveness, exact-type-in-union rule with example)
- EDIT: `spec/maybe.md` (note M6 expands `.value` narrowing to early-return + `&&` + `||`)
- EDIT: `design/control-flow.md` (cross-ref `design/narrowing.md` for the narrowing analysis)
- EDIT: `design/decisions.md` (add row for M6 design files)
- EDIT: `.claude/plans/active/v0-1-compiler.md` (M6 status → in progress)
- EDIT: `.claude/todos.md` (move M6 catch-up items to "active")
**Deviation rule**: Executor MAY touch other design files for cross-refs; document each deviation in PR description. If a deviation reveals a wider design question (e.g., a spec example uses `match` instead of multi-case `if`), STOP and revise this plan.
**Steps**:
1. Create `design/options.md` with sections: User Spec link; LLVM lowering (i8 tag); Built-in options (`SortOrder`, `Comparison`); Exhaustiveness rule; Ambiguous shorthand rule; `.toString()` for options values (variant name as string); cross-refs.
2. Create `design/unions.md` with sections: User Spec link; LLVM Lowering Decision Table (verbatim from this plan's `### Performance`); `is`-exact-type rule with extends example; Exhaustiveness rule; Single-variant rejection; Cross-refs; "No user-override on layout" deliberate-omission rationale.
3. Create `design/narrowing.md` with sections: User Spec link to relevant pieces of spec/maybe.md and spec/unions.md; Complete flow-sensitive `.value` and `.is Type` rules table (10+ rows: positive `is`, negative `is`, early-return positive, early-return negative, `&&` propagation, `||` non-propagation, reassignment-invalidation, function-call-with-lend invalidation, closure-non-propagation forward-defensive, etc.); LSP muted-hint obligation for v0.2 (informational category).
4. Edit `spec/options.md`: add example of options multi-case; cite the ambiguous-shorthand rule with example.
5. Edit `spec/unions.md`: add example of multi-case `if (x) { is Foo => ... }`; clarify exact-type rule.
6. Edit `spec/maybe.md`: note that M6 expands `.value` narrowing per `design/narrowing.md`.
7. Edit `design/control-flow.md` exhaustiveness section to cross-ref `design/narrowing.md`.
8. Edit `design/decisions.md` index table: rows for `design/options.md`, `design/unions.md`, `design/narrowing.md`.
9. Edit `.claude/plans/active/v0-1-compiler.md` M6 paragraph: status → in progress.
10. Edit `.claude/todos.md`: surface M6 catch-up items.
**Acceptance criteria**:
- [x] `design/options.md`, `design/unions.md`, `design/narrowing.md` exist with all sections listed in Steps 1-3.
- [x] `spec/options.md`, `spec/unions.md`, `spec/maybe.md` updated with M6-relevant clarifications.
- [x] `design/control-flow.md` cross-refs `design/narrowing.md`.
- [x] `design/decisions.md` index table has rows for the three new files.
- [x] Master plan's M6 paragraph shows "in progress" status.
- [x] `.claude/todos.md` lists M6 catch-up items as active.
- [x] Jargon audit (manual grep) clean on all new docs.
- [x] No code touched.
**Quality gate** (check BEFORE moving to next phase):
- [x] Every locked decision in this plan has a corresponding section in one of the three new design docs.
- [x] No "TBD" or "will decide later" markers in P0 docs — every M6 decision is locked here.
- [x] User-facing terms match `.claude/rules/vocabulary.md` (no `match`, `tagged union`, `discriminant`, `enum`).
- [x] At least one example per design doc uses real Yinz operations per `.claude/rules/dot-postfix.md` "Examples-must-use-real-operations rule".
**Verification**: `git diff --stat` shows new design files + spec edits only; no `crates/` touched; full test suite (`cargo test --workspace`) still 574 tests green (no behavior change).

**STATUS: P0 COMPLETE** — staged on `chore/m6-doc-lockdown`, awaiting commit + PR. Next: P1 (lexer).

---

### Phase 1: Lexer (`options`, `is`)
**PR scope**: Add `Token::Options` and `Token::Is` to the lexer; ensure `enum` continues to redirect to `options`; add jargon-audit-only test that diagnostics mentioning `is`/`options` use those words correctly.
**Branch**: `feat/m6-lexer`
**Flag**: N/A
**Est. lines**: ~80
**Ships via**: `/pr`
**Objective**: M6 keywords lex into dedicated tokens (rather than `Token::Identifier`). All M1–M5 tests still green.
**Why this phase exists**: parser and typeck phases need to match on `Tok::Options` / `Tok::Is`; doing the lexer first keeps each phase's diff small.
**Current-state anchors**:
- `crates/ynz-parser/src/lexer.rs:300-371` — keyword dispatch table; add new arms
- `crates/ynz-parser/src/tokens.rs` (or wherever `Token` enum lives — let executor find via grep)
- `crates/ynz-parser/tests/lex.rs` (or equivalent — extend with token tests)
**Files (expected scope)**:
- `crates/ynz-parser/src/lexer.rs` — add `"options" => Token::Options`, `"is" => Token::Is`
- `crates/ynz-parser/src/tokens.rs` (or wherever the Token enum lives) — add variants
- `crates/ynz-parser/tests/lex.rs` — extend lex tests
- `crates/ynz-driver/tests/fixtures/` — NO new fixtures here (typeck phases own them)
**Deviation rule**: If the executor discovers that `options` or `is` already appear as identifiers anywhere in `.ynz` fixtures, fix the fixture (rename to a non-keyword alternative — e.g., `theIs` → `equalCheck`) in this same PR. Document the rename in the PR description.
**Steps**:
1. Grep `crates/ynz-driver/tests/fixtures/*.ynz` and `examples/**/*.ynz` for the identifiers `options` and `is`. If found, rename to avoid the keyword collision (preserving test intent). Stage these renames in this PR.
2. Add `Options` and `Is` variants to the `Token` enum (alphabetic placement among other M-N keywords).
3. Add the two arms to the keyword-dispatch match in `lex_identifier`: `"options" => Token::Options`, `"is" => Token::Is`. Place near the M5 keywords block (after `"none"`).
4. Bump the locked variant-count test for `Token` (if one exists — grep `variant_count_locked` in parser tests).
5. Add lex-level tests: `let tokens = lex("options Status { active }"); assert_eq!(tokens[0], Token::Options); ...` similar for `is`.
6. Run full test suite. All previous tests must still pass.
**Acceptance criteria**:
- [ ] `Token::Options` and `Token::Is` exist in the AST.
- [ ] The lexer produces these tokens for `options` and `is`.
- [ ] Variant-count test (if present) bumped.
- [ ] At least 2 new lex tests assert correct tokenization.
- [ ] Full `cargo test --workspace` still green.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
**Quality gate**:
- [ ] No new tokens beyond Options + Is (no preemptive `Match`/`Enum`/etc.).
- [ ] If fixtures renamed, the rename motivation is captured in PR description.
**Verification**: `cargo test -p ynz-parser` shows new tests passing; `cargo test --workspace` totals 574+ tests + 2 (= 576+).

---

### Phase 2: AST + Parser
**PR scope**: Add AST nodes for `options` declarations, options-value expression (`Foo.bar`), union types in Type position (`A | B | C`), and the multi-case `is Type` and `OptionName` arm forms. Widen M3's stub `IsType(String) → Is(TypePath)` and `Variant(String) → OptionName(String)` per REPLACE-AT markers. Parser produces these nodes from M6 source. NO typeck logic yet — that's P3.
**Branch**: `feat/m6-parser`
**Flag**: N/A
**Est. lines**: ~400 (parser + AST + tests)
**Ships via**: `/pr`
**Objective**: M6 source parses into the correct AST shape. Parser-level diagnostics for obvious malformations (e.g., `options Foo {}` empty body).
**Why this phase exists**: typeck (P3) needs valid AST nodes to walk. Splitting parser from typeck keeps each PR reviewable.
**Current-state anchors**:
- `crates/ynz-ast/src/nodes.rs:204-218` — `MatchPatternKind` enum (widen here)
- `crates/ynz-ast/src/nodes.rs` — `Item` enum (add `OptionsDecl`); `Expr` enum (add `OptionsValue`); `Type` enum (add `Union`)
- `crates/ynz-parser/src/parser.rs` (or wherever `parse_*` functions live) — add parsing for `options` declaration, union type position, `is Type` arm, `OptionName` arm
- `crates/ynz-parser/tests/parse.rs:8` — locked variant-count test for `MatchPatternKind`; bump expected count and update comment
**Files (expected scope)**:
- `crates/ynz-ast/src/nodes.rs` — new variants on `Item`, `Expr`, `Type`, `MatchPatternKind`; new `TypePath` struct if needed
- `crates/ynz-parser/src/parser.rs` (or equivalents) — parsing functions
- `crates/ynz-parser/tests/parse.rs` — extensive new tests (positive + edge cases)
**Deviation rule**: NO typeck logic in this PR. NO codegen changes. The parser may emit "I see this, but typeck will reject" tolerant parses (so error messages can be improved at typeck time). Document any deviations.
**Steps**:
1. Add `TypePath { segments: Vec<String>, span: SourceSpan }` to AST (or reuse if already present from M5 generics work). This represents `SortOrder.desc` / `Status.active` / a bare `Circle` type name.
2. Add `Item::OptionsDecl { name: String, variants: Vec<String>, span: SourceSpan }`. Built-in options live in a stdlib prelude file (P3 typeck wires them).
3. **LOCKED: do NOT add a new `Expr::OptionsValue` variant.** Re-use existing `Expr::FieldAccess` and disambiguate at typeck per P3a Step 1. Rationale: the `Foo.bar` syntax is identical at parse-time whether `Foo` is a shape or an options type; the parser has no way to know which (`Foo` could even be locally shadowed by a `let Foo`). Pushing disambiguation to typeck — where the receiver's type resolution is the authoritative answer — produces ONE handler instead of duplicated logic. AST stays minimal. (Decision locked at planning, not deferred to coding.)
4. Add `Type::Union { variants: Vec<Type>, span: SourceSpan }` for the union-type Type-position form. Parser parses `A | B | C` in Type position (and ONLY Type position) into this.
5. Widen `MatchPatternKind::IsType(String) → Is(TypePath)` (closes M3 REPLACE-AT marker).
6. Widen `MatchPatternKind::Variant(String) → OptionName(String)` (closes M3 REPLACE-AT marker). Bump variant-count test if present; update its `WHY` comment to reflect M6 status.
7. Parser changes:
   - `parse_item` recognizes `options Name { ... }` → `Item::OptionsDecl`.
   - In Type position (function return type, parameter type, let/const type annotation, shape-field type, shape-alias `shape Foo = ...`), parse `|`-separated types into `Type::Union`. Disambiguate from bitwise-OR: in Type position, `|` is always union; in Expr position, `|` is always bitwise-OR.
   - In multi-case-arm-pattern position, `is TypeName` parses to `MatchPatternKind::Is(TypePath)`. A bare `OptionName` (single identifier matching no top-level binding) defers resolution to typeck (parser produces `MatchPatternKind::OptionName(String)`; typeck rejects if scrutinee isn't options or union).
   - Tolerant: `options Foo {}` (empty variants) PARSES but typeck rejects with a "needs at least one variant" message.
8. Parser tests: positive cases for options decl, options value, union type in Type position, `is Type` arm, `OptionName` arm; negative cases for `is` outside multi-case, `|` in wrong position (with helpful error), empty options body (parses; typeck error), trailing comma in options body, duplicate variant name (typeck error, not parser).
**Acceptance criteria**:
- [ ] All AST changes compile and pass `cargo clippy --workspace -- -D warnings`.
- [ ] `MatchPatternKind` variant-count test updated (still 3 variants; renamed two of them).
- [ ] Parser tests cover: options decl (≥3 cases incl. empty body), options value (`Foo.bar`), union type in 3 positions (return type, parameter type, shape alias `shape S = A | B`), `is Type` arm, `OptionName` arm.
- [ ] Negative parser tests: `is` outside multi-case (error), bare `|` in expr position works as bitwise-OR (existing test continues to pass), empty options body parses (typeck owns the rejection).
- [ ] M5 fixtures still parse without error (`m5_array.ynz`, `m5_fixed.ynz`, `m5_map.ynz`, `m5_maybe.ynz`, `m5_identity.ynz`).
- [ ] Full `cargo test --workspace` green; new parser tests add to total.
**Quality gate**:
- [ ] No typeck logic introduced (typeck phase reviews and confirms).
- [ ] No new error classes in `crates/ynz-diagnostics` (parser only emits structural-malformation errors; semantic errors are P3).
- [ ] All examples in design/spec docs still parse.
**Verification**: `cargo test -p ynz-parser` shows new tests; full suite stays green; spot-check one of each new AST variant by printing in a debug binary if needed.

---

### Phase 3a: Typeck — Options + Fallible Conversions
**PR scope**: Options typeck (declaration + value lookup); built-in options registration (`SortOrder`, `Comparison`); options multi-case typeck (exhaustiveness, `OptionName` arms); ambiguous-shorthand resolution including function-vs-shorthand priority; comparison-between-options-types rejection; fallible-conversion typeck (`.toInt()`/`.toNumber()`/`.toFloat()`) added to `PrimitiveIntrinsicTable` for `int`/`float`/`number`/`string` source types. `is`-arm and union-narrowing typeck still defer with the M3 diagnostic (now pointing to "M6 phase 3b" rather than "M6"); P3b closes that.
**Branch**: `feat/m6-typeck-options`
**Flag**: N/A
**Est. lines**: ~450 (options typeck + intrinsics + tests)
**Ships via**: `/pr`
**Objective**: Programs using options + fallible conversions type-check correctly and produce three-part diagnostics for the new error classes. M3's `Variant`-arm deferral fixture (`m3_is_type_deferral.ynz`'s sibling for options arms, if any — verify in step 0) still reports the M6 deferral but with text amended to "see M6 P3b" for `is` arms specifically. P3a is independently shippable: options-only programs work end-to-end after P3a + P4a (codegen for the options subset).
**Why this phase exists**: per plan-reviewer feedback — options typeck (~350 lines), intrinsics extension (~100 lines), and exhaustiveness checking are independent of union typeck. Splitting unblocks options fixture work and lets reviewers focus on each semantic surface separately. The combined P3 would have been ~1200 lines; that's too large for honest review.
**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs:440-456` — current `MatchPatternKind::Value` handling + `IsType`/`Variant` deferral stub
- `crates/ynz-typeck/src/intrinsics.rs:57-68` — existing M2 conversion intrinsics (no `.toInt()` exists today; extend here)
- `crates/ynz-codegen/src/emit.rs:843-846` — codegen `Variant`/`IsType` stub (untouched in P3a; P4 implements)
- `crates/ynz-diagnostics/src/banned_jargon.rs` — jargon audit table
**Files (expected scope)**:
- `crates/ynz-typeck/src/check.rs` — options decl/value typeck, ambiguous-shorthand resolution, multi-case `OptionName`-arm typeck, options exhaustiveness, comparison-between-options rejection
- `crates/ynz-typeck/src/options_table.rs` (NEW) — `OptionsTable` registry + built-in options registration (`SortOrder`, `Comparison`)
- `crates/ynz-typeck/src/intrinsics.rs` — add `.toInt()` for int/float/number, add `.toInt()`/`.toNumber()`/`.toFloat()` for string; reject `.toInt()` on bool
- `crates/ynz-diagnostics/src/lib.rs` (or wherever diagnostics defined) — new diagnostic kinds for options
- `crates/ynz-typeck/tests/check.rs` — new tests (options-only)
- `crates/ynz-driver/tests/fixtures/` — `m6_options_*` typeck-only fixtures (compile-time, no run yet — P4a wires codegen)
- `examples/errors/m6_errors.ynz` — CREATE in this phase; add the options-related error classes here (non-exhaustive options multi-case; comparison-between-options; ambiguous-shorthand; function-vs-shorthand collision; single-variant-options rejection; 256+-variant options rejection). Per the `### Demo & Error Gallery` per-phase obligation.
- `examples/basics/src/main.ynz` — extend with M6 options demo (declare `options Status { pending, active, banned }`; print `Status.active.toString()` deferred to P4 codegen; for P3a, declare + use via assignment + comparison). Per the per-phase obligation.
**Deviation rule**: NO union typeck logic in this PR (P3b owns). NO codegen changes (P4 owns). NO docs changes beyond inline rustdoc.
**Steps**:
1. **OptionsValue AST representation** (LOCKED — picks here, not at coding time): re-use `Expr::FieldAccess` for `OptionsName.variantName`. Typeck distinguishes at lookup time: if `FieldAccess.receiver` resolves to a type name (not a value) AND the receiver's "type" is an options type, it's an OptionsValue; lookup variant in `OptionsTable`. Otherwise fall back to shape-field-access logic. Rationale: AST stays minimal; the same `Foo.bar` syntax serves both contexts; disambiguation lives in one place (typeck `FieldAccess` handler).
2. **Options declarations**: walk `Item::OptionsDecl`; populate `OptionsTable` (name → variant list). Built-in options (`SortOrder`, `Comparison`) registered at typeck startup. Reject: empty variants (compile error "options needs at least 2 variants"); SINGLE variant (compile error with rationale per Safety invariant); duplicate variant names within one options type; options name clashing with any shape/options name; ≥256 variants (compile error with hint "this is probably a code smell").
3. **Options values**: when typechecking `Expr::FieldAccess { receiver: Identifier(N), field: V }` where `N` is in `OptionsTable`, resolve to options value `(N, V)` typed as `OptionsType(N)`. Lookup `V` against the options type's variant list; reject unknown variant naming all valid variants.
4. **Ambiguous shorthand**: when typechecking a bare `Identifier(V)` whose expected type at the call site is `OptionsType(T)`, look up `V` across:
    - The function/binding scope (functions ALWAYS WIN; if `V` is a function name in scope, do NOT try shorthand — the function's type either matches the parameter type or produces the normal type-mismatch diagnostic; CITES both candidates per the locked diagnostic text).
    - The options-variant table: if exactly ONE visible options type defines variant `V`, resolve to `(thatType, V)`. If MULTIPLE visible options types define `V`, produce the locked ambiguous-shorthand diagnostic.
5. **Options multi-case typeck (`OptionName` arms)**: scrutinee must be a single options type (rejects union-of-options scrutinee with hint to use qualified `OptionsName.variant` form via `Is` arm). Each `OptionName(V)` must be a known variant of the scrutinee's options type. Exhaustiveness: each declared variant must appear in some arm, OR `else =>` must exist; missing variants named in diagnostic.
6. **Comparison between options types**: `==`/`!=` between two distinct options types is a compile error citing both types. `==` between two values of the SAME options type passes typeck (codegen lowers in P4).
7. **`.toInt()` / `.toNumber()` / `.toFloat()` intrinsic registration** (extending `crates/ynz-typeck/src/intrinsics.rs:57-68`):
    - `(int).toInt() -> int` — identity, returns `int` directly (not wrapped in `maybe`; conversion is infallible).
    - `(float).toInt() -> maybe<int>` — wraps the conversion (P4 emits the locked codegen sequence).
    - `(number).toInt() -> maybe<int>` — wraps ynz-numerics decimal→i64 (P4 wires).
    - `(string).toInt() -> maybe<int>` — wraps runtime call (P4 implements runtime).
    - `(string).toNumber() -> maybe<number>` — same pattern.
    - `(string).toFloat() -> maybe<float>` — same pattern.
    - `(bool).toInt()` → compile error per Teaching invariant.
8. **`.toString()` on options values** — typeck registration: each options type gets a `.toString() -> string` method via `OptionsTable`. Codegen in P4.
9. **m3 deferral diagnostic message update**: the `MatchPatternKind::Variant` deferral now points to "M6 phase 3a" (this phase) — but since this phase IMPLEMENTS it, the deferral diagnostic for variant arms is REMOVED. The `MatchPatternKind::IsType` deferral diagnostic stays, pointing to "M6 phase 3b" (next phase).
10. **Add to `examples/errors/m6_errors.ynz`** (create file): the options-related error cases listed in Files (each with `// WHY:` comment).
11. **Add to `examples/basics/src/main.ynz`**: M6 options section showing declaration + value use + comparison + multi-case dispatch. Final `.toString()` line marked `// after P4 codegen lands`.
**Acceptance criteria**:
- [ ] Options decl + value typeck working; positive + negative fixtures.
- [ ] Built-in `SortOrder { asc, desc }` and `Comparison { equal, greater, less }` available without import.
- [ ] Exhaustiveness check for options multi-case names all missing variants.
- [ ] Ambiguous shorthand produces locked diagnostic text (assertion in test, not just "some diagnostic").
- [ ] Function-vs-shorthand collision case produces the type-error variant with both candidates named.
- [ ] Single-variant options rejected with diagnostic.
- [ ] 256+ variant options rejected.
- [ ] Comparison-between-options types rejected with both types named.
- [ ] `.toInt()` etc. added to `intrinsics.rs` for all source types per step 7.
- [ ] `.toInt()` on bool rejected with diagnostic.
- [ ] `examples/errors/m6_errors.ynz` created with options error cases (and grows in P3b/P4 for union/narrowing cases).
- [ ] `examples/basics/src/main.ynz` extended with M6 options section.
- [ ] Full `cargo test --workspace` green.
- [ ] Jargon audit clean.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
**Quality gate**:
- [ ] Every new diagnostic has WHAT/WHAT-INSTEAD/WHY per banned_jargon audit.
- [ ] The two locked diagnostics (`||` non-propagation in P3b, ambiguous-shorthand here) match the exact text locked in this plan.
- [ ] No M5 fixture broken; no M3 fixture's stderr snapshot broken (the variant-arm portion of M3 deferrals is REMOVED here since P3a implements it).
**Verification**: `cargo test -p ynz-typeck` shows ~20 new tests; `./target/debug/ynz run examples/errors/m6_errors.ynz` produces all options-related error classes; `./target/debug/ynz run examples/basics/src/main.ynz` runs through the new M6 options section.

---

### Phase 3b: Typeck — Unions + Narrowing
**PR scope**: Union typeck (variant validation, exhaustiveness, single-variant rejection, `T | none` redirect to `maybe<T>`); `is` narrowing in `if`-condition position (positive + negative); multi-case `Is(TypePath)` arm typeck + exhaustiveness; `&&` propagation; `||` non-propagation (with locked diagnostic text); early-return narrowing extending `return_paths.rs` (recognized-exit set: `return`, `panic`, infinite `loop`); reassignment invalidation; lend-call invalidation; documented closure non-propagation rule (forward-defensive). Replaces the M3 `IsType` deferral diagnostic.
**Branch**: `feat/m6-typeck-unions`
**Flag**: N/A
**Est. lines**: ~850 (union + narrowing typeck + extensive tests)
**Ships via**: `/pr`
**Objective**: All union + narrowing surface type-checks correctly. M5's narrow form of `.value` narrowing is generalized to the full M6 rules table.
**Why this phase exists**: union typeck + the flow-narrowing analysis are the bulk of M6. Splitting from P3a keeps each diff under ~900 lines. P3a's options surface lands first because options is structurally simpler.
**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs:~900` (per M5) — current `.value` narrowing implementation (M5 narrow form); generalize to M6 full form
- `crates/ynz-typeck/src/return_paths.rs` — return-path analysis; extend with early-return narrowing
- `crates/ynz-typeck/src/check.rs:440-456` — `MatchPatternKind::IsType` deferral stub; widen here
- `crates/ynz-ast/src/nodes.rs` — `Type::Union` variant added in P2; consume here
**Files (expected scope)**:
- `crates/ynz-typeck/src/check.rs` — union typeck, `is`-narrowing in if-condition, multi-case `Is`-arm typeck
- `crates/ynz-typeck/src/narrow.rs` (NEW or extend if M5 created one) — flow-narrowing analysis (generalize M5's `.value` engine)
- `crates/ynz-typeck/src/return_paths.rs` — extend with early-return narrowing analysis
- `crates/ynz-typeck/src/unions_table.rs` (NEW) — union layout decisions cache (built here, consumed in P4 codegen)
- `crates/ynz-diagnostics/src/lib.rs` — new union/narrowing diagnostics; LOCKED text for `||` non-propagation
- `crates/ynz-typeck/tests/check.rs` — comprehensive union + narrowing tests
- `crates/ynz-driver/tests/fixtures/` — `m6_union_*`, `m6_narrow_*`, `m6_neg_*` typeck-only fixtures
- `examples/errors/m6_errors.ynz` — extend with union/narrowing error cases (this phase adds: non-exhaustive union multi-case; `is Type` on non-union; `is Type` with type not in union; `.foo` access without `is`; union `Foo | Foo`; non-adjacent duplicate `Foo | Bar | Foo`; single-variant union; namespace shadow for `is X`; `||` non-propagation diagnostic; reassignment invalidates; `lend` call invalidates; non-recognized-exit form for early-return; redundant `is Foo` INFO diagnostic; nested-narrowing edge cases)
- `examples/basics/src/main.ynz` — extend with M6 union + narrowing demo (classify-account union; early-return narrowing example; `.toInt()` runtime conversion final line — codegen lands in P4 but typeck approves here)
**Deviation rule**: NO codegen changes (P4 owns). NO docs changes beyond inline rustdoc. If a locked rule in P0 docs is wrong, STOP and revise plan + docs.
**Steps**:
1. **Union types**: walk `Type::Union`; build a flat variant list (each variant must be a concrete shape or built-in primitive). Reject (per Safety invariant): single-variant union; duplicate variants regardless of position (`Foo | Foo`, `Foo | Bar | Foo`); union including `none` (typeck rewrites to `maybe<T>` semantically); union nested via reference (out of scope — reject with hint).
2. **`is` namespace resolution**: `MatchPatternKind::Is(TypePath)` and `Expr::Is { expr, ty }` look up `TypePath` in types-only namespace per Safety invariant.
3. **`is` narrowing — positive form**: for `if (x is Foo) { ... }`, inside the then-block, narrow `x` to `Foo` in the typeck environment. Reject: `is Foo` when `Foo` not in `x`'s union variants (cite the actual variants in the diagnostic). For `x` whose static type is already `Foo` (not union), emit the INFO-level "redundant is-check" diagnostic per Teaching invariant.
4. **`is` narrowing — negative form**: for `if (!(x is Foo)) { ... }`, inside the then-block, narrow `x` to `union\{Foo}` (the remaining variants), respecting exact-type-match rule (no subtype expansion via `extends`).
5. **Multi-case narrowing**: in `if (x) { is Foo => ..., is Bar => ..., else => ... }`, each `is` arm narrows `x` to the matched type within its body. The `else =>` arm narrows `x` to the remaining union variants. Exhaustiveness: each declared variant must appear in some `is Foo` arm, OR an `else =>` arm must exist. Missing variants enumerated in diagnostic.
6. **`&&` propagation**: in `if (cond1 && cond2) { body }`, propagate narrowings from `cond1` into `cond2`'s typecheck context AND into `body`. Symmetric: `(cond2 && cond1)`.
7. **`||` non-propagation**: in `if (cond1 || cond2) { body }`, do NOT propagate single-branch narrowings into `body`. Only propagate narrowings that BOTH `cond1` and `cond2` independently prove (rare). Diagnostic on `.value` (or other narrowing-required access) inside the body emits the LOCKED text from Teaching invariant.
8. **Early-return narrowing**: extend `return_paths.rs`. Recognized-exit set: `return <expr>`, `return` (in `nothing`-returning function), `panic(...)`, `loop { /* body has no break */ }` (infinite loop). After an `if` statement whose THEN-block always exits via a recognized-exit form, typeck environment for the remainder of the enclosing block gets the negation of the if's condition's narrowing facts. Diagnostic when user expects narrowing from a non-recognized exit form follows the Teaching invariant.
9. **Nested early-return**: outer if-narrowing facts propagate through inner if's; inner if's exits do NOT add facts to outer block per the adversarial test case rule.
10. **Reassignment invalidation**: any narrowing fact about `x` is invalidated by `x = ...`. Field mutation that typeck can't prove is harmless also invalidates (conservative; document in `narrow.rs`).
11. **Lend-call invalidation**: passing `x` as `lend self` to a function invalidates `x`'s narrowing facts. (`share self` does NOT invalidate.)
12. **Closure non-propagation**: forward-defensive rule documented in `narrow.rs`; no test (closures don't ship until v0.3+).
13. **Compute union layout decisions**: for each `Type::Union` encountered, compute its layout per the locked decision table (Performance invariant); cache in `UnionLayoutTable` keyed by canonical variant set. P4 codegen consumes this cache.
14. **m3 `IsType` deferral diagnostic REMOVED**: the AST variant is now fully implemented; remove the deferral path; remove the M3 fixture's stderr snapshot at the end of this phase (replace with stdout snapshot in P5).
15. **Extend `examples/errors/m6_errors.ynz`** with all union + narrowing cases per Files list above (each with `// WHY:` comment).
16. **Extend `examples/basics/src/main.ynz`** with the classify-account union example + early-return narrowing example + `.toInt()` runtime conversion line. The `.toString()` and runtime conversion lines compile but won't run end-to-end until P4 lands codegen; mark them with `// runs after P4 codegen`.
**Acceptance criteria**:
- [ ] Union typeck working; positive + negative fixtures per Adversarial Test Cases.
- [ ] All narrowing rules from `design/narrowing.md` (locked in P0) have positive + negative fixtures.
- [ ] `||` non-propagation diagnostic matches the LOCKED text exactly (assertion compares strings).
- [ ] Early-return narrowing's recognized-exit set is the ONLY set that proves; non-recognized exits emit the locked diagnostic.
- [ ] Nested-early-return and nested-multi-case adversarial fixtures pass per the locked semantics.
- [ ] Redundant `is Foo` produces INFO-level diagnostic AND compilation succeeds.
- [ ] M3 `m3_is_type_deferral.ynz` stderr snapshot DELETED (P5 ships the new stdout snapshot with the runnable replacement).
- [ ] `UnionLayoutTable` cache exposed for P4 codegen consumption.
- [ ] Full `cargo test --workspace` green; new tests add ~30+.
- [ ] Jargon audit clean.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
**Quality gate**:
- [ ] Each narrowing rule has `// WHY:` comment in the test naming the invariant.
- [ ] No typeck path allows accessing a union variant's field without an `is` check (grep-audited).
- [ ] All diagnostics WHAT/WHAT-INSTEAD/WHY; jargon clean.
- [ ] No `match`/`tagged union`/`discriminant`/`enum` in user-facing diagnostic text.
- [ ] `narrow.rs` documents the closure-non-propagation rule even though no test fires.
**Verification**: `cargo test -p ynz-typeck` shows new tests; spot-test `./target/debug/ynz run examples/errors/m6_errors.ynz` shows all union+narrowing error classes; the locked-text diagnostics produce byte-identical output to the spec.

---

### Phase 4: Codegen — Options + Unions + Narrowing
**PR scope**: LLVM codegen for `options` (i8 tag); LLVM codegen for unions (tagged-struct + pointer-niche per locked table); LLVM codegen for `is` narrowing (tag-load + compare); LLVM codegen for multi-case `is`/`OptionName` arms (switch on tag); LLVM codegen for `.toString()` on options values; LLVM codegen for fallible conversions (string-runtime function calls; existing primitive conversions wrapped in `maybe<T>` constructor).
**Branch**: `feat/m6-codegen`
**Flag**: N/A
**Est. lines**: ~900 (codegen + runtime extensions + tests)
**Ships via**: `/pr`
**Objective**: Every typeck-approved M6 program compiles to correct LLVM IR and links into a runnable binary. IR snapshots assert layout decisions.
**Why this phase exists**: codegen is mechanical given P3's typeck; splitting keeps the diff focused on emission semantics + the runtime extensions.
**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs` — main codegen driver
- `crates/ynz-codegen/src/emit.rs:839-846` — current `MatchPatternKind::Value` codegen + `IsType`/`Variant` deferral stub; replace stubs with real implementation
- `crates/ynz-codegen/src/emit.rs` (search for `maybe_alloc` or M5 maybe lowering) — exemplar for the tagged-struct vs pointer-niche pattern; mirror for unions
- `crates/ynz-runtime/src/lib.rs` — extend with `ynz_string_to_int`, `ynz_string_to_number`, `ynz_string_to_float`
- `crates/ynz-codegen/tests/snapshots.rs` — IR-snapshot tests live here
**Files (expected scope)**:
- `crates/ynz-codegen/src/emit.rs` — major changes
- `crates/ynz-codegen/src/options.rs` (NEW) — options-type codegen helpers
- `crates/ynz-codegen/src/unions.rs` (NEW) — union-type codegen helpers (layout decision, tag load/store, payload extract)
- `crates/ynz-runtime/src/lib.rs` — new `ynz_string_to_*` functions
- `crates/ynz-runtime/Cargo.toml` — if any new deps (unlikely; `str::parse` covers most)
- `crates/ynz-codegen/tests/snapshots.rs` — IR snapshots for each row of the union-layout decision table
- `crates/ynz-driver/tests/fixtures/` — runnable fixtures asserting end-to-end behavior
**Deviation rule**: Codegen choices outside the locked decision table need design-doc updates first. If the executor finds a layout choice the design doc didn't anticipate, STOP and amend `design/unions.md`.
**Steps**:
1. **Options codegen**: each `Item::OptionsDecl` produces a no-op at codegen (compile-time only). Each `OptionsValue` lowers to an `i8` constant (variant index). `==` between options values lowers to `icmp eq i8`.
2. **`.toString()` on options values**: emit a per-options-type LLVM global `[N x *const u8]` array of variant-name string literals (UTF-8 byte literals followed by null terminator). The `.toString()` method indexes the array by the variant's tag and constructs a Yinz string. NEW runtime function `ynz_string_from_static(ptr: *const u8, len: usize) -> *mut YinzString` added to `ynz-runtime` in this phase (it does NOT exist today — verified via grep over `crates/ynz-runtime/src/lib.rs`); it allocates a heap-owned `YinzString` and copies the bytes (so the returned string follows Yinz's normal ownership semantics, not a borrow into immortal memory). Heap copy chosen over zero-copy because: (a) Yinz strings are always owned (immortal-borrow would create a special-case `YinzStringStatic` type with different drop semantics); (b) the cost is one alloc + memcpy per `.toString()` call, which is negligible for what the call already does (caller will print or compose).
3. **Union codegen — layout selection**: at the point of emitting any LLVM type for a `Type::Union`, walk the variants and apply the decision table:
   - All variants are heap shapes AND no `none` variant → pointer-niche layout: `{ i8 tag, ptr data }`.
   - Otherwise → tagged-struct layout: `{ i8 tag, [maxPayloadSize x i8] payload }` aligned to max(alignof variant).
   - Cache the chosen layout in a `UnionLayoutTable` keyed by the union's canonical variant set (to avoid recomputing for identical unions in different positions).
4. **Union construction**: when an expression's static type is a union and the value's runtime type is a known variant, emit: allocate the union struct → store the variant's tag in the tag slot → store/copy the variant's value into the payload slot (with appropriate alignment).
5. **`is` narrowing codegen**: `(x: Union).is Foo` lowers to: load tag from x → compare to `Foo`'s tag constant → `i1` result. Inside a narrowed-then-block, payload-extract uses the variant-specific offset.
6. **Multi-case `is` arms — switch lowering**: lower the multi-case to LLVM `switch tag` with one BB per arm. The matched-against-arms's tag value is the switch's case label. `else =>` becomes the switch's default BB. Within each arm's BB, payload-extract is automatic (typeck-narrowed x).
7. **Multi-case `OptionName` arms — switch lowering**: same as `Is` for options types; tag is `i8`, no payload.
8. **Comparison between options types**: REJECTED at typeck (P3); no codegen needed.
9. **Fallible primitive conversions — MANDATORY codegen sequence (IR-snapshot asserted)**:
   - `(int).toInt()` → identity, no codegen wrapper.
   - `(float).toInt() -> maybe<int>` — emit verbatim:
     ```
     entry:
       %is_nan = fcmp uno double %x, %x          ; true if x is NaN
       br i1 %is_nan, label %ret_none, label %check_range
     check_range:
       ; NOTE: i64::MAX (2^63 - 1) is NOT representable exactly in f64;
       ; the nearest f64 = 9.223372036854776e+18 = 2^63 (one above i64::MAX).
       ; So "x in valid range" means: x < 2^63 AND x >= -2^63.
       ; Using strict `<` on the upper bound catches the boundary exactly.
       %too_big = fcmp oge double %x, 0x43E0000000000000   ; >= 9.223372036854776e+18 (2^63)
       br i1 %too_big, label %ret_none, label %check_low
     check_low:
       %too_small = fcmp olt double %x, 0xC3E0000000000000  ; < -9.223372036854776e+18 (-2^63)
       br i1 %too_small, label %ret_none, label %do_convert
     do_convert:
       %v = fptosi double %x to i64              ; RAW fptosi (not .sat), proven safe
       %m = insertvalue {i1, i64} {i1 1, i64 undef}, i64 %v, 1
       ret {i1, i64} %m
     ret_none:
       ret {i1, i64} {i1 0, i64 0}
     ```
     Rationale: `fptosi.sat` returns 0 for NaN (LLVM-defined) and saturates for out-of-range — both behaviors are wrong for our spec ("none on NaN/OOR"). We do the checks ourselves THEN use raw `fptosi` on the proven-safe input.
     **Critical boundary**: i64::MAX = 9223372036854775807 (2^63 - 1) cannot be represented exactly in f64; the nearest f64 above it is 2^63 = 9.223372036854776e+18, which IS i64::MAX + 1 and therefore overflows. So the upper check is `>= 2^63` (strict greater-or-equal against 2^63), NOT `> i64::MAX`. The lower bound -2^63 = i64::MIN IS representable exactly in f64, so `< -2^63` is correct. Test vectors: `(9.223372036854776e18).toInt() == none` (the boundary case — without this fix, codegen would call `fptosi` on a value that overflows, producing poison); `(-9.223372036854775808e18).toInt() == some(i64::MIN)` (the boundary holds since the f64 representation equals i64::MIN exactly).
   - `(number).toInt() -> maybe<int>` — call existing `ynz_numerics_to_i64` (already shipped in M2) wrapped to check its overflow flag; if overflow → `{has_value:0}`, else `{has_value:1, value:N}`. Verify M2's `ynz_numerics_to_i64` returns an overflow flag; if not, P4 extends it (small change to `ynz-numerics`).
10. **Fallible string conversions** — runtime functions implement the LOCKED parsing rule from the Risk row, NOT `str::parse` directly:
    - Add three new `extern "C"` Rust functions in `ynz-runtime`:
      - `ynz_string_to_int(s: *const u8, len: usize) -> { has_value: i1, value: i64 }` (struct return matching `maybe<int>` lowering for primitives per `design/maybe.md`)
      - `ynz_string_to_number(...) -> { has_value: i1, value: Decimal128 }`
      - `ynz_string_to_float(...) -> { has_value: i1, value: f64 }`
    - Implementation in safe Rust:
      ```rust
      pub extern "C" fn ynz_string_to_int(ptr: *const u8, len: usize) -> MaybeInt {
          let s = unsafe { std::slice::from_raw_parts(ptr, len) };
          // Step a: strip ONLY ASCII whitespace [0x20, 0x09, 0x0A, 0x0D]
          let trimmed = trim_ascii_ws(s);
          // Step b+c: accept optional [+-], reject empty / lone-sign
          let (sign, digits) = parse_sign(trimmed);
          if digits.is_empty() { return MaybeInt::none(); }
          // Step d: only [0-9]+ — no prefix, no decimal, no exponent
          if !digits.iter().all(|b| b.is_ascii_digit()) { return MaybeInt::none(); }
          // Step e: parse to i64 with overflow check
          match parse_i64_decimal(sign, digits) {
              Some(n) => MaybeInt::some(n),
              None => MaybeInt::none(),  // out of range
          }
      }
      ```
      (Helper functions `trim_ascii_ws`, `parse_sign`, `parse_i64_decimal` private to `ynz-runtime`; tested independently with the locked test-vector table.)
    - `ynz_string_to_float`: same structure but allows `.` + scientific notation per the locked rule; uses `f64::from_str_radix` or hand-written parse — NOT `str::parse::<f64>` (which accepts `inf`/`nan` strings; we don't want those parsed as values).
    - `ynz_string_to_number`: forwards to a decimal128 parser in `ynz-numerics` (verify the parser exists or extend it; the parsing rules match `.toFloat()`'s but produce decimal128).
    - Test vectors from the Risk row are asserted in Rust unit tests in `ynz-runtime` AND in end-to-end fixtures.
    - Codegen emits the call for `.toInt()`/`.toNumber()`/`.toFloat()` on string receivers; the struct return matches the existing `maybe<T>` ABI for primitives.
11. **IR-snapshot tests**: one per layout decision table row. Use `insta` (the snapshot crate the M5 plan introduced) to lock the emitted IR for `shape S = HeapA | HeapB` (pointer-niche), `shape S = HeapA | ValueB` (tagged-struct), etc.
12. **Runnable fixtures**: `m6_options_basic.ynz`, `m6_union_basic.ynz`, `m6_narrowing_early_return.ynz`, `m6_string_to_int.ynz`, `m6_match_options.ynz` (this last one renames M3's `m3_match_keyword_banned.ynz` deferral context — to avoid confusion, m6's fixtures use `m6_multicase_*` naming). Each fixture asserts stdout snapshot.
13. **Existing M5 fixtures**: re-run; assert no regression.
**Acceptance criteria**:
- [ ] Each row of the union-layout decision table has an IR-snapshot test asserting the expected LLVM type.
- [ ] `(float).toInt()` codegen sequence matches the locked IR pattern in Step 9 (IR-snapshot test asserts the `fcmp uno` + range-check + raw `fptosi` order — NOT `fptosi.sat`).
- [ ] `ynz_string_from_static` added to `ynz-runtime` per Step 2.
- [ ] `ynz_string_to_int/number/float` added to `ynz-runtime` per Step 10; Rust unit tests in `ynz-runtime` assert ALL test vectors from the locked parsing-rule risk row.
- [ ] `m6_options_basic.ynz` compiles + runs + matches stdout snapshot.
- [ ] `m6_union_basic.ynz` compiles + runs + matches stdout snapshot.
- [ ] `m6_narrowing_early_return.ynz` compiles + runs + matches stdout snapshot.
- [ ] `m6_float_to_int_nan.ynz`, `m6_float_to_int_out_of_range.ynz` compile + run + match stdout snapshots.
- [ ] `m6_string_to_int_whitespace_sign.ynz` compiles + runs through the full test-vector table from the locked parsing rule.
- [ ] `m6_multicase_options.ynz` compiles + runs + matches stdout snapshot, with `switch` instruction in IR snapshot.
- [ ] **Per-phase Demo + Gallery obligation**: `examples/basics/src/main.ynz` M6 section runs end-to-end after P4 (the `.toString()` and runtime conversion lines from P3a/P3b that were marked `// runs after P4 codegen` now run). `examples/errors/m6_errors.ynz` extended with any P4-specific error cases (e.g., if any new errors emerge during codegen — typically just confirmation that P3-time errors still fire end-to-end).
- [ ] Full `cargo test --workspace` green; new fixtures add to total; M5 fixtures still green.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
**Quality gate**:
- [ ] No `unsafe` in the new codegen Rust code beyond what's required for FFI signatures (extern "C" runtime functions).
- [ ] All `ynz_string_to_*` runtime functions handle every input in the locked test-vector table (Rust-level test).
- [ ] No fixture relies on undefined behavior or platform-specific layout (assert on LLVM IR text patterns, not byte layouts).
- [ ] IR-snapshot for `(float).toInt()` does NOT contain `fptosi.sat` (anti-assert: codegen-correctness gate).
**Verification**: `cargo test -p ynz-codegen` shows new IR snapshots passing; `cargo test -p ynz-runtime` shows test-vector table green; `./target/debug/ynz run crates/ynz-driver/tests/fixtures/m6_float_to_int_nan.ynz` prints the expected `none` outputs.

---

### Phase 5: Catch-Up Fixture Creation + Diagnostic Polish
**PR scope**: Replace M3's `m3_is_type_deferral.ynz` deferral content with a runnable union example. CREATE (not rename — they don't exist on disk) the M2 string-parse catch-up demonstration fixtures: `m2_string_to_int.ynz`, `m2_string_to_number.ynz`, `m2_string_to_float.ynz` (along with negative variants `_invalid.ynz` for each). Sweep for any M6 diagnostic that doesn't follow WHAT/WHAT-INSTEAD/WHY. Update `.claude/state.md` and `.claude/todos.md` to reflect closed catch-ups.
**Branch**: `feat/m6-catchups`
**Flag**: N/A
**Est. lines**: ~250 (mostly new + replaced fixture files + state updates)
**Ships via**: `/pr`
**Objective**: Every catch-up item from earlier milestones is closed. The "M6 must catch up" todo items are checkboxes-able.
**Why this phase exists**: catch-up FIXTURES are demonstration code; they don't belong in P3/P4 (which are implementation) and shouldn't ride P6 (which is verification + tag). Dedicated phase keeps them visible.
**Current-state anchors**:
- `crates/ynz-driver/tests/fixtures/m3_is_type_deferral.ynz` — current deferral content (replace)
- `crates/ynz-driver/tests/fixtures/m3_is_type_deferral.ynz.stderr` (or wherever the snapshot lives — find via grep) — DELETE during this phase
- NO `m2_string_parse_*.ynz` files exist today — confirmed via `ls crates/ynz-driver/tests/fixtures/m2_*` returning only `int_max_deferred`, `wrapping_add_deferred`, `bignum_deferral` and other non-string fixtures. P5 creates the string-related catch-up fixtures from scratch.
- `crates/ynz-typeck/src/intrinsics.rs:57-68` — M2 conversion intrinsic table (P3a extended with fallible conversions)
- `crates/ynz-diagnostics/src/banned_jargon.rs` — audit list
- `.claude/state.md:96` — "M6 catch-up obligations from M3" list
- `.claude/plans/done/m2-literals-arithmetic.md:62` — "M6 must catch up" list
**Files (expected scope)**:
- `crates/ynz-driver/tests/fixtures/m3_is_type_deferral.ynz` — REWRITE to runnable union example with stdout snapshot
- `crates/ynz-driver/tests/fixtures/m3_is_type_deferral.ynz.stderr` (if exists) — DELETE
- `crates/ynz-driver/tests/fixtures/m3_is_type_deferral.ynz.stdout` — NEW snapshot
- `crates/ynz-driver/tests/fixtures/m2_string_to_int.ynz` — NEW (runs `.toInt()` on strings; demonstrates the test-vector table)
- `crates/ynz-driver/tests/fixtures/m2_string_to_number.ynz` — NEW
- `crates/ynz-driver/tests/fixtures/m2_string_to_float.ynz` — NEW
- `crates/ynz-driver/tests/fixtures/m6_narrow_or_no_propagate.ynz` — NEW (M5-deferred narrowing edge cases — fixtures even though typeck implementation lives in P3b)
- `crates/ynz-driver/tests/fixtures/m6_narrow_reassign_invalidates.ynz` — NEW
- `crates/ynz-driver/tests/fixtures/m6_narrow_lend_invalidates.ynz` — NEW
- `crates/ynz-typeck/src/check.rs` — small diagnostic-text polish if jargon audit flags anything
- `.claude/state.md` — update catch-up status (mark closed)
- `.claude/todos.md` — check off completed catch-up items
**Deviation rule**: NO new features. Fixture creation + diagnostic polish only. Document any deviation.
**Steps**:
1. Rewrite `m3_is_type_deferral.ynz` to a runnable `shape Shape = Circle | Square` with multi-case `is` arm; add stdout snapshot via `insta`; delete stderr snapshot.
2. CREATE `m2_string_to_int.ynz` with the locked test-vector table cases (good + bad inputs; demonstrate via `.or(default)` pattern). Add stdout snapshot.
3. CREATE `m2_string_to_number.ynz` and `m2_string_to_float.ynz` similarly.
4. CREATE `m6_narrow_or_no_propagate.ynz`, `m6_narrow_reassign_invalidates.ynz`, `m6_narrow_lend_invalidates.ynz` (P3b's typeck implements; P5 ships the demonstration fixtures). Each asserts the locked diagnostic text via stderr snapshot.
5. Run jargon audit (re-grep `crates/ynz-diagnostics/` and `crates/ynz-typeck/src/check.rs` for any user-facing text mentioning `match`/`tagged union`/`enum`/`discriminant`). Fix any hits.
6. Update `.claude/state.md` to mark M6 catch-up obligations as closed.
7. Update `.claude/todos.md` to check off the M2/M3 catch-up items.
**Acceptance criteria**:
- [ ] `m3_is_type_deferral.ynz` is now a runnable example with stdout snapshot; old stderr snapshot deleted.
- [ ] Three M2 string-conversion fixtures created with stdout snapshots covering the full test-vector table.
- [ ] Three M6 narrowing-edge-case fixtures created with stderr snapshots asserting the locked diagnostic text.
- [ ] No `// CATCH-UP M6:` comments remain in source.
- [ ] No `M6 deferral` mentions in any error-message string in M6-touched code.
- [ ] `.claude/state.md` and `.claude/todos.md` reflect closed catch-ups.
- [ ] Full `cargo test --workspace` green.
**Quality gate**:
- [ ] All M2 + M3 + M5 catch-up obligations explicitly checked off (no items remain in the "still must close" status).
- [ ] All diagnostics in M6 phases pass three-part audit.
- [ ] Locked-text diagnostics in the new fixture snapshots match the plan's locked wording exactly.
**Verification**: `grep -rn "CATCH-UP M6\|M6 deferral" crates/ examples/` returns no hits; full test suite green; `./target/debug/ynz run crates/ynz-driver/tests/fixtures/m2_string_to_int.ynz` produces the expected vector results.

---

### Phase 6: Demo + Error Gallery + Verification + Tag
**PR scope**: Extend `examples/basics/src/main.ynz` with the M6 section per `### Demo & Error Gallery` invariant. Create `examples/errors/m6_errors.ynz`. Run full verification sweep: TODO sweep, catch-up audit, jargon audit, immutable-test audit, plan-invariant audit. Bump `Cargo.toml` to `0.1.0-m6`. Tag `v0.1.0-m6`. Update master plan to mark M6 SHIPPED.
**Branch**: `feat/m6-verification`
**Flag**: N/A
**Est. lines**: ~400 (demo + error gallery + state/todos + CHANGELOG)
**Ships via**: `/pr` (then `/release` after merge)
**Objective**: M6 is shipped, tagged, and the master plan's M7 milestone paragraph is ready for `/plan M7`.
**Why this phase exists**: per `.claude/rules/plan-invariants.md` and the M5 plan exemplar — every milestone closes with verification + tag.
**Current-state anchors**:
- `examples/basics/src/main.ynz` — current state has M1+M2+M3+M4+M5 sections; M6 appends below
- `examples/errors/m5_errors.ynz` — exemplar for the error-gallery format
- `.claude/plans/done/m5-generics.md` P6 — exemplar for the verification phase
- `Cargo.toml` — `package.version = "0.1.0-m5"` (M6 bumps to `0.1.0-m6`)
- `CHANGELOG.md` (if exists; check during phase) — append M6 entry
**Files (expected scope)**:
- `examples/basics/src/main.ynz` — append M6 section
- `examples/errors/m6_errors.ynz` — new file
- `Cargo.toml` (workspace + all member packages) — version bump
- `CHANGELOG.md` — M6 section
- `.claude/state.md` — M6 SHIPPED status
- `.claude/todos.md` — M6 items checked off
- `.claude/plans/active/v0-1-compiler.md` — M6 status → shipped, M7 ready
- `.claude/plans/active/m6-options-unions.md` — phase statuses updated; eventually moved to `done/` post-merge
**Deviation rule**: NO new features. Verification + tag only. Document any deviation.
**Steps**:
1. Extend `examples/basics/src/main.ynz` per the demo content listed in `### Demo & Error Gallery`. Run it; assert stdout matches expectations.
2. Create `examples/errors/m6_errors.ynz` per the error gallery content listed in `### Demo & Error Gallery`. Run it; assert every expected error class fires.
3. Add stdout/stderr snapshots for both files via `insta`.
4. TODO sweep: grep for `TODO`, `FIXME`, `// REPLACE-AT M6`, `// CATCH-UP M6`, `// will do later`, `// M7+` in source. Resolve or document each.
5. Catch-up audit: walk `.claude/state.md` "M6 catch-up obligations" — every item must be either closed or have an owner in M7/M8.
6. Jargon audit: re-run.
7. Immutable-test audit: spot-check any test files renamed during phases for the `// test-ratchet:` markers per `~/.claude/CLAUDE.md`.
8. Plan-invariant audit: confirm the 6 subsections of `### Invariants This Milestone Must Preserve` (this plan's section) have all their testable assertions covered by fixtures.
9. Bump `Cargo.toml` workspace version + all member packages to `0.1.0-m6`.
10. Generate CHANGELOG section from `git log` since `v0.1.0-m5` tag.
11. Run full `cargo test --workspace`. Run `cargo clippy --workspace -- -D warnings`. Run `cargo fmt --all --check`.
12. Open PR for verification phase via `/pr`.
13. After merge, run `/release` to tag `v0.1.0-m6` and push.
14. Update `.claude/state.md` with the M6-shipped entry (tag, test count, brief feature summary).
15. Move `m6-options-unions.md` plan file to `.claude/plans/done/`.
16. Update master plan `.claude/plans/active/v0-1-compiler.md`: M6 paragraph status → "shipped"; M7 paragraph cross-refs M6 narrowing infrastructure.
**Acceptance criteria**:
- [ ] `examples/basics/src/main.ynz` runs end-to-end through every milestone (M1-M6) demonstrated.
- [ ] `examples/errors/m6_errors.ynz` produces every M6 error class.
- [ ] All catch-up obligations closed or formally re-owned.
- [ ] Jargon audit clean.
- [ ] `Cargo.toml` at `0.1.0-m6`.
- [ ] `v0.1.0-m6` tag created and pushed.
- [ ] CHANGELOG section added.
- [ ] `.claude/state.md` + `.claude/todos.md` updated.
- [ ] Plan moved to `done/`.
- [ ] Master plan reflects M6 shipped + M7 ready.
- [ ] Full `cargo test --workspace` green (target: ≥640 tests; realistic estimate is M5's 574 + P1's +2 + P3a's +20 + P3b's +30 + P4's IR snapshots + P5's fixtures = ~640-660; do not block on hitting a specific higher number).
**Quality gate**:
- [ ] No outstanding M6 work items in any persistence file.
- [ ] Every M6 design doc (`design/options.md`, `design/unions.md`, `design/narrowing.md`) cross-references at least one fixture that demonstrates its locked decisions.
- [ ] CHANGELOG entry reads like a user-facing release note, not an internal log.
**Verification**: `./target/debug/ynz run examples/basics/src/main.ynz` produces expected stdout; `git tag --list 'v0.1.0-m6'` shows the tag; `cargo test --workspace` total ≥ 640 (per the relaxed acceptance criterion above).

---

## Quality Checklist (verify at completion)

- [ ] All M6 surface (options decl, options value, union, `is`, multi-case `is`/`OptionName`, exhaustiveness, narrowing in all forms locked in this plan, fallible conversions) type-checks correctly with positive AND negative fixtures.
- [ ] All M6 surface lowers to correct LLVM IR per the locked decision tables; IR snapshots assert.
- [ ] All M5 + M4 + M3 + M2 + M1 fixtures still pass without modification (except the catch-up fixtures renamed in P5).
- [ ] Every new compile-error class follows WHAT/WHAT-INSTEAD/WHY; jargon audit clean.
- [ ] `examples/basics/src/main.ynz` demonstrates each M6 feature in a realistic context.
- [ ] `examples/errors/m6_errors.ynz` triggers every M6 error class with `// WHY:` comments.
- [ ] No `TODO`/`FIXME`/`REPLACE-AT M6`/`CATCH-UP M6`/`will do later` in source.
- [ ] `Cargo.toml` bumped, tag `v0.1.0-m6` created, CHANGELOG section written.
- [ ] `.claude/state.md`, `.claude/todos.md`, master plan all reflect M6 SHIPPED.
- [ ] Plan file moved to `.claude/plans/done/`.

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: Each phase = one PR. Per plan-reviewer feedback, the original combined P3 (~1200 lines) was split into P3a (options + fallible-conversion intrinsics, ~450 lines) and P3b (union + narrowing typeck, ~850 lines). Each is independently shippable: P3a unblocks options-only programs; P3b adds union + narrowing. Phases 1, 2, 4, 5, 6 are smaller and naturally split-by-concern.
- **Shadow main branches**: Each phase branches from main, merges back via `/pr`. No long-lived shadow branches. Branch names follow `feat/m6-*` or `chore/m6-*` per `~/.claude/memory/branching.md`.
- **Building the engine before shipping value**: Each phase is independently shippable. P0 ships docs (zero code, zero behavior change). P1 ships lexer tokens (existing parser tolerates them as identifiers — no semantic change). P2 ships parser/AST (typeck still defers, behavior unchanged). P3 ships typeck (programs that use M6 surface get correct typeck; programs that don't are unaffected). P4 ships codegen (M6 programs now compile + run). P5 + P6 polish. Even if M6 stops mid-flight, what's shipped is correct and additive.
- **Hotfix that isn't**: M6 is not a hotfix; it's a planned milestone. The "hotfix" antipattern doesn't apply.
- **Abandoned branches**: Each phase's branch deletes after PR merges (standard `/pr` flow). The plan file in `active/` is the single living record; archive to `done/` only after `v0.1.0-m6` tags.
- **Flag graveyards**: M6 ships no feature flags. Compiler is shipped via binary version; users opt in by installing. No flag to clean up.

---

## Questions

Original Patrick-confirmed planning decisions:
- Scope: matches as listed
- Union LLVM layout: mechanical decision table (recommended)
- Narrowing reach: full (is + early-return + `&&` + `||`-non-propagate + reassignment-invalidation)
- Options shorthand: ship in M6 P3a
- AST rename: keep `MatchPattern*` family naming; widen the two stub variants in place
- `Is` arm variant name: `Is(TypePath)`
- Options-arm variant name: `OptionName(String)`

Additional decisions locked during plan-reviewer round 1 (no open questions for Patrick — all resolved against the spec / no-duct-tape rule):
- `.toInt()` on float/number returns `none` for NaN, ±Inf, OOR; truncates toward zero for in-range (locked in `### Risks` and P4 codegen step).
- String→numeric parsing: ASCII whitespace only; `[+-]?` sign; `.toInt()` rejects fractional/scientific; `.toFloat()`/`.toNumber()` accept fractional+scientific; no `0x`/`0o`/`0b` prefix at runtime (those are integer literal lexer-level only).
- `(float).toInt()` codegen MUST do explicit `fcmp uno` NaN-check + range-check BEFORE `fptosi` (NOT `fptosi.sat`) — locked via IR-snapshot test.
- `OptionsValue` AST representation: re-use `Expr::FieldAccess`; disambiguate at typeck.
- Single-variant `options` REJECTED (symmetry with single-variant union).
- `is X` namespace resolution: types-only namespace; same-name bindings do not shadow.
- Function-vs-options-shorthand priority: function wins.
- Recognized-exit set for early-return narrowing: `return`, `panic`, infinite `loop` only.
- Redundant `is Foo` (where `x: Foo` static): INFO-level diagnostic (precursor to v0.4 lint).
- `||` non-propagation diagnostic text + ambiguous-shorthand diagnostic text: locked verbatim.
- P3 split into P3a (options) + P3b (union+narrowing) per reviewer feedback.
- P5 CREATES catch-up fixtures (does not rename — they don't exist on disk).

No open questions remain. Ready for plan-reviewer round 2.
