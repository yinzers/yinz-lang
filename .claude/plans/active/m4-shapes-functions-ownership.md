---
slug: m4-shapes-functions-ownership
owner: patrick
status: active
files:
  - crates/ynz-parser/src/**
  - crates/ynz-ast/src/**
  - crates/ynz-typeck/src/**
  - crates/ynz-codegen/src/**
  - crates/ynz-runtime/src/**
  - crates/ynz-diagnostics/src/banned_jargon.rs
  - crates/ynz-driver/tests/fixtures/**
  - design/decisions.md
  - design/type-system.md
  - design/ownership.md
  - design/numeric-types.md
  - spec/types.md
  - spec/ownership.md
  - spec/variables.md
created: 2026-05-15
last_updated: 2026-05-16-r17
depends_on: [v0-1-compiler]
flag: N/A
---

# Plan: M4 — Shapes, Methods, Ownership

Created: 2026-05-15
Status: pending_approval

> **Why this plan exists as a separate file from `v0-1-compiler.md`.** The v0-1-compiler plan covers the M1–M8 roadmap and held M1/M2's full detail. M3 was planned in its own file (`done/m3-control-flow-fns.md`) for review focus; M4 follows the same pattern. The v0-1-compiler plan's roadmap remains the index — this plan is the M4 detail. Both files coexist via the `depends_on: [v0-1-compiler]` front-matter.

---

## Context & Why

**Goal.** Ship M4 of the v0.1 compiler: user-defined types (`shape` declarations) with fields, methods, single inheritance (`extends`), structural contracts (`follows`), the `override` keyword, hidden fields (`hidden`), runtime polymorphism (`dynamic`), all five ownership modifiers (`.share`/`.lend`/`.give`/`.copy`/`.freeze`), an ownership-analysis salsa query that enforces `const` deep-immutability + use-after-give, heap allocation via libc `malloc`/`free`, drop-on-scope-exit, and LLVM `readonly`/`noalias` attribute emission on parameter loads. Plus the M2-deferred catch-up: overflow escape methods (`.wrappingAdd` family) and type-attached constants (`int.max`, `int.min`, `number.epsilon`).

**Why.** M4 is the spine of the language's safety + performance moat. Until M4, every program is primitive-only (M2/M3) — no user types, no ownership, no heap. M4 introduces all three together because they're inseparable: types need ownership (a heap value must have one owner); ownership needs types (the borrow checker tracks per-binding lifetimes); both need codegen (heap alloc, drop, LLVM attributes). The roadmap (`.claude/plans/active/v0-1-compiler.md:178`) calls this the hardest milestone in v0.1.

**Background.** M1 shipped end-to-end pipeline (hello-world, `820bfdc`). M2 shipped numerics + variables + arithmetic + bool (118 tests, decimal128). M3 shipped control flow + user-defined functions + return-path analysis (310 tests, `fib(10) = 55`). All M3 source structures (Scope, signatures pre-pass, Diagnostic infrastructure, salsa queries, intrinsic table) are in place and ready to extend. The design-lockdown plan (closed 2026-05-14) reserved `shape` as the keyword (NOT `type`), produced the 5-subsection Invariants rule (`.claude/rules/plan-invariants.md`), the auto-promotion rule (`.claude/rules/auto-promotion.md`), the inference rule (`.claude/rules/inference.md`), and the dual-audience vocabulary rule (`.claude/rules/vocabulary.md`). M3's plan now carries a retroactive Invariants section that we model M4's after.

**Constraints.**
- M4 plan MUST include `## Invariants This Milestone Must Preserve` with all five subsections (`### Safety` · `### Performance` · `### Teaching` · `### Runtime Dependencies` · `### Kernel-Mode Behavior`) per `.claude/rules/plan-invariants.md`. Each subsection lists testable assertions, not aspirations.
- `const` deep-immutability MUST be enforced in M4 typeck (block `.lend`, `.give`, field-mutation on const bindings) AND the LLVM `readonly`/`noalias` contract MUST be emitted by M4 codegen. Both are enforced by `.claude/graveyard.md` Entry 1.
- `shape` is the reserved declaration keyword (per design-lockdown locked decision). `type` MUST be a banned-keyword token in the lexer with a teaching diagnostic (same pattern M3 used for `fn`, `match`, `switch`).
- The M2 catch-up list explicitly named M4 as the owner of overflow escape methods AND type-attached constants (`v0-1-compiler.md:651-664`). Per user direction this milestone honors that.
- No `try`/`catch`/`recover` syntax (`.claude/graveyard.md` Entry 5 — critical).
- No requiring explicit ownership annotation at CALL sites (`.claude/graveyard.md` Entry 2 — warning). Call sites infer; signatures declare.
- All user-facing diagnostics use WHAT/WHAT-INSTEAD/WHY three-part format (Golden Rule 11). Banned jargon stays banned (`infer`/`inference`/`narrowing`/`monomorphize`/...) — verified by `tests/jargon_audit.rs`.

**Success criteria.**
- `ynz run examples/m4_player.ynz` compiles and runs a program with: a shape declaration with at least two fields, a method with `share self`, an instance constructed at `let p = Player { ... }`, a method call `p.greet()` that mutates a `lend` parameter, an explicit `.give` transferring ownership, and prints expected output to stdout.
- `cargo test --workspace` passes with M4 added (estimate ~470–520 tests; M3 ended at 310).
- LLVM IR for a `share T` parameter contains `readonly` and `noalias` attributes (asserted by a codegen IR-text snapshot).
- A negative fixture exercising use-after-give produces a three-part diagnostic naming the give-site as the consume point AND the use-site as the offense (asserted by `insta` stderr snapshot).
- A negative fixture passing a `const`-bound value where a function declares `lend` produces a three-part diagnostic explaining `const` blocks mutation paths (asserted by snapshot).
- The catch-up fixtures from M2 (`m2_wrapping_add_deferred.ynz`, `m2_int_max_deferred.ynz`) are CLOSED — they now compile and run, replacing their deferral-stderr snapshots with success-stdout assertions, and the catch-up entries in the M2 plan get marked done in `v0-1-compiler.md`.
- The plan-reviewer agent issues PASS on this plan before any P1 code lands.
- M4 ships behind the `v0.1.0-m4` tag with an updated `CHANGELOG.md` listing every catch-up + new feature.

---

## Research Findings

**Locked decisions from prior milestones / design-lockdown** (each shapes M4):

1. **`shape` keyword reserved for M4 type declarations** (NOT `type`). Lexer adds `Shape` token in P1; `type` becomes a banned-keyword diagnostic. Source: `v0-1-compiler.md:1399-1403`, `design/decisions.md`, design-lockdown plan.
2. **Const deep-immutability** — `const` blocks all five paths to mutation: reassignment (already enforced M2 `check.rs:264`), field mutation (M4 to enforce), `.lend` mutable-borrow (M4), `.give` ownership transfer (M4), mutable inference (M4 — compiler never infers `.lend`/`.give` for a `const` binding). Source: `design/ownership.md:33-76`.
3. **Uniform inference + IDE muted hints at CALL sites; explicit at SIGNATURES.** Inverse anti-pattern (requiring `.share` at call sites) is in `.claude/graveyard.md` Entry 2. Source: `.claude/rules/inference.md`.
4. **Static dispatch is the default; `dynamic T` is opt-in.** Naming locked: `dynamic` NOT `dyn` (anti-jargon). Source: `design/type-system.md:57-210`.
5. **Method receiver = `share self` / `lend self` / `give self` at first parameter position.** Source: `design/type-system.md:75-89`.
6. **Structural typing.** `return { quotient: a / b, remainder: a % b }` works when return type is `DivResult` — no `return DivResult { ... }` needed. Source: `design/type-system.md:49-55`.
7. **`hidden field: T = default` requires a default value.** Hidden fields are invisible outside the declaring shape's methods; defaults make initial state explicit. Source: `design/type-system.md:243-260`.
8. **`override` keyword required in both directions.** Missing `override` when parent has the method = error; using `override` when parent doesn't = error. Source: `design/type-system.md:41-47`.
9. **Single inheritance with `extends`; any number of `follows`.** `shape Warrior extends Entity follows Damageable, Attackable`. Source: `design/type-system.md:15-46`.
10. **Default args owned at first call, not shared mutable.** Ownership prevents Python's mutable-default bug by construction — no special compiler rule needed. Source: `design/functions.md:47-58`.
11. **Capital letter = type; lowercase = everything else** (Golden Rule 13). `Player`/`Self` = types; `player`/`self` = values/instances. Source: project `CLAUDE.md`.
12. **No tuples.** Returning multiple values requires defining a shape. Source: `design/functions.md:23-28`. M4 makes this finally possible (M1–M3 had no user shapes, so multi-return wasn't expressible).
13. **No `try`/`catch`/`recover` syntax** (graveyard Entry 5 — critical).
14. **LLVM attribute contract**: `readonly` on every `share T` param (and every param inferred from a `const` binding); `noalias` on every param the borrow checker proved non-aliased. Source: `design/ownership.md:51-66`.

**Salsa-query architecture continuity.** M3 added `signatures` pre-pass + `return_paths` + `check` queries. M4 adds two more salsa queries: `shapes` (resolves every shape declaration in a module → field table + method table) and `ownership` (per-function borrow-check result). Both must be cache-invalidated by changes to their dependencies; both produce `DiagnosticBucket`s; both lower into the existing `check` flow.

**Heap allocation strategy.** M4 emits `malloc(size)` / `free(ptr)` calls via the runtime crate `ynz-runtime`. The runtime adds two new extern declarations (`ynz_alloc` / `ynz_free`) that thin-wrap libc for telemetry hooks v0.3+ may add. Drop-on-scope-exit emits `free` calls at scope end, reverse declaration order. No reference counting; no GC; no per-instance metadata.

**Vtable strategy for `dynamic`.** `dynamic Foo` lowers to a fat pointer `{ data_ptr, vtable_ptr }`. The vtable is a per-(concrete-type, contract) constant pointer table emitted at compile time, indexed by method slot. Method call on a `dynamic Foo` value compiles to: load vtable_ptr → load method slot → indirect call. Polar Signals' Go benchmark (`design/type-system.md:114`) puts the runtime cost at ~3× a direct call, which is exactly why dynamic is opt-in.

**Object layout for shapes.** Compiler auto-reorders fields for ABI tightness (per `.claude/rules/auto-promotion.md` example — no opt-out keyword; FFI handled at the boundary). For M4 we default to insertion order (declaration order); the auto-reorder optimization is locked to v0.3+ and the design doc reservation is noted here. Each shape becomes an LLVM `%StructName = type { field0_ty, field1_ty, ... }` definition.

**Method dispatch — name resolution.** Method lookup is two-phase: (a) inherent methods on the receiver's concrete type, (b) `follows`-contract methods inherited from declared contracts. M4 does (a) for static dispatch + (b) for `dynamic Foo` dispatch. No automatic method-from-extends lookup yet — `extends` methods are inherited but `override` is required to redeclare. Method overloading by argument count or type is NOT supported (one method name = one signature per concrete type, modulo `override` which keeps the parent's signature).

**Field-access expression model.** `value.field` reuses the M3 `Expr::MethodCall { receiver, method, args }` slot? No — fields and methods are distinct in M4 to enable assignment-via-`Expr::FieldAccess`. M4 adds `Expr::FieldAccess { receiver, field }` as a separate variant. `value.field = x` lowers to `Stmt::FieldAssign { target_receiver, target_field, value }`.

**Constructor syntax.** Structural literal: `Player { name: "Patrick", health: 100 }`. The parser must distinguish `Foo { ... }` (struct literal) from `if (cond) { ... }` (block) — looking at first token after `{` is sufficient: an `Identifier ':'` start indicates struct literal; anything else is a block.

**Dot-modifier parsing.** `.share`, `.lend`, `.give`, `.copy`, `.freeze` are POSTFIX dot-modifiers. The lexer already produces `Dot` + `Identifier`. The parser treats `.share`/`.lend`/`.give`/`.copy`/`.freeze` as recognized postfix markers; everything else after a `.` is a method call or field access. Modifiers are not keywords reserved at the lexer level — `share` can be used as an identifier OUTSIDE of dot-postfix context (e.g., variable name) without ambiguity, because dot-postfix is parser-level. Verified by negative test.

**Catch-up — overflow escape methods AND type-attached constants.** Both close their M2 deferral fixtures. `.wrappingAdd`/`.wrappingSub`/`.wrappingMul`/`.saturatingAdd`/`.saturatingSub`/`.saturatingMul` are intrinsics on `int` (M4 piggybacks on the M2 `PrimitiveIntrinsicTable`). `int.max`, `int.min`, `number.epsilon`, `number.max`, `number.min` are TYPE-ATTACHED constants requiring a new lookup path — type names parsed in expression position (`int` is normally a Type, but `int.max` parses as `int` + `.` + `max`).

**Banned-jargon evolution.** The `BANNED_JARGON` constant (`crates/ynz-diagnostics/src/banned_jargon.rs`) gets two new lexer-level bans (not in BANNED_JARGON itself, which is for diagnostic prose): `type`, `struct`, `class`, `interface`, `enum`, `abstract`. Each gets a Token variant + a teaching diagnostic per the M3 pattern for `fn`/`match`/`switch`. This is P1 work.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Ownership analysis bugs let unsafe code compile | Medium | Critical (language safety) | Two-stage typeck split (shape decls + ownership as separate salsa queries); positive AND negative fixture coverage per ownership rule; jargon-clean diagnostics force precision in error wording; plan-reviewer + Bouncer enforce Invariants section |
| `const` deep-immutability gap survives M4 | Medium | Critical (graveyard Entry 1) | Explicit `### Safety` invariant lists all five paths; positive test asserts each path produces correct diagnostic; LLVM IR snapshot asserts `readonly` on `share T` params + `readonly` inferred from `const` bindings |
| Use-after-give silently allowed | Medium | Critical (memory safety) | Borrow-check assigns a unique consume-id at each `.give` site; use of a consumed binding looks up consume-id → reports give-site span + use-site span; negative fixture per pattern (give-then-read, give-then-give, give-in-branch-then-use) |
| Drop-on-scope-exit double-frees or leaks | Medium | High (correctness; harder to detect at compile time) | Each owned binding is tracked in a per-scope drop-list; `.give` moves it to the receiver scope's drop-list; conditionally-given bindings get a runtime drop-flag (initialized true at decl, flipped to false at `.give`) — this is the only runtime state M4 needs beyond malloc; valgrind run in CI on a heap-allocating fixture catches leaks |
| Inheritance breaks structural typing's predictability | Low | Medium | `is` keyword (M6) is the only place inheritance crosses with subtyping; M4 inheritance is field+method inheritance only, no subtyping decisions in expression context (M4 has no `is` operator); inheritance unit tests pin behavior |
| `dynamic` vtable layout silently breaks ABI across recompiles | Low | High | M4 emits per-(concrete-type, contract) vtables as anonymous compile-time globals — no exported symbols, no cross-module ABI yet (modules land M8). When modules land, vtable layout becomes a v0.1 ABI question — recorded in the catch-up list for M8 |
| LLVM `readonly`/`noalias` attribute emission gets skipped or wrong | Medium | High (perf regression invisible to functional tests) | IR-text snapshot test exists per parameter kind: `share T` → both attributes; `lend T` → noalias + writable; `give T` → no readonly; `const`-bound passed to inferred `share` → both attributes; snapshot diff fails CI |
| `.give` and pattern-matching across branches becomes inconsistent | Medium | High (compile-time soundness) | Flow-sensitive give-analysis: at every block end, the set of consumed bindings must match across branches (otherwise use-after-conditional-give); explicit test |
| Methods + overrides break with diamond-like inheritance via `follows` | Low | Medium | `follows` is structural only in M4 (no method-bodies-from-contracts; contracts only assert "this shape provides these signatures"); two contracts requiring the same method name with different signatures = compile error citing the conflict; M4 has no method-from-contract default-bodies (those land M5+ if ever) |
| Two-pass shape resolution misses forward references in same module | Low | High | Pre-pass collects all shape decls before resolving method bodies (mirroring M3's signatures pre-pass); cyclic field dependencies (shape A field of type B which has field of type A — direct, not via pointer) error with three-part diagnostic naming the cycle |
| Catch-up phase silently regresses M2 numeric correctness | Low | High | Catch-up fixtures from M2 (`m2_wrapping_add_deferred.ynz`, `m2_int_max_deferred.ynz`) are CONVERTED from deferral-stderr snapshots to success-stdout snapshots; the prior stderr snapshot files are deleted with explicit reason; M2 catch-up entry in `v0-1-compiler.md` is closed in the same PR with a `# CLOSED: <commit-sha>` annotation |
| Heap-alloc failure (OOM) handling undefined | Medium | Medium (forward-compat for embedded/--kernel) | M4 panics on malloc failure for now (the system isn't going to recover from OOM in normal user code anyway); kernel-mode (v0.3+) plan adds plug-in allocator with user-defined OOM behavior; recorded in `### Runtime Dependencies` and `### Kernel-Mode Behavior` |
| Plan size collapses into a "one giant milestone" trap | Medium | Medium | Phase split below has 9 phases — each is one PR, each has objective + acceptance criteria; verification phase confirms `cargo test --workspace` + IR snapshots + jargon audit + Bouncer-clean before tag |
| `dynamic` is added with no use case in M4 itself | Medium | Low | M4 demos `dynamic Foo` with a Drawable-like contract + array of mixed concrete types; even without M5 generics, `dynamic` lowering is exercised end-to-end |
| `hidden` field default-value evaluation has unexpected runtime cost | Low | Low | Default expressions are evaluated at every `Player { ... }` construction, NOT at type-decl-time (Python footgun avoided by ownership per `design/functions.md:47`); test confirms `hidden cache: map<string, number> = {}` produces a fresh map per construction |

---

## FINAL LOCKED DECISIONS (r11 — authoritative, supersedes anything inconsistent below)

> **READ THIS FIRST.** Plan body sections (Phases, Invariants, etc.) below this section were written before r10/r11 and reflect the pre-r10 OOP-like model. The PHASE BODIES will be rewritten in Doc-PR 3 (Task #9). Until then, **THIS SECTION is the source of truth.** Any conflict between this section and the body: this section wins.

### Language model (locked r10–r11)

1. **Yinz is NOT object-oriented.** Data shapes + standalone functions + UFCS dot-call sugar (Go/Rust style, not Java/Swift). [r10]
2. **Shapes hold ONLY data fields + contract method-signature declarations.** NO method implementations inside shape declarations. [r10]
3. **Methods are standalone functions** taking the receiver as the first parameter. **OPEN-Q11 LOCKED (r13): Option A — UFCS.** Both `value.method()` and `method(value)` are legal; compiler treats them as identical at parse time. Per-codebase style is a Tier 3 lint concern (v0.4+), not a language constraint. The dot-call form preserves Golden Rule 1 (dot-first / autocomplete discoverability) and TS familiarity; the function-call form serves utility-function ergonomics (`max(a, b)` reads better than `a.max(b)`). [r10, r13]
3a. **Dual-style teaching in diagnostics (locked r13)**: every UFCS-related error message MUST show BOTH call forms in its WHAT-INSTEAD section so users learn both styles from the same diagnostic. Compiler "did-you-mean" suggester searches both directions: (a) functions of the same name with mismatched signatures (explain WHY), (b) functions of any name whose first param matches the receiver type (show "things you CAN do with this value"). IDE renders the same suggestion via hover/tooltip. First-encounter teaching: the FIRST `tower.foo()` error a new user hits teaches both call styles in one shot. See `.claude/rules/non-oop.md` for the canonical diagnostic format.
4. **`function` keyword ALWAYS requires a body.** Body-less `function f() -> nothing` is a parse error. [r9]
5. **Contract method signatures use bare-signature form (NO `function` keyword)**: `compare(share self, share other: Self) -> int` inside a `shape` block. [r9]
6. **`extends` is DATA-only inheritance.** Child shape gets parent's fields; behavior comes from standalone functions. [r10]
7. **`override` keyword REMOVED entirely.** No methods on shapes → nothing to override. Function overloading by argument type is the dispatch mechanism. [r10]
8. **`follows` checked via standalone-function-signature matching** — a shape follows a contract when standalone functions exist whose signatures match the contract's bare-signature declarations. [r10]
9. **`dynamic Foo` lowers to a fat pointer** + per-(shape, contract) function-pointer table emitted as a compile-time global. Runtime dispatch ~3× cost vs static call. [r10, unchanged from r1]

### Ownership model (locked r4 / r10 / r11)

10. **Body-level `.share() / .lend() / .give()` syntax REMOVED.** Compiler always infers at call sites from the callee's signature. There is no `.share()` / `.lend()` / `.give()` syntax in the body — full stop. [r11]
11. **`.copy()` and `.freeze()` KEPT as body operations** (with parens per dot-postfix rule). `.copy()` produces a new value (only on trivially-copyable types); `.freeze()` locks a binding's mutability from this point. [r4 / r11]
12. **Signature ownership modifiers (`share`/`lend`/`give`)**:
    - **Free functions WITH body**: OPTIONAL (inferred from body usage; muted IDE hint shows what was inferred; explicit always allowed for emphasis, compiler verifies match)
    - **Contract method signatures**: REQUIRED (no body to infer from)
    - **Function-type annotations** (`let f: function(...)`): REQUIRED (type-level, no body) [r9 / r11]
13. **Const deep-immutability** unchanged — `const` blocks reassignment AND inferred-mutable AND field-mutation AND mutation through contract methods that need `lend self`. [r1, design-lockdown]
14. **Partial moves out of nested-shape fields BLOCKED in M4.** `o.inner` produces a borrow only; to "move out" inner, consume the whole `o` or `.copy()` (if trivially-copyable). Revisit in M5+ if a real use case emerges. [r5]
15. **Drop ordering at return-with-give**: locals drop in reverse-declaration-order BEFORE return executes; given value moves to caller. Standard Rust-style. [r4]
16. **Use-after-give = compile error** with both spans (give-site + use-site). Branch-merge inconsistent-consume = compile error. [r1]
17. **No runtime ownership errors possible** — compile-time-only system. Either compiles cleanly or refuses to emit a binary. [r8]

### Syntax + style rules (locked r4 / r9 / r10)

18. **Dot-postfix rule** [r4]: actions use parens (`value.method()`, `value.copy()`, `value.freeze()`, `intrinsic.parse(...)`); field/constant access uses no parens (`player.health`, `int.max`, `number.epsilon`). Does NOT apply to ownership modifiers (which have no body syntax per #10). New rule file: `.claude/rules/dot-postfix.md`.
19. **Struct literals: ANNOTATION-ONLY form.** `let p: Player = { name: "...", health: ... }` is the only legal form. `Player { ... }` prefix form is a compile error with teaching diagnostic redirecting to annotation form. [r4]
20. **`shape` is the locked declaration keyword** for type declarations (NOT `type`). `type`/`struct`/`class`/`interface`/`enum`/`abstract` are banned-keyword diagnostic tokens (lexer-level). [design-lockdown]
21. **Hidden-field defaults: constants + empty literals only.** No function calls, no field references, no `self`. Hidden fields require a default because callers can't see them at construction. [r4]
22. **`.copy()` strict**: only legal on transitively-trivially-copyable types. For non-trivial types, user writes a standalone `copy()` function the caller invokes with normal call syntax. [r4]
23. **All examples in every spec/design/plan/rule file MUST use real Yinz operations from the current scope.** No invented APIs for illustration (the `int.parse()` mistake won't repeat). [r5]
24. **Doc-PR process**: grep first, write second. Every doc-PR's process MUST grep the codebase for the relevant patterns before editing, list every hit in the PR description, fix every hit in the same PR, and re-grep after to verify zero remaining. [r11]

### Teaching surfaces (locked r4 / r10 / r11)

25. **All M4 diagnostics follow WHAT/WHAT-INSTEAD/WHY three-part format** — enforced by the `Diagnostic` constructor (M1+ carried through). [Golden Rule 11]
26. **Banned-jargon enforcement** (`crates/ynz-diagnostics/src/banned_jargon.rs`) stays active. New diagnostics must pass `tests/jargon_audit.rs`. [M1+]
27. **IDE muted hints** render at call sites showing the inferred modifier (e.g., `db.save(p [.give()])`) and on bare free-function signatures showing inferred parameter ownership (e.g., `function save([give] p: Player)`). v0.2 LSP carries this; M4 generates the inference data the LSP renders. [r4 / r11]
28. **Tier 3 lint surfaces** (`design/linting.md`): the `ownership-contract-changed` lint warns when a body change in PR review shifts a function's inferred ownership contract (caller-visible behavior change). v0.4 deliverable. [r7]

### M4 phase impact (Doc-PR 3 rewrites the body)

29. **P1 lexer**: adds `shape`, `follows`, `extends`, `base`, `hidden`, `dynamic`, `SelfType` (capital `Self`), `SelfValue` (lowercase `self`). REMOVED from earlier plan: `Override` token (the keyword is gone). Banned-keyword diagnostic list unchanged (`type`, `struct`, `class`, `interface`, `enum`, `abstract`). Variant count ratchets to reflect.
30. **P2 parser**:
    - ShapeDecl body = `FieldDecl` + bare-signature contract-method-signature declarations only. NO `MethodDecl` (because methods aren't inside shapes).
    - NO struct-literal prefix form (`Player { ... }` is a compile error). Only annotation-driven anonymous literals.
    - NO body-level `.share()` / `.lend()` / `.give()` postfix-modifier. Only `.copy()` and `.freeze()` postfix-modifiers are parsed.
    - `function` declarations always at top level or inside `shape` blocks (as bare-signature contracts), never as nested function declarations.
31. **P3a typeck**: shape body declares fields + contract-method-signatures only. UFCS resolution at call sites: `value.fn()` → `fn(value)` desugars at lookup time. Static dispatch when concrete receiver type known.
32. **P3b**: `extends` is data-only inheritance (parent fields prepended to child layout). `follows` checked via standalone-function-signature matching against the contract's bare-signature declarations. `override` REMOVED entirely. `dynamic Foo` lowering via per-(shape, contract) function-pointer table.
33. **P3c ownership analysis**: simpler. No body-level modifier expressions to check. Per-parameter inference walks the body and classifies each parameter as `share`/`lend`/`give` based on usage. Cascade rule: a parameter passed to a function that takes `give` causes the outer parameter to infer `give` too.
34. **P4 codegen**: no per-shape vtable for static method dispatch (just normal function calls). Contract dispatch tables ONLY for `dynamic Foo`. LLVM `readonly`/`noalias` attribute contract unchanged. Heap alloc, drops, drop-flags unchanged.
35. **P5 catch-up**: wrapping/saturating int methods + type-attached constants unchanged.
36. **P6 driver + fixtures**: success-criteria fixture rewritten in new model (no methods inside shapes; standalone functions; annotation-driven literals; no body `.give()`).
37. **P7 verification + tag**: unchanged.

### Pre-P1 doc-PR sequence (Tasks #7 / #8 / #9)

- **Doc-PR 1** (Task #7 — FOUNDATION): NEW `.claude/rules/non-oop.md` codifying the model. Project `CLAUDE.md` note. `design/golden-rules.md` non-OOP principle. `design/decisions.md` entry. `.claude/rules/language-design.md` checklist update. NEW `.claude/rules/dot-postfix.md`. Grep-first process verification.
- **Doc-PR 2** (Task #8 — DOCS REWRITE): Major rewrite of `design/type-system.md` + `spec/types.md` (remove methods-inside-shapes; document standalone+UFCS pattern; remove `override`; redocument `extends` as data-only). Update `design/ownership.md` + `spec/ownership.md` (REMOVE body-level `.share()/.lend()/.give()` syntax — they don't exist; rewrite examples to use signatures + call-site inference only; `.copy()` and `.freeze()` stay as body operations). Update `.claude/rules/inference.md` (new domain row for ownership inference at call sites + signatures of free functions). Update `.claude/rules/vocabulary.md`.
- **Doc-PR 3** (Task #9 — PLAN REWRITE): Rewrite this plan's phase bodies (P2/P3a/P3b/P4) for the standalone+UFCS model per items #29-#37 above. Update success criteria, invariants Safety/Performance/Teaching subsections, anti-pattern callouts, M4 catch-up obligations.

After Doc-PR 3 merges, M4 P1 (lexer) starts.

### Doc-PR execution status (r14, 2026-05-16)

| Doc-PR | Task | Status | Files landed / pending |
|---|---|---|---|
| **Doc-PR 1** (Foundation) | #7 | ✅ COMPLETE | NEW `.claude/rules/non-oop.md` (283 lines), NEW `.claude/rules/dot-postfix.md` (100 lines), UPDATED `CLAUDE.md` (rule entries + non-OOP line + real-operations rule), UPDATED `design/golden-rules.md` (Cross-cutting principle section before Rule 1), UPDATED `design/decisions.md` (Cross-Cutting Architectural Principles section + forward-flagged `override` removal), UPDATED `.claude/rules/language-design.md` (OOP Drift Test section). Grep verification clean. Not yet committed — Patrick decides commit-per-doc-PR vs batch-commit at end. |
| **Doc-PR 2** (Docs rewrite) | #8 | PENDING | Major rewrite scope per r12 grep: `design/type-system.md` (lines 70-181 + remove §`override` Keyword Required at 41-46), `spec/operators.md` (8 method-in-shape examples at lines 148-181), `design/iterables.md` + `spec/iterables.md` (method-in-shape Iterable pattern), `spec/types.md:62`, `spec/overview.md:16`, `design/ownership.md` (extensive no-parens dot-modifier examples), `spec/ownership.md` (same + REMOVE body-level `.share()/.lend()/.give()` syntax), `design/ide-hints.md` (examples at lines 33, 48, 61), `design/errors.md:27` ("method on it" wording), `.claude/rules/inference.md` (ownership-inference domain row simplification), `.claude/rules/vocabulary.md` (audit). |
| **Doc-PR 3** (Plan rewrite) | #9 | PENDING | Rewrite this plan's phase bodies (P2/P3a/P3b/P4) for standalone+UFCS model per FINAL LOCKED DECISIONS items #29-#37. Update success criteria + invariants subsections + anti-pattern callouts. |

After Doc-PR 3 merges, M4 P1 (lexer) starts.

### Historical record (superseded by above; kept for traceability)

The following subsections preserve the per-round resolution trail. Don't act on them — they're for understanding HOW we got to the locked decisions above.

### LOCKED in r4 (2026-05-15) — superseded by FINAL LOCKED DECISIONS above

All six r3 OPEN questions resolved + one new design rule adopted. See `## Reviewer Disputes → Round 4` for the discussion. Summary:

| ID | Decision |
|---|---|
| Q1 | `.freeze()` KEPT (with parens per NEW dot-postfix rule) |
| Q2 | `extends` keyword KEPT; P3b adds teaching diagnostic clarifying extends-vs-follows |
| Q3 | Hidden-field defaults: constants + empty literals only |
| Q4 | Struct literals: **Option A — annotation only**. `Player { ... }` becomes a compile error with teaching diagnostic redirecting to `let p: Player = { ... }` |
| Q5 | `.copy()` strict cheap-only; user defines `copy()` method for deep copies |
| Q6 | Drop order: locals reverse-decl-order BEFORE return; given value moves to caller |
| **NEW-R4** | **Dot-postfix rule** locked: actions use parens (`value.method()`, `value.share()`, `value.give()`, `value.freeze()`); field/constant access uses no parens (`player.health`, `int.max`). New rule file `.claude/rules/dot-postfix.md` to be added. |
| **NEW-R4** | All five ownership modifiers move to parens form: `.share()` / `.lend()` / `.give()` / `.copy()` / `.freeze()`. Updates `spec/ownership.md`, `design/ownership.md`, plan P1+P2+P3c, `.claude/rules/inference.md`, `.claude/rules/vocabulary.md`. |
| **NEW-R4** | Plan examples reframed: `.give()` / `.share()` / `.lend()` are INFERRED at call sites + return statements per `.claude/rules/inference.md`. The user rarely types them; IDE renders muted hints. Plan's success-criteria fixture rewritten to reflect this. |

**Pre-P1 doc + plan rewrite obligations** (one consolidated PR, branch `docs/m4-dot-postfix-and-annotation-only`):

1. NEW: `.claude/rules/dot-postfix.md` — codifies the parens-for-actions / no-parens-for-access rule with examples + design-doc checklist item
2. UPDATE: `.claude/rules/language-design.md` — add dot-postfix checklist item to "Before Adding Anything New"
3. UPDATE: `.claude/rules/inference.md` — replace `.share` / `.lend` / `.give` examples with parens forms; the muted-hint examples become `// muted: .give()` (with parens) etc.
4. UPDATE: `.claude/rules/vocabulary.md` — rename-table entries for ownership modifiers get `()` suffix
5. UPDATE: `design/ownership.md` — rewrite dot-modifier examples with parens; clarify inference at call sites AND return statements
6. UPDATE: `spec/ownership.md` — same; also remove "you write `.share` at the call site" framing — replace with "the IDE shows the inferred modifier as muted text; type it explicitly if you want to be loud about it"
7. UPDATE: `design/type-system.md:53` — remove the `DivResult { ... }` prefix-form example; replace with anonymous + annotation
8. UPDATE: `spec/types.md` — rewrite all struct-literal examples (lines 83, 107, 120, 130, 174, 183, 205) to annotation form
9. UPDATE: this plan file — P1 parser examples use parens forms; P2 parser drops `Identifier {` lookahead (no more prefix-form struct literals); P3a typeck adds prefix-form teaching diagnostic; P3c ownership examples reframed to inferred; M4 success criteria fixture rewritten

After this consolidated doc-PR merges, P1 (lexer) starts.

### NEW OPEN — surfaced in r6 (2026-05-15) — MAJOR design reversal candidate

Patrick observed in r6: putting `share`/`lend`/`give` as a keyword PREFIX on signature parameters violates the just-locked dot-postfix rule (which says ownership operations are dot-postfix-with-parens on values, e.g., `p.give()`). The keyword-prefix form (`function save(give p: Player)`) is the odd one out now.

**OPEN-Q10 (NEW): Signature ownership semantics — outside-in (Rust-style explicit) vs inside-out (Yinz-inferred)?**

Patrick's example (`/workspaces/ynz/temp.ts` selected in r6):
```yinz
function save(p: Player) -> nothing {
  wait db.save(p.give())
}
```
Bare signature; body declares ownership via dot-postfix `.give()`. The compiler INFERS the function's ownership contract by analyzing the body — what the body does to the parameter.

**Inference rules** (proposed for M4):
- Body `.give()`s the parameter → infer `give`
- Body mutates parameter (field-write OR calls a `lend self` method) → infer `lend`
- Body only reads (or doesn't use) → infer `share`
- Body has inconsistent use across branches (e.g., gives in one branch, reads in another) → COMPILE ERROR per branch-merge rule (already in plan's P3c Safety invariants)

**Three forms considered for explicit-when-author-wants-to-lock-it**:
- **Form A (planner-leaning)**: bare always — no syntax for explicit. Compiler-inferred is the only form. IDE muted hint surfaces the inferred contract for IDE/code-review visibility.
- **Form B**: `(p: Player.give)` — modifier as dot-postfix on the TYPE. Respects the dot-rule but reads awkwardly ("Player to give").
- **Form C**: explicit `consumes`/`borrows`/`mutates` annotation block separate from signature. Verbose; declarative.

Planner-leaning: **Form A**. Cleanest, no new syntax, compiler does the work (Golden Rule 4). The IDE LSP surface (v0.2) carries the teaching load — muted hint shows the inferred contract; click-to-make-explicit would convert source to... well, Form A has nothing TO convert to. So if Patrick wants click-to-make-explicit, we need Form B or C as the explicit form.

**Direct contradiction with previously-locked design**: `design/ownership.md:14` says "Function signatures always declare intent (`share`, `lend`, `give`). The contract is visible at the definition — no surprises for callers." OPEN-Q10 PROPOSES REVERSING this. If Patrick approves, the design doc must be updated under a clear "Reversed decision (r6 2026-05-15): inside-out inference replaces explicit signature modifier" block.

**Trade-offs Patrick is implicitly accepting** (planner-flagged for transparency):
- (+) Less typing — bare signature is shorter
- (+) Dot-postfix consistency — all ownership lives in `.share()`/`.lend()`/`.give()` form
- (+) Compiler does the heavy lifting (Golden Rule 4)
- (−) Caller can't see ownership in the signature alone — must rely on IDE muted hint OR read the body
- (−) Changing body silently changes the function's effective contract (caller-side inference re-runs; callers may now error on use-after-give where they didn't before)
- (−) Cross-module: compiled module must export the inferred contract as part of signature metadata (so importing code knows what each function consumes/borrows/mutates)

Mitigation for the (−) items: v0.2 LSP shows the inferred contract prominently on hover; a Tier 3 lint warns when a body change in PR shifts a function's inferred ownership (`ownership-contract-changed`).

**DECISION REQUIRED before P1.** This is the largest design pivot in this plan's history. Patrick should sit with it overnight if he wants — the cost of getting this wrong is "every M4 fixture written one way then rewritten the other."

### NEW OPEN — surfaced in r5 (2026-05-15)

Patrick pushed back in r5: "I'm not sure YOU have thought the entire give thing out." Correct — the planner was being imprecise. Real gaps surfaced; two NEW decisions need locking before P1.

**OPEN-Q7 (NEW): Partial moves out of a nested-shape field.** Scenario:
```yinz
shape Inner { count: int }
shape Outer { inner: Inner }

let o: Outer = { inner: { count: 5 } }
return o.inner    // does this consume `inner` out of `o`?
```
- **Option X (planner-leaning)**: NO partial moves in M4. `o.inner` produces a `.share()` borrow only. To "move" inner out, you must consume the whole `o` OR `.copy()` (if Inner is trivially-copyable). Simpler; fewer edge cases; matches Yinz's "shapes are values, not field-bags."
- **Option Y (Rust-style)**: partial moves allowed. `o.inner` can be `.give()`'d; `o` becomes partially-dead (can use other fields, not the whole). More flexible; more complex; M5+ candidate at earliest.

Planner-leaning: **Option X for M4**, revisit if a real use case forces it. **DECISION REQUIRED before P1**.

**OPEN-Q8 (NEW): Function signatures must have explicit ownership modifier on every parameter.** Scenario:
```yinz
function save(p: Player) -> DbResult { ... }
//             ^^^^^^^^^ no share/lend/give — what's the default?
```
- **Locked-by-planner (pending Patrick confirm)**: **No implicit default.** `function save(p: Player)` is a compile error in M4. Author MUST type one of `share p: Player` / `lend p: Player` / `give p: Player`. Forces the ownership decision at the signature, which is the load-bearing decision the rest of the system depends on.
- Diagnostic: WHAT (parameter `p` is missing an ownership modifier) / WHAT-INSTEAD (pick `share` for read-only, `lend` for mutation, `give` for ownership transfer) / WHY (Yinz signatures explicitly declare ownership; there's no "implicit default" because the wrong default silently changes program meaning).

**DECISION REQUIRED before P1**.

**OPEN-Q9 (NEW — clarifying call-site inference table)**: lock the concrete inference table for `.share()` / `.lend()` / `.give()` at call sites + return statements. Table below (lock for M4 if Patrick approves):

| Source position | Receiving slot says | Compiler infers |
|---|---|---|
| `foo(p)` | `share p: T` | `.share()` |
| `foo(p)` | `lend p: T` | `.lend()` |
| `foo(p)` | `give p: T` | `.give()` |
| `return p` (local of type T) | function return type `T` | `.give()` (only valid choice — share/lend borrow can't outlive function) |
| `return p.field` where field is primitive (int, float, bool, string in M4) | function return type matches field | (copy, no modifier — primitives in M4 are copy-on-read) |
| `return p.field` where field is a nested shape | function return type matches field type | **OPEN-Q7 outcome** decides this |
| `array.add(p)` where add signature is `give T` | (same as function call) | `.give()` |
| `myStruct.field = p` (assigning a local to a field of a heap-owned struct) | field slot expects T (owned) | `.give()` |

Planner-leaning: **lock this table as the M4 inference contract** (P3c ownership-analysis depends on it). **DECISION REQUIRED before P1**.

### OPEN — historical record from r2/r3 (now all resolved in r4)

Six items pending Patrick's explicit confirm before P1 lands. Planner recommendations in **bold**. Discussion captured in `## Reviewer Disputes → Round 3` below.

**OPEN-Q1: `.freeze` — keep or remove?** **Planner-leaning + Patrick-leaning: KEEP.** Real use case is build-then-lock with conditional/intermediate mutation (`let cfg = ...; cfg.addRule("a", 1); if (debug) cfg.addRule("b", 2); cfg.freeze; runApp(cfg.share)`). Alternative via const-shadowing breaks down for the conditional-mutation case. JS/TS has `Object.freeze()` for the same pattern — familiar mental model. Cost to keep is small (one modifier + one typeck flag). Locks if Patrick confirms.

**OPEN-Q2: `extends` keyword name — keep or rename?** **Planner-leaning: KEEP `extends`.** Matches TS class extension per Golden Rule 6. The conceptual gap between extends (incoming code-inherit) vs follows (outgoing promise) stays regardless of spelling. Alternatives considered + rejected: `from` (collides with future imports), `inherits` (verbose), `built-on` (kebab-keyword). Mitigation: a teaching diagnostic on first `extends` use that explains the distinction in 3 lines. Locks if Patrick confirms.

**OPEN-Q3: Hidden-field default-expression scope.** **Planner-leaning + Patrick-leaning: constants + empty literals only.** Aligns with Patrick's "shape is contract not implementation" framing in r3 — function-call defaults would put implementation logic in the contract. Defaults are only on hidden fields (where they're load-bearing because hidden fields can't be construction-provided). Visible fields have NO defaults — always provided at construction. Locks if Patrick confirms.

**OPEN-Q4 (NEW in r3): Idiomatic struct-literal syntax — `Player { ... }` vs `let p: Player = { ... }`.** Patrick raised in r3: prefer the annotation-driven form over the type-name-prefix form because the prefix form reads OOP-y. Both forms ARE legal per `design/type-system.md:53` (structural typing — either way produces the same value).
- **Option A**: ban prefix form entirely. Breaks generic-with-inline-literal (no way to disambiguate what type the literal is in a generic call site).
- **Option B (planner-leaning)**: annotation idiomatic at let/const declarations + Tier 3 lint suggestion (`prefer-typed-literal-over-prefix-construction`). Prefix form stays legal for inline construction in generic function args where type isn't inferable. Spec/types.md examples switch to annotation form.
- **Option C**: keep both equal weight.

Lock if Patrick confirms Option B. Affects: `spec/types.md` examples (rewrite to annotation form), parser (no change — both already parsed), v0.4 lint catalog.

**OPEN-Q5 (re-asking): `.copy` strict cheap-only — confirm or relax?** Currently locked: `.copy` only on transitively-trivially-copyable shapes; user defines `copy()` method (called as `value.copy()`) for expensive deep-copies. Mirrors Rust's Copy vs Clone separation. The compiler picks `.copy` (modifier) when type is trivially-copyable, falls through to `.copy()` (method call) when there's a user-defined method on a non-trivial type — teaching diagnostic if neither applies. **Planner-leaning: KEEP strict.** Patrick raised in r3 ("regardless probably isn't the best use in this case BUT as long as we teach that maybe having it exist is still a nice feature"). Could relax to allow auto-deep-copy with teaching warning, but that violates "cheap by design" + creates a footgun (silent expensive copies in hot loops). Locks if Patrick confirms.

**OPEN-Q6 (re-asking): Drop ordering at `.give`-return — confirm.** Locked: locals drop in reverse declaration order BEFORE the return executes; the `.give`'d value moves OUT to the caller. Standard Rust-style. **Planner-leaning: KEEP.** Patrick raised in r3 asking what `.give` and "drop" mean; planner provided full explanation with TS-vs-Yinz comparison. Patrick's TS intuition ("the function returns Player rather than inheriting anything else done in that function") matches exactly. Locks if Patrick confirms.



### LOCKED (confirmed Patrick r2 2026-05-15)

**For Patrick — confirm before P1 lands:**

1. **`base shape` instantiation guard at construction site only, or also at typeck?** Both. Constructor `Entity { ... }` against `base shape Entity` is a typeck-time error. Diagnostic: WHAT (cannot instantiate a base shape) / WHAT-INSTEAD (instantiate a derived shape that extends Entity, or remove `base`) / WHY (base shapes are partial declarations meant to be extended). Confirmed in P3.
2. **`Self` keyword behavior in inherited methods.** When `shape Warrior extends Entity` inherits a method declared `function ping(share self) -> Self`, calling `warrior.ping()` returns `Warrior` (the concrete type) NOT `Entity`. Locked here as the "Self = concrete receiver type" rule, matching Rust/Swift. P3.
3. **Field default-expression evaluation order.** When a `hidden` field has a default expression that depends on a non-hidden field (e.g., `hidden cache: map<int, int> = {}; size: int`), the default expression CANNOT reference other fields (forward reference complexity). Restrict to constant expressions and `{}`/`[]` literals in M4. Document in spec.
4. **`dynamic Foo` array element layout — fat-pointer per element, or vtable-per-element?** Fat-pointer per element: `array<dynamic Foo>` is `array<{ data_ptr, vtable_ptr }>`. Each element is a 16-byte struct. Confirmed.
5. **Should `.copy` on a shape with a non-trivially-copyable field be allowed?** M4: no — `.copy` requires every field to be trivially copyable (transitively). Otherwise compile error with WHAT (field X of type Y is not trivially copyable) / WHAT-INSTEAD (define a `copy()` method and call that, or use `.give` to transfer ownership) / WHY (.copy is the cheap-by-design escape valve; non-trivial copy needs explicit semantics). Locked.
6. **`.freeze` semantics in M4.** Converts a `let`-bound value to an effectively-const binding for the rest of the scope. Underlying value cannot be modified; cannot be `.lend`'d or `.give`'d after freeze. Implementation: typeck flips the `is_const` flag on the binding's ScopeEntry from the freeze statement onward; codegen treats the binding as readonly from that point. Locked.
7. **Drop ordering at function return when ownership transfers via `.give` in the return expression.** The given value is moved out of the function's drop-list; remaining locals drop in reverse declaration order before return. Standard Rust-style; no surprises. P5 ownership-analysis spec.

---

## Risk Assessment & Rollout Strategy

**Risk level: HIGH** (lowered to MEDIUM with mitigations applied)

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler |
| Touches auth/permissions | No | |
| Raw SQL / literals | No | |
| Modifies existing data | No | Adds new compiler features; M3 behavior unchanged |
| Third-party integration | No | LLVM via inkwell only (already in use M1+) |
| Changes existing endpoints | Yes (in compiler sense) | Adds tokens, AST variants, types, intrinsics — every prior layer evolves. Mitigated by variant-count tests that ratchet rather than break. |

**Mitigations applied** (each lowers risk):

- Comprehensive test coverage per phase (positive + negative + edge per ownership rule + jargon-audit + LLVM IR snapshot) → HIGH → MEDIUM
- Idempotent compiler — same source compiles to same binary (deterministic codegen verified by output SHA-256 comparison) — MEDIUM → LOW
- Phase split into 9 small reviewable PRs — MEDIUM → LOW
- Plan-reviewer agent gate before P1 starts (Step 7) — independent check
- Backward compatible for M3 fixtures — every M3 fixture passes unchanged on the M4 branch before merge — verification phase asserts this

**Rollout plan** (this is a compiler — "rollout" is "tag a release"):

1. **Internal testing**: each phase PR runs `cargo test --workspace` + `ynz run` against the fixture suite. No automatic merge; user reviews each PR.
2. **Pre-release**: P9 verification sweep runs M1+M2+M3+M4 fixture suite end-to-end + IR snapshot diff + jargon audit + Bouncer clean + `cargo run --release` smoke on a real program.
3. **Tag**: after P9 green, tag `v0.1.0-m4`. CHANGELOG entry covers every M4 feature + closed M2 catch-ups.

This is a compiler; no production-traffic ramp.

---

## Roadmap (milestones — see `v0-1-compiler.md` for the full index)

This plan covers ONLY Milestone 4 in detail. M1–M3 are shipped (`done/`); M5–M8 stay one-line summaries in `v0-1-compiler.md` until their own plans are drafted.

### Milestone 4 (M4): Types + ownership — multi-session
`shape Foo { fields }` declarations with fields, methods, single inheritance (`extends`), structural contracts (`follows`), `override`, hidden fields (`hidden`), runtime polymorphism (`dynamic`). Ownership modifiers (`.share`, `.lend`, `.give`, `.copy`, `.freeze`). Ownership analysis as a salsa query. Heap allocation via libc `malloc`/`free`. Drop-on-scope-exit. LLVM `readonly`/`noalias` attribute emission. M2 catch-up: overflow escape methods + type-attached constants.
**Flag**: N/A
**Status**: in planning (THIS PLAN)
**Depends on**: M3 (`done/m3-control-flow-fns.md`)
**Ships via**: `/pr` per phase; `/release` after P9 (project skills detected — see `.claude/skills/pr/`, `.claude/skills/release/`)

### What M4 explicitly is NOT (deferred to later milestones)

- Function generics / type generics — M5
- Method overloading — never (one method name = one signature; structural typing handles polymorphism)
- `is Type` narrowing — M6 (depends on union types)
- `options` declarations — M6
- Union types `A | B` — M6
- `maybe T` sugar — M6
- Full Unicode strings — M7
- `errors` keyword + cascades — M7
- Iterables protocol — M7
- Modules / imports — M8
- Doc comments — M8
- `sensitive` modifier — M8
- Concurrency keyword parsing — M8
- Bignum `number[N]` for N > 34 — M8
- `ynz watch` / `ynz fmt` / LSP — v0.2 (separate plan)
- Auto-SoA layout transform — v0.3+ (`design/future/auto-soa.md`)
- Auto-Arc cross-thread inference — v0.3+ (`design/future/concurrency.md`)

**If a phase below feels like it's drifting into any of the above, STOP and re-plan.**

---

## Phases

> **READ FIRST (r17 added Demo & Error Gallery requirement)**: Per `.claude/rules/plan-invariants.md` `### Demo & Error Gallery` subsection (added 2026-05-16 per r17), every M4 phase that adds executable surface MUST also:
> - **Extend `examples/basics/src/main.ynz`** with the new feature in context (showing it doing real work, not isolated `print(featureName())`)
> - **Extend `examples/errors/m4_errors.ynz`** with intentional triggers for every new compile-error class added by that phase (each trigger gets a `// WHY:` comment naming the diagnostic class)
> - Both files get `insta` stdout/stderr snapshots in the phase's verification step
>
> The basics demo grows M1→M8 (one project; every v0.1 feature in context). The error gallery is per-milestone (`m4_errors.ynz` builds on the patterns established in `m1_errors.ynz`/`m2_errors.ynz`/`m3_errors.ynz` which exist as the retroactive baseline). Patrick reviews each phase's UX via these files — features that ship without hands-on demo + error-experience review go un-validated until users hit them.
>
> **READ FIRST**: phase bodies below were written before r10-r15 and reflect the pre-non-OOP model in places (e.g., references to MethodDecl inside ShapeDecl, override keyword, body-level `.share()/.lend()/.give()` postfix-modifier parsing). The **FINAL LOCKED DECISIONS section at the top of this file is authoritative** — when implementing, items #29-37 there describe how each phase changes for the non-OOP / UFCS / standalone-functions model. Specifically:
>
> - **P1 (lexer)**: REMOVE Override token from variant list (override keyword doesn't exist anymore). All other lexer additions stand (shape, follows, extends, base, hidden, dynamic, Self/self).
> - **P2 (parser)**: ShapeDecl body = `FieldDecl` + bare-signature contract-method-signature declarations only (no `MethodDecl` with `function` keyword + body inside shape). NO struct-literal prefix-form (`Player { ... }` is a compile error — only `let p: Player = { ... }` is legal). NO body-level `.share()`/`.lend()`/`.give()` postfix-modifier parsing (only `.copy()` and `.freeze()` postfix-modifiers exist).
> - **P3a (typeck)**: shape body validates fields + contract method-signatures only. NO method type-check inside shapes. UFCS resolution at call sites: `value.fn()` → `fn(value)` desugars at lookup time.
> - **P3b (inheritance + follows + override + dynamic)**: `extends` = data-only inheritance (no method inheritance). `follows` = compiler verifies standalone functions exist whose signatures match the contract's bare declarations. **`override` REMOVED entirely** (function overloading by argument type replaces it). `dynamic Foo` = fat pointer + per-(shape, contract) function-pointer table.
> - **P3c (ownership analysis)**: simpler. No body-level modifier expressions to check (because they don't exist as syntax). Per-parameter inference: walk body, classify each parameter as share/lend/give based on body operations (`.give()` → none, since not body syntax; mutation → lend; pass-to-give-signature → give; only-read → share).
> - **P4 (codegen)**: no per-shape vtable for static dispatch (just normal function calls). Contract dispatch tables only for `dynamic Foo`. LLVM `readonly`/`noalias` attribute contract unchanged. UFCS sugar lowers to direct function call (zero overhead).
> - **P5 (catch-up)**: unchanged.
> - **P6 (driver + fixtures)**: success-criteria fixture rewritten to use the non-OOP model (standalone functions, UFCS dot-call, annotation-driven literals, no body `.give()`).
> - **P7 (verification + tag)**: unchanged.
>
> The phase bodies that follow contain useful test-count plans, anchor file:line citations, LLVM attribute requirements, and fixture lists that remain valid. The OOP-leaning sections (especially in P2/P3a/P3b/P4) will be tweaked during implementation per the items above — but the structural plan (9 PRs labeled P1, P2, P3a, P3b, P3c, P4, P5, P6, P7; risks; success criteria; invariants subsections; M2 catch-up; verification sweep) is unchanged.

> **Phase numbering.** 9 PRs total, labeled P1, P2, P3a, P3b, P3c, P4, P5, P6, P7. P3a/P3b/P3c split typeck into three reviewable PRs because the combined work would exceed a single-PR scope. P3a delivers shape/field/method typeck; P3b delivers inheritance + follows + override; P3c delivers ownership analysis as a salsa query. P5 is catch-up (numerics escape valves + type-attached constants — independent of the type-system spine, lands in its own PR for reviewer focus). The labels stop at P7 even though there are 9 phases — the `a/b/c` split is the source of the count mismatch. Don't be surprised.

---

### Phase 1: Lexer extension (M4 keywords + banned-keyword diagnostics)
**PR scope**: Extend `ynz-parser::lex` with M4's new tokens: `Shape`, `Follows`, `Extends`, `Base`, `Override`, `Hidden`, `Dynamic`, `SelfType` (capital `Self`), `SelfValue` (lowercase `self`). Add banned-keyword diagnostic tokens for `Type` (the literal word), `Struct`, `Class`, `Interface`, `Enum`, `Abstract` — each produces a teaching diagnostic at first use and recovers as an `Identifier` (same pattern as M3's `Fn`/`Match`/`Switch`). Update the `m4_token_variant_count_locked` test.
**Branch**: `feat/m4-lexer`
**Flag**: N/A
**Est. lines**: ~500
**Ships via**: `/pr`
**Objective**: Lexing an M4 source like `shape Player { name: string, health: int }` produces the expected token stream; `type Player { ... }` produces a banned-keyword diagnostic AND recovers as `Identifier("type")` followed by the rest of the token stream. Total tokens after P1: 58 (49 M3 + 9 new); banned-keyword tokens DON'T count against the variant since they recover as Identifier — they're diagnostic-only.
**Why this phase exists**: Every later phase parses M4 syntax; the lexer must reserve the keywords first. The banned-keyword diagnostics are load-bearing per the design-lockdown locked decision (`v0-1-compiler.md:1399-1403`).
**Current-state anchors**:
- `crates/ynz-parser/src/token.rs:25` — variant count comment (currently `49`); update to reflect new count.
- `crates/ynz-parser/src/token.rs:150-155` — M3's `If`/`Else`/`While`/`For`/`In`/`Return`/`FatArrow` block; M4 tokens append after.
- `crates/ynz-parser/src/lexer.rs` — M3 added keyword lookup for `if`/`else`/`while`/`for`/`in`/`return`. M4 extends the same `match keyword` block (around line where M3 keywords are recognized — confirm in P1 execution).
- `crates/ynz-diagnostics/src/banned_jargon.rs:6-16` — dual-audience disclaimer; declaration-keyword bans are lexer-level, NOT in BANNED_JARGON. Confirms approach.
- `.claude/plans/done/m3-control-flow-fns.md` Phase 1 — M3's banned-keyword pattern for `fn`/`match`/`switch` is the template.
**Files (expected scope)**:
- `crates/ynz-parser/src/token.rs` (add 9 new variants + comment update)
- `crates/ynz-parser/src/lexer.rs` (extend keyword table; add banned-keyword recognition path)
- `crates/ynz-parser/tests/lex.rs` (positive + negative tests + variant-count ratchet)
- `crates/ynz-parser/tests/__snapshots__/` (new snapshots for banned-keyword diagnostics)
**Deviation rule**: Only the 9 token variants listed. NO `Pub`/`Priv`/`Static` (not M4 features). NO `Mut` (Yinz's mutation model is `let` + `.lend`, not a `mut` keyword). If a phase reviewer pushes for additional tokens, surface as a Reviewer Dispute.
**Steps**:
1. Add 9 Token variants: `Shape`, `Follows`, `Extends`, `Base`, `Override`, `Hidden`, `Dynamic`, `SelfType` (Self), `SelfValue` (self). Per Golden Rule 13: `SelfType` is for the literal `Self` keyword (capital); `SelfValue` is for `self`. Variant count comment → `58`.
2. Extend `lexer.rs` keyword-lookup match: `"shape" => Token::Shape`, etc. Lowercase `self` → `SelfValue`; literal `Self` → `SelfType`. Identifiers are case-sensitive (already the case).
3. Add banned-keyword detection BEFORE keyword recognition: if the identifier matches `type` | `struct` | `class` | `interface` | `enum` | `abstract`, emit a three-part diagnostic pointing to the Yinz replacement (`type` → `shape`; `struct`/`class`/`interface` → `shape`; `enum` → `options`; `abstract` → `base shape`). Recovery: return `Token::Identifier(s)` so the parser sees an identifier and continues. M3's `fn`/`match`/`switch` pattern is the template.
4. Update `m3_token_variant_count_locked` test → `m4_token_variant_count_locked` with `// test-ratchet: M4 adds 9 for shapes + inheritance + override + hidden + dynamic + Self/self`.
5. Positive lex tests: `shape Player { name: string }` → `[Shape, Identifier("Player"), LBrace, Identifier("name"), Colon, Identifier("string"), RBrace]`. Similar tests for each new keyword.
6. Negative lex tests with snapshot:
   - `type Player { ... }` → banned-keyword diagnostic at span of `type`; recovers as Identifier; stderr snapshot pinned via `insta`.
   - `struct Player { ... }`, `class Player`, `interface Drawable`, `enum Status`, `abstract shape` — same pattern, each snapshot pinned.
7. WHY-comment on each test: `// WHY: P1 reserves M4 keywords and forces banned legacy terms to teaching errors — see .claude/plans/active/m4-shapes-methods-ownership.md`.
8. Confirm `cargo test -p ynz-parser` + `cargo test --workspace jargon_audit` pass; the new banned-keyword diagnostics use no jargon (the words `infer`, `narrowing`, etc., must not appear in their bodies).
**Acceptance criteria**:
- [ ] All 9 new tokens lex correctly in positive fixtures (snapshot match).
- [ ] All 6 banned identifiers (`type`/`struct`/`class`/`interface`/`enum`/`abstract`) produce a three-part diagnostic pointing to the Yinz replacement keyword.
- [ ] Banned-keyword recovery emits `Token::Identifier(s)` so the parser can continue (snapshot of token stream confirms this).
- [ ] `m4_token_variant_count_locked` test ratchets from 49 → 58 with the inline `// test-ratchet:` marker.
- [ ] `jargon_audit` test (workspace-wide) passes: no banned-jargon word in any banned-keyword diagnostic.
**Quality gate**:
- [ ] No `unwrap()` in new lexer code paths.
- [ ] No `String::new()` placeholder messages; all diagnostic strings non-empty (Diagnostic constructor enforces, but assert at construction site).
- [ ] Every new test has WHY-comment per global testing rules.
**Verification**: `cargo test -p ynz-parser` passes; snapshot files under `crates/ynz-parser/tests/__snapshots__/` committed; `cargo test --workspace jargon_audit` green.

---

### Phase 2: AST + parser extension (shapes, methods, fields, dot-modifiers, dynamic, struct literals)
**PR scope**: Extend `ynz-ast::nodes` with the M4 AST surface: `Item::ShapeDecl` (with optional extends + zero-or-more follows + base flag + fields + methods), `MethodDecl` (function-like with `share`/`lend`/`give self` receiver + optional `override` flag + optional contract-via-follows source), `FieldDecl` (with optional `hidden` + optional default expression), `Expr::StructLit { name, fields }`, `Expr::FieldAccess { receiver, field }`, `Expr::Modifier { receiver, kind: ShareKind|LendKind|GiveKind|CopyKind|FreezeKind }`, `Stmt::FieldAssign { target_receiver, target_field, value }`, `Type::Shape(ShapeRefId)`, `Type::Dynamic(ContractRefId)`, `Type::SelfType`. Parser handles all M4 surface syntax + struct-literal-vs-block disambiguation + dot-postfix-modifier-vs-method-call distinction.
**Branch**: `feat/m4-parser`
**Flag**: N/A
**Est. lines**: ~900
**Ships via**: `/pr`
**Objective**: Parsing an M4 source like the success-criteria fixture produces an AST matching the committed snapshot, with zero diagnostics. Malformed inputs (struct literal missing comma, method missing receiver position 1, field assign on non-LValue, modifier on non-binding) produce three-part ariadne-rendered errors per snapshot.
**Why this phase exists**: First time M4 syntax is parsed end-to-end. Error-recovery patterns established here are reused across every later milestone that adds expression/statement variants.
**Current-state anchors**:
- `crates/ynz-ast/src/nodes.rs` — current Expr/Stmt/Item variants (`316` lines); M4 extends. Confirm variant-count comment exists per M3 pattern.
- `crates/ynz-parser/src/parser.rs` — Pratt-style precedence climbing; M3 added method-call postfix (around `parse_postfix_chain`); M4 extends with field access, dot-modifiers, struct literals.
- `design/type-system.md` — full spec for shape/follows/extends/override/hidden/dynamic syntax (this is the source-of-truth document).
- `spec/types.md` / `spec/ownership.md` — user-facing spec for the same constructs.
**Files (expected scope)**:
- `crates/ynz-ast/src/nodes.rs` (add new variants; bump variant-count test if M3 had one)
- `crates/ynz-parser/src/parser.rs` (extend with shape parsing, method parsing, field-access postfix, dot-modifier postfix, struct literal vs block disambiguation)
- `crates/ynz-parser/tests/parse.rs` (positive + negative + recovery tests)
- `crates/ynz-parser/tests/__snapshots__/` (new AST snapshots + new error snapshots)
**Deviation rule**: Only the variants listed. NO `Expr::Index` (collections — M5); NO `Expr::Cast` (type conversion — M5); NO `Expr::Match` (deferred; multi-case `if` covers it for M3+); NO union-type syntax in Type position (M6); NO `is` operator (M6).
**Steps**:
1. Add AST variants per spec. `ShapeDecl { name, extends: Option<TypeRef>, follows: Vec<TypeRef>, is_base: bool, fields: Vec<FieldDecl>, methods: Vec<MethodDecl> }`. `MethodDecl` reuses `Stmt::Fn` shape from M3 plus a first-parameter receiver-kind enum (`Share`/`Lend`/`Give` only — methods always have a `self` receiver in M4; `static` methods are M5+ and out of scope). `FieldDecl { name, ty, default: Option<Expr>, is_hidden: bool }`. `Expr::Modifier { receiver, kind: ShareKind|LendKind|GiveKind|CopyKind|FreezeKind }`. `Stmt::FieldAssign { target_receiver, target_field, value }`. Type variants: `Type::Shape { name: Ident }` (resolved to a ShapeId later in typeck), `Type::Dynamic { contract: Ident }`, `Type::SelfType`.
2. Parser — top-level. Extend `parse_item` to recognize `shape Foo { ... }` after the `Shape` token. Method parsing inside a shape body: `function name(share self, ...) -> Ret { ... }` — same shape as M3's free-function parsing, plus a check that the FIRST parameter must be `share self`/`lend self`/`give self`. If not, three-part error: WHAT (method's first parameter must be `self` with an ownership modifier) / WHAT-INSTEAD (write `share self` for read-only access, `lend self` for mutation, `give self` to consume) / WHY (methods are dispatched by receiver type; `self` is the receiver).
3. Parser — expression position. Add `parse_struct_literal` triggered when the parser sees `Identifier { Identifier : ...` (specifically: identifier followed by `{` followed by identifier-or-end-of-args). Use lookahead. Struct-literal-vs-block disambiguation: if `{` is followed by `Identifier Colon` it's a struct literal; otherwise it's a block. `if (cond) { ... }`'s body is unaffected because `cond` is an expression that ends before `)`. Track for failure cases (e.g., `if (true) { x: 1 }` ambiguity) — the `(cond)` parens force this to parse as `if`-with-block, then `{ x: 1 }` is a struct literal at statement position — that's a parse error (struct literal as statement), recover by treating as a block.
4. Parser — postfix chain. Extend M3's `parse_postfix_chain` to recognize `.share`/`.lend`/`.give`/`.copy`/`.freeze` after a `Dot` token. The lexer produces `Dot` + `Identifier("share")` (or `share` is not a keyword at lexer level — it's lowercase, treated as identifier); the parser inspects the identifier name and produces an `Expr::Modifier` if it matches one of the five modifiers, otherwise falls through to existing method-call/field-access parsing.
5. Parser — field access (non-modifier postfix). `value.fieldname` — if the postfix `Dot Identifier` is NOT followed by `LParen` (method call), it's field access — produce `Expr::FieldAccess { receiver, field }`. If followed by `LParen`, it's the existing method-call path from M3.
6. Parser — field assignment. `value.field = expr` — the postfix-chain output is wrapped: if the entire LHS of an `=` is a chain of `Expr::FieldAccess`/`Expr::Identifier` (i.e., an LValue), produce `Stmt::FieldAssign`. Otherwise, three-part error: WHAT (cannot assign to this expression) / WHAT-INSTEAD (assign through a variable or field name) / WHY (only named locations can be the target of `=`).
7. Parser — `dynamic Foo` in type position. `parse_type` recognizes `dynamic Identifier` and produces `Type::Dynamic { contract: Identifier }`. Inside `array<dynamic Foo>`, the inner type lookup also goes through `parse_type`, so this works at any nesting level. (Generics syntax `array<T>` lands in M5; M4 just recognizes `dynamic` at the type-syntax position.)
8. Variant-count ratchet: add `m4_ast_variant_count_locked` test for `Expr`, `Stmt`, `Item`, `Type` variants if M3 didn't have these per-enum. Annotate with `// test-ratchet: M4 adds N for shapes / methods / fields / modifiers / struct literals / dynamic / Self`.
9. Positive parse tests: snapshot the AST for a fixture covering every M4 surface (`Player` shape with two fields, a method, an `extends`, a `follows`, a `hidden` field with default, a method body that uses `self.field = ...`, a call site using `.give`, a struct literal, a `dynamic Drawable` array element).
10. Negative parse tests with snapshot:
    - `shape Player { name }` (missing field type) → three-part error
    - `shape Player { function greet() { ... } }` (method missing `self` receiver) → three-part error
    - `Player {}` (struct literal with no fields where fields are required) → three-part error at struct site (this is also a typeck error later, but parser still succeeds — typeck catches it)
    - `player.field +=` (compound assign — not in M4; only `=` lands) → three-part error pointing to expand-form
    - `let x = player.give.copy` (two modifiers chained — not legal) → three-part error
11. WHY-comments on every test per global rules.
**Acceptance criteria**:
- [ ] AST snapshots for full-coverage positive fixture match.
- [ ] Negative tests produce three-part diagnostics that match committed stderr snapshots.
- [ ] Struct-literal-vs-block disambiguation has dedicated test coverage; ambiguity falls back to block + error.
- [ ] Variant-count ratchet tests in place per AST enum.
- [ ] No banned-jargon in any new parser diagnostic body (`jargon_audit` workspace test confirms).
**Quality gate**:
- [ ] No `unwrap()` in parser code paths.
- [ ] Parser recovers to next item / next stmt at every error point; no panics on malformed input.
- [ ] Postfix-chain handling for `.share`/`.lend`/`.give`/`.copy`/`.freeze` works correctly when chained with method calls (e.g., `player.share.greet()` is a parse-time form — typeck rejects later with a teaching error).
**Verification**: `cargo test -p ynz-parser` passes; snapshots committed.

---

### Phase 3a: Typeck — shape declarations, fields, methods (no inheritance, no ownership analysis yet)
**PR scope**: Extend `ynz-typeck` to: resolve every `ShapeDecl` in a module via a new `shapes` salsa query, populate a per-module ShapeTable mapping `ShapeName → ShapeDef { fields, methods, follows: Vec<ContractRef> }`, type-check method bodies against their declared signatures (reusing M3's two-pass for fns), type-check struct literals (`Foo { a: 1, b: 2 }` matches `Foo`'s fields), type-check field access (`value.field` resolves to the field's declared type), type-check field assignment (LHS must be a field of a non-const binding's owned shape value), type-check the new `Type::Shape` and `Type::SelfType` variants. Hidden-field default-expression resolution. NO inheritance handling yet (P3b); NO ownership analysis (P3c).
**Branch**: `feat/m4-typeck-shapes`
**Flag**: N/A
**Est. lines**: ~1200
**Ships via**: `/pr`
**Objective**: A fixture declaring `shape Player { name: string, health: int }`, constructing `Player { name: "Patrick", health: 100 }`, calling `player.greet()`, reading `player.name`, writing `player.health = 50`, and using `Self` in a method return type — all type-checks cleanly. Negative fixtures (wrong field type, unknown field, method-call-on-primitive, struct-lit-missing-required-field, hidden-field-accessed-outside-shape, default-expression-with-forward-reference) each produce the three-part diagnostic specified in the design.
**Why this phase exists**: Shapes are the spine. Get them right BEFORE layering inheritance (P3b) and ownership (P3c) on top.
**Current-state anchors**:
- `crates/ynz-typeck/src/types.rs:6-42` — current 8-variant Type enum; M4 adds 3 variants (Shape, Dynamic, SelfType). Variant-count comment update required.
- `crates/ynz-typeck/src/scope.rs:8-18` — `ScopeEntry { ty, is_const, is_param, is_loop_var, defined_at }`. P3a does NOT add ownership-tracking fields; those go in P3c.
- `crates/ynz-typeck/src/signatures.rs` — M3's two-pass signature collection (124 lines). M4 extends with method-signature collection via the new `shapes` salsa query.
- `crates/ynz-typeck/src/check.rs:264` — existing `check_assign` for `const` reassignment; M4 extends with field-assignment-on-const check (P3a wires it; P3c finishes the deep-immutability proof).
- `crates/ynz-typeck/src/intrinsics.rs` — `PrimitiveIntrinsicTable` (M2 + M3). M4 has its own method-resolution path for shape methods (not via intrinsic table — that's primitives only).
- `crates/ynz-typeck/src/queries.rs` — salsa query wiring; M4 adds `shapes` query.
**Files (expected scope)**:
- `crates/ynz-typeck/src/types.rs` (add Type::Shape, Type::Dynamic, Type::SelfType; bump variant-count comment to 11; update `type_name`)
- `crates/ynz-typeck/src/shapes.rs` (NEW: ShapeDef + ShapeTable + ShapeId allocator)
- `crates/ynz-typeck/src/queries.rs` (add `shapes` salsa query; wire it before `check`)
- `crates/ynz-typeck/src/check.rs` (handle ShapeDecl items; struct literals; field access; field assignment; method call resolution for shape values; `Self` in method bodies; hidden-field visibility check)
- `crates/ynz-typeck/src/signatures.rs` (method signatures collected via shapes query)
- `crates/ynz-typeck/tests/` (new tests; expand fixture coverage)
**Deviation rule**: P3a does NOT handle `extends` / `follows` resolution beyond storing them in ShapeDef. The actual inheritance-aware type-checking lives in P3b. P3a does NOT handle ownership analysis (modifiers, give-consume tracking, const deep-immutability beyond reassignment). Ownership lives in P3c.
**Steps**:
1. Add Type::Shape, Type::Dynamic, Type::SelfType. Update `type_name`: `Shape { name } => name.as_str()` (PascalCase preserved), `Dynamic { contract } => "dynamic <contract>"`, `SelfType => "Self"`. Variant-count comment → 11.
2. NEW `shapes.rs`: ShapeId allocator (per-module `u32` increment); `ShapeDef { name, fields: Vec<FieldDef>, methods: Vec<MethodDef>, follows: Vec<ShapeId>, extends: Option<ShapeId>, is_base: bool }`; `FieldDef { name, ty, default: Option<ExprId>, is_hidden: bool }`; `MethodDef { name, receiver_kind: ShareKind|LendKind|GiveKind, params: Vec<ParamDef>, return_ty, is_override: bool }`. `ShapeTable` = `HashMap<ShapeName, ShapeId>` + `Vec<ShapeDef>` indexed by ShapeId.
3. NEW `shapes` salsa query in `queries.rs`: `fn shapes(db, source_id) -> Arc<(ShapeTable, DiagnosticBucket)>`. Collects every ShapeDecl from the parsed AST, resolves field types and method signatures (without resolving method bodies — bodies are checked in `check`). Cyclic field dependencies (direct, not via pointer) produce a three-part diagnostic naming the cycle. M3's `signatures` query is the architectural template; this is the same shape but for shapes-and-methods instead of free-functions.
4. Update `check.rs` to consume the ShapeTable:
   - `check_item(Item::ShapeDecl)` — already resolved by `shapes`; no per-item action needed except verifying methods type-check (next steps).
   - `check_expr(Expr::StructLit { name, fields })` — look up `name` in ShapeTable. Verify every required (non-hidden, no-default) field is present; every provided field has correct type; no extra fields. Diagnostic for each violation.
   - `check_expr(Expr::FieldAccess { receiver, field })` — check receiver type is a Shape; look up `field` in the ShapeDef. If `field.is_hidden` AND we're not inside a method of the same shape, three-part error: WHAT (field is hidden — cannot be read here) / WHAT-INSTEAD (move the access inside a method of `<ShapeName>`) / WHY (hidden fields are accessible only to the shape's own methods).
   - `check_stmt(Stmt::FieldAssign { target_receiver, target_field, value })` — same field-resolution as field-access, plus a const-binding check: if the root binding of target_receiver is `is_const`, three-part error per `### Safety` invariant. Hidden-field write also checked.
   - `check_expr(Expr::MethodCall)` — when receiver is a Shape value, look up the method on the ShapeDef. NO inheritance lookup yet (P3b will add the parent-chain walk). Method-call-on-primitive still goes through `PrimitiveIntrinsicTable` (M2/M3 path unchanged).
   - Method body checking: inside a method body, the `self` keyword is a binding of type `Self` (the enclosing shape). `Self` resolves to the concrete shape (not parameterized — M4 has no generics). M3's two-pass approach scales: pre-pass collects every method signature; body-pass checks each body with `self` bound to `Self`.
5. Hidden-field default-expression evaluation. P3a checks the default expression's type matches the field's declared type at the point of declaration (no `self` available — defaults are evaluated at construction site, not declaration site; per the Question #3 locked decision). Default expressions are restricted to constant expressions, empty struct literals, empty array literals (`[]`), empty map literals (`{}`). Anything else → three-part error.
6. Update variant-count tests: `m3_type_variant_count_locked` → `m4_type_variant_count_locked` with ratchet marker (8 → 11).
7. Positive tests:
   - Single shape with two fields + a `share self` method that reads a field
   - Shape with a `lend self` method that writes a field
   - Shape with a `give self` method that consumes self
   - Struct literal across multiple lines
   - Hidden field with `= {}` default; read inside method (allowed); read outside (error — covered in negative)
   - `Self` in return type of a method
8. Negative tests with snapshot (each gets a stderr snapshot via `insta`):
   - `Player { health: 100 }` missing required `name` field → three-part
   - `Player { name: 1, health: 100 }` wrong field type → three-part
   - `Player { age: 30, name: "x", health: 100 }` unknown field → three-part with name suggestion (Levenshtein, like M2's undefined-var)
   - `let p = Player { ... }; print(p.privateThing)` hidden field accessed outside shape → three-part
   - `function f(p: Player) { p.cache = ... }` writing a hidden field outside the shape → three-part
   - `shape A { b: B } shape B { a: A }` direct cyclic field dependency → three-part naming the cycle
   - `shape Player { name: string = self.name }` default-expression referencing self → three-part (`self` not available in default-expression position)
   - `shape Player { name: string = computeDefault() }` non-const default expression → three-part
9. WHY-comments on every test.
**Acceptance criteria**:
- [ ] All M4-spec-compliant shape declarations, struct literals, field accesses, field assignments, and method calls type-check correctly.
- [ ] All 8 negative tests produce three-part diagnostics matching snapshots.
- [ ] Variant-count test for Type ratchets from 8 → 11.
- [ ] `Self` resolves to the enclosing shape's concrete type inside method bodies.
- [ ] Hidden fields are accessible only inside the declaring shape's methods (read AND write).
- [ ] Default expressions restricted to constants + empty literals; anything else errors.
- [ ] `jargon_audit` workspace test green: no banned-jargon in P3a diagnostics.
**Quality gate**:
- [ ] `shapes` salsa query exists and is cache-invalidated by AST changes; smoke test mutating a field and asserting re-computation occurs.
- [ ] No `unwrap()` outside test code.
- [ ] Every M3 fixture passes unchanged on this branch (regression gate).
- [ ] No banned-jargon in any new diagnostic.
**Verification**: `cargo test --workspace` passes; snapshot files committed; M3 fixtures green.

---

### Phase 3b: Typeck — inheritance (`extends`), structural contracts (`follows`), `override`, `dynamic Foo`
**PR scope**: Layer inheritance and contracts on top of P3a's shape system. Resolve `extends` chains (parent-method lookup, field merging at the type level — codegen lays them out in P4). Verify `follows` contracts (every required-by-contract method is provided by the shape — same name + signature). Enforce `override` keyword bidirectionally (missing-when-parent-has + present-when-parent-doesn't). Type-check `dynamic Foo` references in expression and type positions: a `dynamic Foo` value holds any concrete shape that follows Foo; method calls on a `dynamic Foo` are dispatched at runtime (codegen builds the vtable in P4).
**Branch**: `feat/m4-typeck-inheritance`
**Flag**: N/A
**Est. lines**: ~700
**Ships via**: `/pr`
**Objective**: A fixture with `base shape Entity { name: string, function greet(share self) -> string { return self.name } }; shape Player extends Entity { health: int; override function greet(share self) -> string { return "I am " + self.name } }` type-checks cleanly. A `function show(d: dynamic Greetable) { print(d.greet()) }` accepting any concrete shape that follows Greetable also type-checks. Negative fixtures cover: missing-override, extra-override, follows-contract-method-missing, base-shape-instantiation, cyclic-extends, override-with-wrong-signature.
**Why this phase exists**: P3a delivers shapes; P3b makes them composable. Type-system completeness — without this phase M4 is "shapes" not "shapes with inheritance and contracts."
**Current-state anchors**:
- `crates/ynz-typeck/src/shapes.rs` (added in P3a) — ShapeDef stores `extends` + `follows`; P3b adds the resolution logic.
- `crates/ynz-typeck/src/check.rs` — P3a added field-access/field-assign/method-call paths for shapes; P3b extends method-lookup to walk the `extends` chain.
- `design/type-system.md:15-46` — single-inheritance + override + multiple-follows spec.
- `design/type-system.md:104-110` — `dynamic` example (heterogeneous collection).
**Files (expected scope)**:
- `crates/ynz-typeck/src/shapes.rs` (extends-chain walker, follows-contract verifier, override checker, base-shape instantiation guard)
- `crates/ynz-typeck/src/check.rs` (method lookup with parent-chain; dynamic dispatch type-check; struct-literal-rejection on base shapes)
- `crates/ynz-typeck/tests/` (inheritance + follows + dynamic tests)
**Steps**:
1. Add `extends`-chain resolver: given a ShapeId, walk parent pointers up to root; reject cycles (three-part diagnostic naming the cycle).
2. `follows`-contract verifier: for every `ShapeName follows Contract`, verify the shape (including inherited methods) provides every method the contract declares with matching signature. Three-part diagnostic per missing method names the contract + the method name + the expected signature.
3. `override` checker (bidirectional):
   - For every method in the shape's body with `is_override: true`, look up the same method name in the parent-chain. If not found → three-part WHAT (method declared `override` but parent doesn't have it) / WHAT-INSTEAD (remove `override` if this is a new method, or check the parent has the method spelled the same way) / WHY (override prevents typo-shadowing — the keyword guarantees you're replacing a known parent method).
   - For every method in the shape's body NOT marked `override`, check the parent-chain. If found with the same signature → three-part WHAT (method shadows parent's method without `override`) / WHAT-INSTEAD (add `override` keyword if intentional, rename if not) / WHY (silent shadowing is a typo magnet — see `design/type-system.md:41-47`).
4. Base-shape instantiation guard. In `check_expr(Expr::StructLit { name, .. })` — BEFORE field-resolution — look up `name` in the ShapeTable and check `shape_def.is_base`. If set, emit three-part WHAT (cannot instantiate a base shape — base shapes are partial declarations meant to be extended) / WHAT-INSTEAD (declare a derived shape and instantiate it, or remove the `base` keyword from Player) / WHY (`base` exists for sharing fields and methods across multiple derived shapes; instantiating it would leave some semantic gap). The early-check ordering matters: if a base-shape struct literal also has wrong field types, we emit the base-instantiation error FIRST (it's the higher-level violation) rather than cascading two errors.
5. Method-lookup with parent-chain: when resolving `value.method()` on a shape value, search the receiver's ShapeDef first; if not found, walk `extends` to parent's ShapeDef. Stop at first match. If found, dispatch is STATIC (concrete type is known). No virtual-method-table; the parent-method is inlined directly via the parent's signature.
6. `dynamic Foo` typeck. Type-checking a `let x: dynamic Foo = somePlayer` requires `Player` to follow Foo (asserted via the `follows`-resolver). Method call on a `dynamic Foo` resolves via the CONTRACT's method-table, NOT the receiver's concrete shape (because we don't know the concrete shape at compile time). Codegen builds the vtable (P4); typeck just confirms the method exists on the contract.
7. Inheritance-method-signature check. An overriding method MUST have the same signature as the parent's method (same param types, same return type, including receiver kind). Mismatch → three-part error.
8. Field-inheritance type-check. A child's struct literal must provide values for ALL non-default fields of both parent AND child. Hidden parent fields are inaccessible from the child — including in struct-literal construction OUTSIDE the child's methods (the parent's default for hidden fields kicks in unless the child explicitly initializes them via a method that has parent-private access — but hidden is shape-private, not extension-public, so child can't init them either; this is the M4 locked decision).
9. Positive tests:
   - `Entity` parent + `Player extends Entity` with one inherited method + one override method
   - `Entity` + `Drawable` contract + `Player extends Entity follows Drawable` (parent + contract, parent provides one method, child provides another)
   - `dynamic Drawable` array of mixed concrete types with shared method call
   - Two contracts `Damageable, Attackable` followed by one shape (no inheritance)
10. Negative tests with snapshot:
    - `override function greet()` in `Player extends Entity` where `Entity` doesn't have `greet` → three-part
    - `function greet()` in `Player extends Entity` where Entity DOES have `greet` (missing `override`) → three-part
    - `shape Player follows Comparable { name: string }` (missing `compare` method) → three-part naming the contract + method + signature
    - `base shape Entity { ... }; Entity { name: "x" }` (base instantiation) → three-part
    - `shape A extends B`; `shape B extends A` (cyclic extends) → three-part
    - Override with wrong return type → three-part comparing expected vs actual signature
    - Override with wrong receiver kind (`override function greet(lend self)` when parent is `share self`) → three-part
    - **Cyclic `follows` graph**: contracts declaring they follow other contracts in a cycle (`shape Contract A follows B; shape Contract B follows A`) → three-part naming the cycle. Verify against `design/type-system.md` whether contracts-that-follow-other-contracts is even legal in M4; if NOT legal, the test asserts a different three-part error (contract cannot follow another contract — contracts are leaf nodes in the follows graph). Either way, locked here.
    - **`dynamic Foo` returned across a function boundary**: `function pickGreeter() -> dynamic Greetable { return Player { ... } }`. Positive test (returning a fat pointer works) + negative test (returning a concrete type with no follows relationship → three-part error).
    - **`.copy` of shape inheriting from a `base shape` with non-trivial fields**: `base shape Owner { name: string, owned: array<int> }; shape Player extends Owner { health: int }` — `.copy` on a Player must traverse inherited fields too; transitive-trivially-copyable check walks the inheritance chain. Negative test asserts the error names the `owned: array<int>` field in the parent.
**Acceptance criteria**:
- [ ] Inheritance + follows + override resolution works correctly for all positive fixtures.
- [ ] All 10 negative tests produce three-part diagnostics matching snapshots (7 originally listed + cyclic-follows + dynamic-returned-across-boundary + copy-of-inheritance-with-non-trivial-parent-field).
- [ ] Cycles in `extends` chain produce a three-part error naming the cycle.
- [ ] `dynamic Foo` type-checks cleanly when the concrete type follows Foo; errors when it doesn't.
- [ ] Method lookup walks parent-chain; static dispatch chosen when concrete type is known; dynamic dispatch chosen when receiver is `dynamic Foo`.
- [ ] All M3 + P3a fixtures still pass on this branch.
**Quality gate**:
- [ ] Parent-chain walker has cycle detection (not relying on call-stack overflow).
- [ ] No `unwrap()` outside test code.
- [ ] No banned-jargon in any P3b diagnostic.
**Verification**: `cargo test --workspace` passes; snapshots committed; M3 + P3a fixtures green.

---

### Phase 3c: Typeck — ownership analysis salsa query (the borrow checker)
**PR scope**: Add a third salsa query, `ownership(db, source_id) -> Arc<(OwnershipReport, DiagnosticBucket)>`, that performs flow-sensitive borrow checking on every function and method body. Tracks: (a) per-binding live/consumed/freezed/borrowed states; (b) outstanding `.share`/`.lend` borrows; (c) `.give` consume points; (d) `.copy` permission per type (transitively trivially-copyable check); (e) `.freeze` flips a binding's mutability state. Enforces: `const` blocks `.lend`/`.give`/field-mutation; `let` allows `.lend` only when no other live borrow exists; `.give` consumes a binding (use-after-give = error); branches must agree on consume-set at merge points. Produces a per-function OwnershipReport that codegen (P4) reads to emit drops and LLVM attributes.
**Branch**: `feat/m4-typeck-ownership`
**Flag**: N/A
**Est. lines**: ~1500 (largest single phase — the borrow checker is the heart of the milestone)
**Ships via**: `/pr`
**Objective**: A fixture exercising every ownership rule (share-of-const-OK, lend-of-const-ERROR, give-then-use-ERROR, give-then-give-ERROR, give-in-one-branch-only-ERROR, freeze-then-mutate-ERROR, copy-of-non-trivially-copyable-ERROR, two-simultaneous-lends-ERROR) produces the expected outcome (each negative produces a three-part diagnostic with span on the give-site AND the use-site; each positive type-checks cleanly). OwnershipReport is consumed correctly by a smoke test (no codegen yet — that's P4).
**Why this phase exists**: This IS the language's safety guarantee. Per design-lockdown locked decisions + graveyard Entry 1 (critical) + `### Safety` invariants below — `const` deep-immutability cannot be enforced without this query, and use-after-give is a class of bugs Yinz refuses to ship.
**Current-state anchors**:
- `crates/ynz-typeck/src/scope.rs:8-18` — `ScopeEntry` adds new flags: `is_consumed: bool`, `is_freezed: bool`, `outstanding_shares: u32`, `outstanding_lend: Option<SpanId>`.
- `crates/ynz-typeck/src/check.rs` — P3a/P3b establish field-assign-on-const error; P3c expands to all four mutation paths.
- `design/ownership.md:33-76` — the full const deep-immutability spec.
- `.claude/graveyard.md:14-37` — Entry 1's Bouncer-enforceable detection signature.
**Files (expected scope)**:
- `crates/ynz-typeck/src/scope.rs` (add ownership-tracking flags to ScopeEntry)
- `crates/ynz-typeck/src/ownership.rs` (NEW — the borrow checker; expected ~800 lines)
- `crates/ynz-typeck/src/queries.rs` (wire `ownership` query; depends on `check` + `shapes`)
- `crates/ynz-typeck/src/check.rs` (integrate ownership checks into Expr::Modifier handling; emit modifier-error diagnostics)
- `crates/ynz-typeck/tests/` (ownership rule coverage: ~30 tests minimum, each rule positive + negative)
**Steps**:
1. Extend `ScopeEntry` with ownership-tracking flags: `is_consumed: bool`, `is_freezed: bool` (set true after `.freeze`), `outstanding_share_count: u32` (number of active read-borrows), `outstanding_lend: Option<SpanId>` (Some when a mutable borrow is currently outstanding — only one at a time).
2. NEW `ownership.rs`: the per-function borrow-check pass. Walks the function body's AST in execution order (within branches, both sides analyzed independently, then merged at branch-merge points). For each statement / expression that touches a binding, update the binding's state per the rules:
   - `let binding = expr` — new owned binding, drop-list adds it; state: live.
   - `const binding = expr` — same as let plus `is_const: true`.
   - `binding.share` — outstanding_share_count += 1; valid when binding is live AND no outstanding_lend.
   - `binding.lend` — outstanding_lend = Some(span); valid when binding is live AND outstanding_share_count == 0 AND NOT is_const. If `is_const`: three-part WHAT (`.lend` requires mutable access; binding is `const`) / WHAT-INSTEAD (declare with `let` if mutation is intended; the function signature requires `lend` so the caller commits to mutation) / WHY (see `design/ownership.md:33-76`).
   - `binding.give` — `is_consumed = true`; remove from drop-list (receiver inherits); valid when binding is live AND outstanding_share_count == 0 AND outstanding_lend is None AND NOT is_const. If `is_const`: three-part WHAT (`.give` transfers ownership; binding is `const` and cannot be transferred) / WHAT-INSTEAD (use `.share` or `.lend` if you need to pass it; declare with `let` if you need to transfer ownership) / WHY.
   - `binding.copy` — produces a new owned value; binding state unchanged. Validity: check the binding's type is trivially copyable (transitively: every field of every field… is a primitive or another trivially-copyable shape). If not: three-part WHAT (cannot `.copy` — type X contains field Y of non-trivially-copyable type Z) / WHAT-INSTEAD (define a `copy()` method that explicitly handles non-trivial fields; or use `.give` to transfer ownership) / WHY (.copy is the cheap-by-design escape valve; non-trivial copy needs explicit semantics).
   - `binding.freeze` — `is_freezed = true`; `is_const = true` from this statement onward; valid when binding is live AND outstanding_share_count == 0 AND outstanding_lend is None.
   - `binding.field = value` — same as field-assign in P3a but ALSO requires `outstanding_lend == None`, `outstanding_share_count == 0`, NOT `is_const`. Each violation → three-part error.
   - Use of `binding` (read, method call) where `is_consumed == true` → three-part WHAT (use-after-give — binding was transferred away) / WHAT-INSTEAD (use `.share` or `.lend` to pass without transferring) / WHY, with span on the give-site (recorded at `binding.give` consume time) AND on the use-site.
3. Branch-merge rule: at the end of every `if`/`else` branch (and `while` body, and `for` body), the set of consumed bindings must MATCH across all branches. If one branch consumes a binding and another doesn't, three-part WHAT (binding `x` is consumed in one branch but not another — use after this would be undefined) / WHAT-INSTEAD (consume in all branches, or in none — restructure the conditional to consume consistently) / WHY.
4. Outstanding-borrow tracking decreases when scope exits. Borrows are scope-tied (function-scoped in M4 — no lifetime annotations needed since there's no cross-function borrowing yet; M4 functions take owned, shared, or mutably-borrowed values as parameters but don't return borrows because we don't have lifetime parameters yet — RETURNING a borrow is an M5+ concern when lifetimes land).
5. Function-call ownership analysis. When `f(arg)` is called and `f`'s signature is `function f(share x: T)`, the arg is `.share`'d at the call site (inferred if not explicit). If signature is `function f(lend x: T)`, the arg is `.lend`'d. If signature is `function f(give x: T)`, the arg is `.give`'d (and consumed). Per the inverse anti-pattern graveyard entry: the user does NOT type `.share` at the call site — it's inferred. The IDE will show the inferred modifier (v0.2 LSP).
6. `Self`-receiver ownership analysis. Method `function greet(share self)` analyzed exactly like a `share self: Self` parameter. Method `function consume(give self)` consumes the receiver at the call site.
7. OwnershipReport struct: per-function `{ drop_list: Vec<SpanId>, give_consumes: HashMap<BindingId, SpanId>, parameter_kinds: Vec<(ParamName, OwnershipKind)>, conditional_drops: HashMap<BindingId, bool> }`. Codegen (P4) reads this to emit drops and LLVM attributes.
8. `ownership` salsa query wired in `queries.rs`, depending on `check` and `shapes`. Cache-invalidates on AST + ShapeTable changes. Smoke test: mutate a function body and assert the query re-runs.
9. Positive tests (~12, covering each rule's success path):
   - Multiple `.share` of the same binding allowed simultaneously
   - One `.lend` followed by drop, then a second `.lend` allowed
   - `.give` followed by no use of binding (binding silently dropped from drop-list)
   - `.copy` on primitives (already trivially copyable)
   - `.copy` on shape of only-primitive fields (transitively trivially copyable)
   - `.freeze` followed by `.share` (allowed — frozen is just const)
   - Field write through a `lend self` method body
   - Branch-merge with consistent consumes (binding `.give`'d in both branches)
10. Negative tests (~18, covering every error rule):
    - `.lend` on `const` binding → three-part
    - `.give` on `const` binding → three-part
    - Field write `const_binding.field = x` → three-part
    - Use after `.give` → three-part with give-site + use-site spans
    - Two simultaneous `.lend`s of the same binding → three-part
    - `.lend` while a `.share` is outstanding → three-part
    - `.share` while a `.lend` is outstanding → three-part
    - `.copy` on shape with a non-trivially-copyable field → three-part
    - `.give` in one branch, not the other → three-part (branch-merge violation)
    - `.lend` carried across branch-merge with one side consuming → three-part
    - Use of a `.freeze`d binding for `.lend` → three-part
    - `.give` of a parameter passed `share` → three-part WHAT (cannot give a shared-borrowed parameter) / WHAT-INSTEAD (change the signature to `give`) / WHY
    - Field assign in a `share self` method body → three-part WHAT (cannot mutate fields through a `share self` receiver) / WHAT-INSTEAD (change to `lend self`) / WHY
    - **`.give` of `self` from inside a method body, then continued use of `self` in the same body**: `function consume(give self) -> nothing { someFn(self); print(self.name) }`. Use-after-give detected; the diagnostic must point at the `give self` parameter declaration AND the offending later use, in the same function. Ariadne related-span path must work intra-function.
    - **`.lend self` method internally calls a `.share self` method on the same value (re-borrow check)**: `function update(lend self) { let snapshot = self.snapshot() }` where `snapshot()` is `share self`. **Locked design decision**: allowed — the outer `.lend` is paused for the duration of the inner call (M4 borrow-checker treats nested method calls as borrowed-suspended-then-resumed). Positive test confirms this; if it errors, the diagnostic must explain why (and the design needs revisiting).
11. WHY-comments on every test.
12. **No codegen changes in P3c.** P3c outputs ONLY diagnostics + OwnershipReport. Codegen consumes the report in P4. This split lets P3c be reviewed for borrow-check correctness independently of codegen complexity.
**Acceptance criteria**:
- [ ] All 13 positive ownership tests pass (12 listed + the `.lend self` → nested `.share self` re-borrow case).
- [ ] All 20 negative ownership tests produce three-part diagnostics matching committed snapshots (18 listed + the `.give self` mid-method case + cyclic-follows OR re-borrow-rejection depending on the design decision flip).
- [ ] Every negative test for a `const`-mutation path (lend/give/field-write/inferred-lend) explicitly names "const" in the diagnostic and references the `let` alternative.
- [ ] Use-after-give diagnostic carries BOTH the give-site span and the use-site span (Ariadne related-span feature).
- [ ] Branch-merge analysis catches every inconsistent-consume case.
- [ ] OwnershipReport is produced for every function and method; smoke test reads a report and asserts expected drop-list size.
- [ ] `ownership` salsa query cache-invalidates on AST changes.
- [ ] All M3 + P3a + P3b fixtures still pass on this branch.
- [ ] `jargon_audit` green on every ownership diagnostic.
**Quality gate**:
- [ ] Borrow-check correctness verified against design/ownership.md spec — every rule has a dedicated test (positive + negative).
- [ ] No `unwrap()` outside test code.
- [ ] No fallthrough states (e.g., `is_consumed && is_freezed` simultaneously) — assert these can't happen by construction OR test them explicitly.
- [ ] No banned-jargon (specifically: `infer`/`inference`/`narrowing` MUST NOT appear in ANY diagnostic produced by P3c).
**Verification**: `cargo test --workspace` passes; snapshots committed; M3 + P3a + P3b fixtures green; OwnershipReport smoke test passes.

---

### Phase 4: Codegen — shape memory layout, methods, heap allocation, drops, LLVM attributes, dynamic vtables
**PR scope**: Extend `ynz-codegen` to lower every M4 surface to LLVM IR: shape memory layout (LLVM struct type per shape), method-call lowering (static dispatch for concrete receivers; dynamic dispatch via vtable for `dynamic Foo`), heap allocation (`malloc` via `ynz_alloc`), drop-on-scope-exit (`free` via `ynz_free`, reverse declaration order, drop-flag for conditionally-consumed bindings), LLVM `readonly` attribute on every `share T` param (and every param inferred from a const binding), LLVM `noalias` attribute on every parameter the borrow check proved non-aliased, vtable globals for every `(concrete_shape, dynamic_contract)` pair used in the module. Consumes P3c's OwnershipReport for drop emission and parameter-attribute emission.
**Branch**: `feat/m4-codegen`
**Flag**: N/A
**Est. lines**: ~1300
**Ships via**: `/pr`
**Objective**: The success-criteria fixture (`examples/m4_player.ynz`) compiles to a binary that prints expected output. LLVM IR snapshot tests assert: `readonly` on `share` params, `noalias` on borrow-check-proven-non-aliased params, no `readonly` on `give` params, `malloc`/`free` calls emitted at correct positions, vtable globals declared once per `(shape, contract)` pair.
**Why this phase exists**: Types are useless without codegen. The LLVM attribute contract from `design/ownership.md:51-66` and graveyard Entry 1 is enforced only when codegen actually emits the attributes.
**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs` — M1/M2/M3 lowering (currently 1084 lines). M4 extends with shape struct types, method definitions, heap alloc/free calls, drops, attributes.
- `crates/ynz-codegen/src/runtime_decls.rs:151` — extern C declarations; M4 adds `ynz_alloc(size: usize) -> *mut u8` and `ynz_free(ptr: *mut u8, size: usize)`.
- `crates/ynz-runtime/src/` — currently has decimal128 + string + print runtime; M4 adds `ynz_alloc` and `ynz_free` thin-wrapping libc malloc/free (with abort-on-OOM behavior locked in `### Runtime Dependencies`).
- `crates/ynz-typeck/src/ownership.rs` (added in P3c) — OwnershipReport is the input to codegen's drop emission + attribute emission.
- `design/ownership.md:51-66` — LLVM attribute contract: `readonly` on share; `noalias` on borrow-checker-proven non-aliased.
**Files (expected scope)**:
- `crates/ynz-codegen/src/emit.rs` (extend with shape lowering, method lowering, heap alloc/free, drops, attributes, dynamic vtables)
- `crates/ynz-codegen/src/runtime_decls.rs` (add `ynz_alloc`/`ynz_free` extern decls)
- `crates/ynz-codegen/src/vtable.rs` (NEW — vtable globals for dynamic dispatch)
- `crates/ynz-runtime/src/lib.rs` (add ynz_alloc + ynz_free C-ABI functions)
- `crates/ynz-codegen/tests/` (LLVM IR snapshot tests + binary-execution tests)
- `crates/ynz-driver/tests/fixtures/m4_*.ynz` (positive end-to-end fixtures)
**Steps**:
1. **Shape struct types.** For every ShapeDef in the ShapeTable, emit an LLVM struct type `%ShapeName = type { field0_ty, field1_ty, ... }`. Field order matches declaration order in M4 (auto-reorder is v0.3+ per `.claude/rules/auto-promotion.md`). Hidden fields are LAID OUT identically to visible fields — visibility is a typeck concern, not a layout concern. Inherited shapes lay parent fields first, then child fields (single-inheritance struct embedding, classic Rust/C++ approach without virtual methods).
2. **Struct literal lowering.** `Player { name: "Patrick", health: 100 }` lowers to:
   - `%player = alloca %Player` (stack allocation; codegen later promotes to heap if the value's lifetime is non-stack — see step 3)
   - `store "Patrick", %player.name`
   - `store 100, %player.health`
   - If hidden fields have defaults, lower each default expression and store at the right offset
3. **Heap-allocation decision.** For M4, shapes are STACK-allocated by default (in their lexical scope's `alloca`). Heap allocation is needed only when ownership crosses a scope boundary in a way the stack can't represent (e.g., a shape value is `.give`d to a function that stores it past the caller's frame). The borrow checker (P3c) annotates each binding in OwnershipReport with `allocation_strategy: Stack | Heap`. Codegen emits `alloca` for Stack and `ynz_alloc(sizeof(T))` for Heap.
   - **For M4 simplicity**: default is Stack; promote to Heap only when borrow-check proves the value outlives its declaration scope. M4's typeck reports `Stack` for every local that doesn't escape; reports `Heap` for values explicitly built into a heap-bound container or moved into a return slot the caller doesn't already own.
   - Without escape-analysis depth, M4 conservatively uses Heap when a `.give` crosses a function boundary AND the receiver function declares ownership.
4. **Method dispatch.** Static dispatch is the default. `player.greet()` resolves to `call @Player_greet(%player.share)`. Method names are mangled `%ShapeName_methodName`. Override methods at child shapes don't change the mangling — `Player_greet` is the child's, `Entity_greet` is the parent's. Parent-method calls from the child are EXPLICIT (M4 doesn't add a `super.method()` syntax yet — the child must redeclare and call `Entity_greet(...)` if it needs parent behavior; cleaner alternatives in M5+).
5. **Dynamic dispatch.** A `dynamic Foo` value is a fat pointer `{ data_ptr: ptr, vtable_ptr: ptr }`. The vtable is a per-(concrete_shape, contract) compile-time global named `@vtable_<ShapeName>_<ContractName>` containing function pointers indexed by method slot. Method call `d.greet()` lowers to:
   - `vtable = load %d.vtable_ptr`
   - `fn_ptr = load vtable[greet_slot]`
   - `call fn_ptr(%d.data_ptr.share)`
6. **`.share` lowering.** A `.share` produces an LLVM `i8*` (or typed pointer) to the borrowed value. For stack-allocated values, this is the alloca pointer. The LLVM `readonly` attribute is added to the parameter at the CALLEE side (caller doesn't need to do anything special).
7. **`.lend` lowering.** Same as `.share` but the parameter at the CALLEE side gets `noalias` (no `readonly`).
8. **`.give` lowering.** The value's bits are copied into the receiver's argument slot. The original binding's drop-list entry is removed (handled by P3c's OwnershipReport). The receiver inherits the drop responsibility.
9. **`.copy` lowering.** For trivially-copyable shapes, emit `memcpy(new_alloca, %old, sizeof(T))`. Original binding remains live; new binding gets its own drop-list entry. P3c already verified transitive-trivially-copyable; codegen can emit memcpy unconditionally.
10. **`.freeze` lowering.** No-op at codegen — frozen is a typeck-only flag.
11. **Drop emission.** At every scope exit (function return, block end), iterate the scope's drop-list in REVERSE declaration order. For each non-consumed binding, emit `call @ynz_free(%binding, sizeof(T))` if the binding is heap-allocated. Stack-allocated bindings drop implicitly with the `alloca`. Per-binding drop-flags for conditionally-consumed bindings (set by P3c) are runtime bools — initialize true at declaration, flip to false at `.give`, check at scope-exit drop emission.
12. **LLVM attribute emission.** Per `### Performance` invariant:
    - Every `share T` parameter → `readonly` + `noalias` (if borrow-check proven non-aliased)
    - Every param inferred from a `const` binding at the CALL site (even if signature says `share` already) → `readonly`
    - Every `lend T` parameter → `noalias` (no `readonly`)
    - Every `give T` parameter → no `readonly`, no `noalias` (the callee owns it)
13. Runtime crate additions. `ynz-runtime` adds:
    ```rust
    #[no_mangle] pub unsafe extern "C" fn ynz_alloc(size: usize) -> *mut u8 { libc::malloc(size).cast() }
    #[no_mangle] pub unsafe extern "C" fn ynz_free(ptr: *mut u8, _size: usize) { libc::free(ptr.cast()) }
    ```
    `ynz_alloc` panics-then-aborts on null (OOM); kernel-mode plug-in API replaces this in v0.3+ per `### Kernel-Mode Behavior`.
14. **IR snapshot tests.** For each parameter-kind / dispatch / allocation combination, emit LLVM IR text and snapshot-match against a committed fixture. Snapshots committed under `crates/ynz-codegen/tests/__snapshots__/m4_ir/`. Each snapshot has a WHY-comment naming the invariant it protects.
15. **Binary-execution tests.** `examples/m4_player.ynz` and friends → compile + run + assert stdout matches expectation.
16. **Drop-flag runtime test.** A fixture conditionally `.give`s a value inside an `if`. Runtime asserts that `ynz_free` is called exactly once (not zero, not twice) regardless of branch taken.
**Acceptance criteria**:
- [ ] All M4 fixtures compile and run; stdout matches expected.
- [ ] LLVM IR snapshot tests pass for every parameter-kind / dispatch / allocation combo.
- [ ] `readonly` attribute appears on `share T` parameters in IR text (asserted by snapshot).
- [ ] `noalias` attribute appears on `share T` AND `lend T` parameters (when borrow-check proved non-aliased).
- [ ] `readonly` appears on parameters inferred from `const` bindings, even when signature is `share`.
- [ ] Dynamic dispatch fixture: `array<dynamic Greetable>` with mixed concrete elements runs correctly.
- [ ] No memory leaks under valgrind for a fixture that allocates + drops + .give's.
- [ ] All M3 + P3a + P3b + P3c fixtures still pass (regression gate).
- [ ] `jargon_audit` workspace test green.
**Quality gate**:
- [ ] No `unwrap()` in emit.rs paths.
- [ ] All `ptr` types correctly typed (no opaque `i8*` where typed pointers are expected per inkwell 0.9 conventions — same discipline as M1-M3 emit).
- [ ] Drop-flag runtime overhead is zero when borrow-check proves a binding is consumed unconditionally (only emit drop-flag for the conditional case).
- [ ] Inkwell types stay inside emit.rs; `CompiledArtifact` exposes only `Vec<u8>` IR bitcode + SHA-256 (M1's discipline carried through).
- [ ] No banned-jargon in any new codegen-error message (if any are emitted at codegen time, though typically codegen errors are panics — M1 discipline).
**Verification**: `cargo test --workspace` passes; binary-execution tests pass; valgrind clean on heap-alloc fixture; IR snapshots committed.

---

### Phase 5: Catch-up — overflow escape methods + type-attached constants
**PR scope**: Close the two M2-deferred catch-up entries by leveraging M4's general method-dispatch infrastructure (built in P3a/P4):
- Overflow escape methods on `int`: `.wrappingAdd(i: int) -> int`, `.wrappingSub(i: int) -> int`, `.wrappingMul(i: int) -> int`, `.saturatingAdd(i: int) -> int`, `.saturatingSub(i: int) -> int`, `.saturatingMul(i: int) -> int`. These are non-panicking arithmetic alternatives to `+`/`-`/`*` on `int`. Implementation: LLVM intrinsics (`llvm.uadd.sat.i64` for saturating; bare wrapping arithmetic without `with.overflow` check for wrapping).
- Type-attached constants: `int.max`, `int.min`, `number.epsilon`, `number.max`, `number.min`. Parsed as `Identifier(int) Dot Identifier(max)`; in expression position, when the receiver is a Type-name, look up the type's attached constants.
- Each closure deletes its M2 deferral-stderr snapshot AND adds a success-stdout snapshot AND updates `v0-1-compiler.md` to mark the catch-up entry CLOSED with the commit SHA.
**Branch**: `feat/m4-catchup-numerics`
**Flag**: N/A
**Est. lines**: ~600 (small phase; catches up M2 debt without spilling into M4's spine)
**Ships via**: `/pr`
**Objective**: `let x: int = 9223372036854775807; let y = x.wrappingAdd(1); print(y)` compiles, runs, and prints `-9223372036854775808`. `print(int.max)` compiles, runs, prints `9223372036854775807`. M2 catch-up fixtures `m2_wrapping_add_deferred.ynz` and `m2_int_max_deferred.ynz` are CONVERTED: stderr snapshot → success-stdout snapshot. M2 plan in `v0-1-compiler.md` gets the catch-up entries marked DONE.
**Why this phase exists**: M2 explicitly named M4 as the owner of these features (`v0-1-compiler.md:651-664`). M4 has methods AND type-attached intrinsics infrastructure; this phase wires them up without touching the type-system spine in P1-P4.
**Current-state anchors**:
- `crates/ynz-typeck/src/intrinsics.rs` — `PrimitiveIntrinsicTable` from M2; M4 extends with 6 new int methods.
- `crates/ynz-driver/tests/fixtures/m2_wrapping_add_deferred.ynz` — M2 catch-up fixture, currently asserts a deferral diagnostic. Convert to success.
- `crates/ynz-driver/tests/fixtures/m2_int_max_deferred.ynz` — same.
- `.claude/plans/active/v0-1-compiler.md:651-664` — M2 catch-up list; mark entries CLOSED in same PR.
**Files (expected scope)**:
- `crates/ynz-typeck/src/intrinsics.rs` (add 6 wrapping/saturating int methods)
- `crates/ynz-typeck/src/check.rs` (handle Type-name-in-expression-position lookup for `int.max` etc.)
- `crates/ynz-codegen/src/emit.rs` (lower wrapping/saturating to LLVM intrinsics; lower type-attached constants to immediate-value loads)
- `crates/ynz-driver/tests/fixtures/m2_wrapping_add_deferred.ynz` (convert from deferral to success)
- `crates/ynz-driver/tests/fixtures/m2_int_max_deferred.ynz` (convert from deferral to success)
- `crates/ynz-driver/tests/__snapshots__/` (delete deferral-stderr snapshots; add success-stdout snapshots)
- `.claude/plans/active/v0-1-compiler.md` (mark M2 catch-up entries closed)
**Steps**:
1. Extend `PrimitiveIntrinsicTable` with 6 method entries on `int`: each takes `(self: int, other: int) -> int`. Wrapping methods: native `add`/`sub`/`mul` with overflow ignored (compile to `add nuw`-free `add` — wrapping is the default no-overflow-check operation). Saturating methods: lower to `llvm.uadd.sat.i64`, `llvm.usub.sat.i64`, `llvm.smul.fix.sat.i64` (or signed variant; confirm intrinsic name during P5).
2. Add type-attached constants: `int.max = 9223372036854775807`, `int.min = -9223372036854775808`, `number.epsilon` (decimal128 smallest representable positive — exact value from `design/numeric-types.md`), `number.max`, `number.min`. Constant table: `HashMap<(TypeName, ConstName), ConstValue>` in `intrinsics.rs`.
3. Parser handling for type-attached constants. `int.max` parses as `Expr::FieldAccess { receiver: Expr::TypeName("int"), field: "max" }`. The parser already handles `Identifier Dot Identifier` for method calls and field access; we need to recognize when the receiver is a Type-name (`int`, `float`, `number`, `bool`, `string`). Since M3 already parses `int` as a Type only in type-annotation position, M4 adds a parser path: in expression position, an identifier that matches a primitive type name OR a user-shape name is parsed as a `TypeName` reference; followed by `Dot Identifier` it becomes a type-attached lookup.
4. Typeck for type-attached constants: when `Expr::FieldAccess` has a TypeName receiver, look up the constant in the `(TypeName, ConstName)` table. Three-part error if not found, naming available constants on that type.
5. Codegen for wrapping/saturating: lower each method-call expression to the corresponding LLVM intrinsic. For wrapping: `add %a, %b` (no overflow check). For saturating: `call @llvm.uadd.sat.i64(%a, %b)`.
6. Codegen for type-attached constants: emit immediate-value loads (`i64 9223372036854775807` etc.). Decimal128 constants emit via the runtime decimal128 constructor (existing M2 path).
7. Convert M2 catch-up fixtures:
   - `m2_wrapping_add_deferred.ynz`: rewrite the source to use `.wrappingAdd(1)` instead of `1 + ...` (or whatever the deferral fixture had). Update expected: success-stdout `-9223372036854775808`.
   - `m2_int_max_deferred.ynz`: rewrite to use `int.max`. Update expected: success-stdout `9223372036854775807`.
8. Delete M2 deferral-stderr snapshots from `__snapshots__/` with explicit reason in commit message: "P5: M2 catch-up closed — was previously a deferral fixture, now success".
9. Add success-stdout snapshots for both fixtures.
10. Mark M2 catch-up entries in `v0-1-compiler.md` as CLOSED with commit SHA annotation.
11. WHY-comments on every test.
**Acceptance criteria**:
- [ ] All 6 wrapping/saturating int methods type-check + lower + run correctly.
- [ ] All 5 type-attached constants (int.max, int.min, number.epsilon, number.max, number.min) compile + run.
- [ ] M2 catch-up fixtures `m2_wrapping_add_deferred.ynz` and `m2_int_max_deferred.ynz` converted; deferral snapshots deleted; success snapshots committed.
- [ ] `v0-1-compiler.md` M2 catch-up section reflects closed status with commit SHA.
- [ ] All M1-M3 tests still pass; jargon audit green.
**Quality gate**:
- [ ] LLVM intrinsic names confirmed against LLVM 18 documentation (no guessing — exact spellings).
- [ ] No `unwrap()` in code paths.
- [ ] Deferral-snapshot deletion explicit in commit message (auditable).
**Verification**: `ynz run examples/wrapping_demo.ynz` produces expected output; `cargo test --workspace` green; deferral snapshots are gone from filesystem.

---

### Phase 6: Driver integration + M4 fixture suite + heap leak sanity
**PR scope**: Driver wires P1-P5 into the pipeline (already covered by salsa for typeck queries; codegen queries get the same wiring). Add an M4 fixture suite covering every major feature (shape + method + inheritance + follows + override + hidden + dynamic + share/lend/give/copy/freeze + heap alloc + drop). Add a valgrind CI smoke run for the heap-allocating fixture. Add IR-snapshot diff-runner that warns when an M3 fixture's IR changes unexpectedly.
**Branch**: `feat/m4-driver-fixtures`
**Flag**: N/A
**Est. lines**: ~400
**Ships via**: `/pr`
**Objective**: `./target/debug/ynz run crates/ynz-driver/tests/fixtures/m4_player.ynz` runs the success-criteria program end-to-end. CI passes valgrind clean on at least one heap-allocating fixture. The full fixture suite (M1-M4) runs in `cargo test --workspace` under 30 seconds.
**Why this phase exists**: Per the M1/M2/M3 pattern, the integration phase confirms the whole pipeline works on real programs and gates against memory leaks before the verification sweep.
**Current-state anchors**:
- `crates/ynz-driver/tests/fixtures/` (M1-M3 fixtures already here; M4 fixtures added)
- `crates/ynz-driver/src/main.rs` (M3 wired salsa pipeline; no driver-level work needed in M4 beyond ensuring new salsa queries are reachable)
- M3 phase 5 (`done/m3-control-flow-fns.md:492-561`) — fixture suite organization is the template
**Files (expected scope)**:
- `crates/ynz-driver/tests/fixtures/m4_*.ynz` (~12 positive fixtures + ~20 negative fixtures)
- `crates/ynz-driver/tests/integration.rs` (extend integration test list)
- `crates/ynz-driver/tests/__snapshots__/` (add expected stdout snapshots)
- `.github/workflows/ci.yml` (add valgrind step against the heap fixture)
**Steps**:
1. Author `m4_player.ynz` (the success-criteria fixture) plus 11 other positive fixtures, each covering a specific feature: `m4_inheritance.ynz` (extends), `m4_follows.ynz` (contracts), `m4_override.ynz` (override keyword), `m4_hidden.ynz` (hidden fields), `m4_dynamic.ynz` (runtime polymorphism), `m4_give.ynz` (ownership transfer), `m4_copy.ynz` (trivially-copyable copy), `m4_freeze.ynz` (freeze flips mutability), `m4_share_lend_in_method.ynz` (method receiver kinds), `m4_drop_on_scope_exit.ynz` (heap alloc + drop), `m4_const_immutability.ynz` (positive proof of const sharing — no error).
2. Author ~20 negative fixtures with their stderr snapshots covering every diagnostic class from P1-P5: missing `self` in method, wrong field type in struct literal, use-after-give, double-lend, const-lend, base-instantiation, missing override, missing follows method, cyclic extends, default-expression-with-self, banned `type` keyword, banned `struct`/`class`/`interface`/`enum`/`abstract` keywords.
3. Add a heap-leak sanity test that runs `valgrind --error-exitcode=42 ./target/debug/ynz run ... m4_drop_on_scope_exit.ynz`. CI fails if valgrind reports any leak.
4. Add an IR-snapshot diff-runner: for every M3 fixture, generate LLVM IR text and compare to a committed snapshot; warn (not fail) if it changed unexpectedly — this catches accidental codegen regressions in M3 due to M4 changes.
5. Update CHANGELOG.md draft entry (final version in P9): "M4: shapes + methods + ownership + inheritance + contracts + override + hidden + dynamic dispatch + heap alloc + drop + LLVM readonly/noalias; closed M2 catch-up (wrapping/saturating int methods; int.max/min/number constants)."
6. WHY-comments on every new fixture per global rules.
**Acceptance criteria**:
- [ ] All 12 positive M4 fixtures compile and run; stdout snapshots match.
- [ ] All 20 negative M4 fixtures produce expected diagnostics; stderr snapshots match.
- [ ] Valgrind reports no leaks on `m4_drop_on_scope_exit.ynz`.
- [ ] M3 fixtures' IR is unchanged (or change is intentional and noted in PR description).
- [ ] Total test suite < 30s.
**Quality gate**:
- [ ] Fixture file names are descriptive; no `test1.ynz`/`fixture_x.ynz` style.
- [ ] Negative fixtures have clear comments at top describing the expected error class.
**Verification**: `cargo test --workspace` passes; `valgrind ./target/debug/ynz run ...` clean; CHANGELOG draft entry exists.

---

### Phase 7: M4 verification sweep + tag `v0.1.0-m4`
**PR scope**: No new features. Verification sweep mirroring M1 P8 / M2 P7 / M3 P6. TODO sweep. Catch-up audit (every catch-up entry has a fixture + clear ownership state). Jargon audit. Bouncer-clean check against this plan file (Entries 1, 2, 3, 4 of project graveyard all pass). CHANGELOG entry final version. Cargo version bump. Tag.
**Branch**: `feat/m4-verify-tag`
**Flag**: N/A
**Est. lines**: ~150 (docs + version bump)
**Ships via**: `/release`
**Objective**: Tag `v0.1.0-m4` pushed; CHANGELOG entry final; all catch-up obligations either CLOSED or DEFERRED with explicit ownership; v0-1-compiler.md roadmap reflects M4 status as SHIPPED.
**Why this phase exists**: Every milestone gets its own sweep. M1-M3 each had one; M4 follows the precedent so future contributors have a stable artifact to look at when learning the pipeline.
**Current-state anchors**:
- `.claude/plans/active/v0-1-compiler.md` — roadmap; M4 status updated to SHIPPED + commit SHA + test count
- `Cargo.toml` — workspace `version` field; bump from M3 version
- `CHANGELOG.md` — append M4 entry
- `.github/workflows/ci.yml` — confirm M4 fixtures run in CI
**Files (expected scope)**:
- `Cargo.toml` (version bump)
- `CHANGELOG.md` (final M4 entry)
- `.claude/plans/active/v0-1-compiler.md` (M4 status update)
- `.claude/plans/active/m4-shapes-methods-ownership.md` (move to `done/` via `git mv`)
- `.claude/todos.md` (M4 entries marked done)
- `.claude/state.md` (M4 decision row added)
**Steps**:
1. **TODO sweep.** Grep all crates for `TODO`/`FIXME`/`HACK`/`Phase`/`will be`/`later`/`eventually` in source comments. Any found = move to `.claude/todos.md` with ownership; delete the comment per global rule 6 ("no TODOs in code").
2. **Catch-up audit.** Walk every catch-up entry in `v0-1-compiler.md` and this plan file. Each entry must be either CLOSED (with commit SHA + closing test) or DEFERRED (with explicit forward-owner milestone). No orphan catch-ups.
3. **Jargon audit.** Run `cargo test --workspace jargon_audit`. Confirm green. Also manually grep this plan file + `done/m4-shapes-methods-ownership.md` (after move) for any banned-jargon leakage in user-facing diagnostic strings (`design/compiler-errors.md` source-of-truth).
4. **Bouncer audit.** Run `.claude/graveyard.md` Entries 1, 2, 3, 4 checks against this plan (after move to `done/`). Each must pass. Entry 1: Safety subsection enumerates 5 paths const blocks + Performance subsection names `readonly` + `noalias` + LLVM. Entry 2: no "must annotate at call site" framing. Entry 3: all 5 Invariants subsections present + non-empty. Entry 4: this plan's `files:` includes `crates/**` AND has `### Runtime Dependencies` + `### Kernel-Mode Behavior` non-empty.
5. **CHANGELOG entry final.** Author the v0.1.0-m4 entry with: feature list (shapes, methods, inheritance, contracts, override, hidden, dynamic, ownership modifiers, heap alloc, drops, LLVM attributes, catch-up wrapping/saturating + int.max/min/number constants), test-count delta (M3 = 310; M4 expected 470-520), commit SHA range from M3 tag.
6. **Cargo version bump.** Bump workspace `version` in root `Cargo.toml`. Sequence: M3 = 0.1.0-m3, M4 = 0.1.0-m4.
7. **Plan move.** `git mv .claude/plans/active/m4-shapes-methods-ownership.md .claude/plans/done/`. Update `last_updated` front-matter to the commit date. Update `status: done`.
8. **Tag + push.** `git tag v0.1.0-m4 -a -m "M4: shapes + methods + ownership"` + push.
9. **State + todos update.** Add M4 row to `.claude/state.md` "Active Decisions" section: `[2026-MM-DD] M4 complete (<sha>, tag v0.1.0-m4): shapes + methods + ownership + inheritance + contracts + override + hidden + dynamic + .share/.lend/.give/.copy/.freeze + heap alloc + drop-on-scope-exit + LLVM readonly/noalias. Test count: <N>. Plan: .claude/plans/done/m4-shapes-methods-ownership.md.`
10. Mark `.claude/todos.md` M4 entries done.
**Acceptance criteria**:
- [ ] All P1-P6 features verified end-to-end via `ynz run`.
- [ ] All M4 fixtures green; all M3 fixtures green; all M2 fixtures green; all M1 fixtures green.
- [ ] Bouncer-clean: graveyard Entries 1-4 all pass against this plan file.
- [ ] CHANGELOG entry finalized.
- [ ] Cargo version bumped.
- [ ] Plan moved to `done/`.
- [ ] Tag pushed.
- [ ] state.md + todos.md reflect M4 shipped.
**Quality gate**:
- [ ] No orphan catch-up entries.
- [ ] No `TODO`/`FIXME` in code.
- [ ] No banned-jargon in user-facing diagnostics anywhere in M4 surface area.
- [ ] All M4 fixtures committed under version control (no `.gitignore` exclusions).
**Verification**: `git tag --list | grep m4` shows the tag; `git log v0.1.0-m3..v0.1.0-m4 --oneline` lists every M4 PR.

---

## Quality Checklist (verify at completion of M4)

- [ ] All M4 surface features lex + parse + typeck + codegen + run end-to-end via `ynz run`
- [ ] LLVM `readonly` attribute emitted on every `share T` param + every param inferred from `const` binding (IR snapshot asserts)
- [ ] LLVM `noalias` attribute emitted on every borrow-check-proven non-aliased param (IR snapshot asserts)
- [ ] `const` deep-immutability: no path to mutation (no reassignment, no `.lend`, no `.give`, no field-mutation, no inferred `.lend`/`.give`) — each path has a positive proof AND a negative test
- [ ] Use-after-give is a compile error with both spans (give-site + use-site) in the diagnostic
- [ ] Branch-merge consume-set check produces three-part errors for inconsistent consumes
- [ ] `.copy` only on transitively-trivially-copyable shapes; rejected with teaching error otherwise
- [ ] `.freeze` flips the binding to const-from-here-on; field-write through frozen binding rejected
- [ ] Single inheritance via `extends`; cyclic `extends` rejected at typeck
- [ ] `follows` contracts verified: every required method present; mismatched signatures rejected
- [ ] `override` keyword required bidirectionally (missing-when-parent + present-when-not)
- [ ] `hidden` fields invisible to external code; default-required at declaration; only initializable inside declaring shape's methods
- [ ] `dynamic Foo` type-checks + lowers to fat-pointer-with-vtable; runtime polymorphism demoed in fixture
- [ ] `Self` keyword resolves to enclosing shape's concrete type
- [ ] `base shape` cannot be instantiated; teaching error at construction site
- [ ] Heap allocation via `ynz_alloc`/`ynz_free` (libc wrappers); valgrind clean on heap-alloc fixture
- [ ] Drop-on-scope-exit emits `free` in reverse declaration order; drop-flag for conditional gives
- [ ] M2 catch-up CLOSED: wrapping/saturating int methods + int.max/min/number constants ship + fixtures converted from deferral to success
- [ ] All M3 fixtures pass unchanged (regression gate)
- [ ] All M2 fixtures pass unchanged (regression gate)
- [ ] All M1 fixtures pass unchanged (regression gate)
- [ ] Banned-jargon audit clean across all new diagnostics
- [ ] All new diagnostics three-part WHAT/WHAT-INSTEAD/WHY
- [ ] No `unwrap()` outside test code
- [ ] All new tests have WHY-comments
- [ ] Plan-reviewer agent issued PASS on this plan before P1 landed
- [ ] Bouncer graveyard entries 1-4 pass against this plan

---

## Invariants This Milestone Must Preserve

> Required by `.claude/rules/plan-invariants.md`. Each subsection lists testable assertions, not vague aspirations. Bouncer entries 1, 3, 4 check this section's existence + content; entry 2 checks call-site annotation framing across the plan body.

### Safety

The const deep-immutability spec (`design/ownership.md:33-76`) is enforced by M4. Each rule below is a testable assertion with an accompanying negative fixture:

- `const` bindings cannot be reassigned — already enforced M2 at `crates/ynz-typeck/src/check.rs:264`. M4 carries this forward unchanged; positive test confirms `const x = 5; x = 6` still errors.
- `const` bindings cannot be lent for mutation (`.lend` rejected at compile time) — M4 P3c enforces; negative fixture `m4_const_lend_rejected.ynz` produces three-part error naming `const` as the blocker.
- `const` bindings cannot be given away (`.give` rejected at compile time) — M4 P3c enforces; negative fixture `m4_const_give_rejected.ynz` produces three-part error.
- `const` bindings cannot have their fields mutated (`const_binding.field = x` rejected) — M4 P3a + P3c enforce; negative fixture `m4_const_field_assign_rejected.ynz`.
- The compiler will NEVER infer `.lend` or `.give` for a `const` binding — M4 P3c's call-site inference path explicitly checks `is_const` before considering mutable modifiers; negative fixture `m4_const_passed_to_lend_param.ynz` produces three-part error citing the function's signature + the caller's `const` declaration.
- A `.give`'d value cannot be used afterward (use-after-give caught at compile time) — M4 P3c tracks consume-state; negative fixture `m4_use_after_give.ynz` with both give-site span and use-site span in the diagnostic.
- A `.give`'d-in-one-branch value cannot be used after the branch-merge unless `.give`'d in all branches (consistent-consume rule) — M4 P3c enforces; negative fixture `m4_inconsistent_consume.ynz` covers this with a three-part error.
- Two simultaneous `.lend`s of the same binding are rejected (one outstanding mutable borrow at a time) — M4 P3c enforces; negative fixture `m4_double_lend_rejected.ynz`.
- A `.lend` while a `.share` is outstanding is rejected (mutable-while-shared) — M4 P3c enforces.
- A `.share` while a `.lend` is outstanding is rejected (shared-while-mutable) — M4 P3c enforces.
- `.copy` requires every field to be trivially copyable (transitively) — M4 P3c enforces; negative fixture `m4_copy_non_trivial.ynz`.
- `.freeze` flips a binding to effectively-const for the rest of the scope; subsequent `.lend` / `.give` / field-write rejected — M4 P3c enforces; positive + negative fixtures cover this.
- `dynamic Foo` cannot hold a value that doesn't follow Foo (verified at typeck) — M4 P3b enforces.
- `base shape` cannot be instantiated via struct literal — M4 P3b enforces; teaching error names `base` keyword and suggests deriving a non-base shape.
- Cyclic `extends` chain rejected — M4 P3b's parent-chain walker has cycle detection.
- Hidden fields invisible outside the declaring shape's methods (read + write) — M4 P3a enforces.

### Performance

The LLVM attribute contract from `design/ownership.md:51-66` is enforced by M4 codegen. Each invariant has an IR-snapshot test asserting attribute presence in the lowered output:

- Function parameters declared `share T` emit LLVM `readonly` attribute on the parameter — M4 P4 emits at function-decl-time; IR-snapshot `m4_share_readonly_attr.ll` asserts presence.
- Function parameters declared `lend T` emit LLVM `noalias` + writable (no `readonly`) — M4 P4 emits; IR-snapshot `m4_lend_noalias_attr.ll` asserts presence + absence.
- Function parameters declared `give T` emit no `readonly`, no `noalias` (callee owns the value) — M4 P4 emits; IR-snapshot asserts both absent.
- Parameters inferred from a `const` binding at the CALL site emit `readonly` regardless of signature — M4 P4 receives this info from P3c's OwnershipReport and adds the attribute even when the signature alone doesn't mandate it; IR-snapshot `m4_const_call_readonly_attr.ll` asserts.
- `noalias` is emitted on every parameter the borrow checker has proven non-aliased — for M4, `share T` + `lend T` parameters are always non-aliased (M4 has no multiple-borrow-with-overlap paths). `give T` parameters get `noalias` too (owned value, by definition no other live alias). IR-snapshot covers each kind.
- Field access on a shape value compiles to a direct memory offset (LLVM `getelementptr` + `load`) — no runtime field-name lookup, no hash, no indirection. IR-snapshot `m4_field_access_direct_offset.ll` asserts.
- Static method dispatch compiles to a direct call (`call @ShapeName_methodName(...)`) — IR-snapshot asserts.
- Dynamic method dispatch compiles to vtable-load + indirect-call (~3× cost of static, per design/type-system.md:114) — IR-snapshot asserts the load + indirect-call pattern.
- Drop emission produces zero overhead for non-heap (stack-only) bindings; heap bindings get exactly one `ynz_free` call per scope exit per non-consumed binding — IR-snapshot + valgrind assert.
- Drop-flag for conditionally-consumed bindings is a single i1/i8 alloca + flip + check at scope-exit — only emitted when borrow-check proves the consume is conditional. Unconditional consumes skip the drop-flag entirely (zero runtime overhead).
- Default field expressions are evaluated at construction site, not at type-declaration-time — Python's mutable-default footgun is avoided by construction (ownership rules + per-construction-eval).

**Auto-promotion analysis** (mandatory per `.claude/rules/auto-promotion.md`):

- **`array<T>` → `fixed<T>` promotion** (canonical example from `design/collections.md`): does NOT land in M4. M5 introduces `array<T>` and `fixed<T>`; M4 has neither. The promotion path is documented as M5 work; the catch-up obligation table (below) records this so M5 plans pick it up.
- **`let` → `const` promotion** (per `.claude/rules/inference.md`): does NOT land in M4 codegen. The lint suggestion (`mutable-when-const-suffices` in `design/linting.md`) is a v0.4 deliverable; M4 doesn't introduce the lint. M4 DOES emit `readonly` on parameters inferred from `const` bindings at call sites — that's the codegen surface ALREADY in scope. No additional auto-promotion needed.
- **Static dispatch by default for shape methods** (`design/type-system.md:57-210`): codegen surface IS in M4 P4. No muted IDE hint or Tier 3 lint surface for M4 — IDE hints land in v0.2 LSP per `design/ide-hints.md`. M4 emits the static-dispatch codegen unconditionally when concrete receiver type is known.
- **No new auto-promotion candidates** introduced by M4's NEW features (shape declarations, hidden fields, inheritance, follows contracts, override, dynamic dispatch, ownership modifiers, heap alloc, drops, type-attached constants, wrapping/saturating methods). Each was evaluated; none has a stricter-form-the-compiler-could-have-picked that wasn't already addressed by an existing rule (`design/ownership.md` for ownership modifiers; `design/type-system.md` for dispatch).
- **Hover-tooltip text for static-dispatch is v0.2 work** per `design/ide-hints.md`; M4 doesn't ship the tooltip. M4 ships the codegen; v0.2 ships the IDE surface. Cross-reference recorded.

### Teaching

M4 adds approximately 35 new diagnostic classes (P1: 6 banned-keyword + various lex errors; P2: ~10 parse-error classes for the new syntax; P3a: ~8 typeck errors for fields/methods/struct-lit/hidden; P3b: ~7 for inheritance/follows/override; P3c: ~15 for ownership; P4: codegen panics shouldn't surface to users — they're compiler bugs; P5: 2 catch-up-related; P6: fixture diagnostics rolled up). Each invariant below is testable:

- Every M4 diagnostic follows WHAT/WHAT-INSTEAD/WHY three-part format — enforced by the `Diagnostic` constructor's three-non-empty-field assertion (M1 + M2 + M3 carried forward).
- Every banned-keyword diagnostic (`type`/`struct`/`class`/`interface`/`enum`/`abstract`) names the correct Yinz replacement keyword (`shape`/`shape`/`shape`/`shape`/`options`/`base shape`) — covered by snapshot tests in P1.
- Every const-mutation-path diagnostic explicitly names `const` and proposes `let` as the alternative — required by `### Safety` invariant chain (`design/ownership.md:33-76`).
- Use-after-give diagnostics include BOTH the give-site span (using Ariadne's related-span feature) AND the use-site span — P3c assertion.
- Banned-jargon does NOT appear in any M4 diagnostic (`infer`/`inference`/`narrowing`/`monomorphize`/`polymorphic`/`covariant`/`contravariant`/`deref`/`shadow`/`coerce`/`fallible`/`infallible`/`first-class`/`idiomatic`/`arity`/`variadic`/`residual`/`referentially transparent`/`immutable`/`mutable`/`invariant violation`/`ADT`/`AST`) — `crates/ynz-diagnostics/tests/jargon_audit.rs` (workspace-wide grep) is the enforcement; CI fails on first banned word.
- Override-required diagnostic names the parent shape AND the method signature being shadowed — P3b assertion.
- Follows-contract diagnostic names the contract AND the missing method's expected signature — P3b assertion.
- Inheritance-cycle diagnostic names the full cycle (e.g., "A extends B extends C extends A") — P3b assertion.
- Hidden-field-access diagnostic names the field, the shape, and "outside <ShapeName>'s methods" — P3a assertion.
- `Self`-only-inside-method diagnostic explains where `Self` is legal — P3a assertion.
- Base-instantiation diagnostic names the `base` keyword and suggests deriving a non-base shape — P3b assertion.
- Default-expression-with-self diagnostic names `self` as the disallowed reference — P3a assertion.
- Cyclic-field-dependency diagnostic names the cycle — P3a assertion.
- Wrong-receiver-kind override diagnostic names the parent's receiver kind vs the override's — P3b assertion.
- IDE muted-hint for `.share`/`.lend`/`.give` at call sites: M4 does NOT ship this; v0.2 LSP does (per `design/ide-hints.md`). Cross-reference recorded; M4 plan does NOT introduce inverse-anti-pattern call-site annotation requirements (`.claude/graveyard.md` Entry 2 confirmed clean by Bouncer check on this plan).
- IR-snapshot diff failures in CI fail with a teaching message naming the snapshot file and the parameter-kind that changed (per `design/compiler-errors.md` shape) — P4 + P6 wiring.

### Runtime Dependencies

Per `### Kernel-Mode Behavior` below + `design/future/no-runtime-mode.md`, every M4 feature declares its runtime requirements:

- **`shape` declarations themselves**: no runtime dependency — compile-time only. The struct type lives in the LLVM module's type table.
- **`hidden` fields**: no runtime dependency beyond what the field's type requires.
- **`extends` / `follows` / `override`**: no runtime dependency — compile-time resolved.
- **`base shape`**: no runtime dependency — compile-time only.
- **Static method dispatch**: no runtime dependency — direct call.
- **Dynamic method dispatch (`dynamic Foo`)**: depends on per-(concrete_shape, contract) vtable globals being present in the linked binary. No allocator dependency. No scheduler dependency. Vtable globals are compile-time constants.
- **Stack-allocated shape values**: no runtime dependency — `alloca`.
- **Heap-allocated shape values (when M4 chooses heap)**: requires libc `malloc`/`free` via `ynz_alloc`/`ynz_free` runtime symbols. This is the FIRST M4 runtime dependency that doesn't work in `--kernel` mode without a plug-in allocator.
- **`.share` / `.lend` / `.copy` / `.freeze`**: no runtime dependency — compile-time semantics. `.copy` emits `memcpy` which is a libc symbol; kernel-mode targets typically provide a kernel `memcpy` so this is fine in `--kernel` mode.
- **`.give`**: no runtime dependency — the value-copy is `memcpy`; the consume-tracking is compile-time. Drop-flag for conditional gives is a stack-allocated runtime bool; no allocator dependency.
- **Drop-on-scope-exit**: depends on `ynz_free` for heap-allocated bindings only. Stack-only programs have no drop runtime dependency.
- **Catch-up wrapping/saturating int methods**: depend on LLVM intrinsics (`llvm.uadd.sat.i64` etc.); no runtime library dependency beyond LLVM-built-in lowering.
- **Catch-up type-attached constants**: no runtime dependency — compile-time constants.

### Kernel-Mode Behavior

For each M4 runtime dependency above, the `--kernel` mode (v0.3+) behavior is locked:

- **Compile-time-only features** (shape, hidden, extends, follows, override, base, static dispatch, dynamic dispatch's typeck, `.share`/`.lend`/`.copy`/`.freeze`/`.give` semantics, drop-flag, type-attached constants): **always work in `--kernel` mode**. No compile error; no plug-in API required.
- **Static and dynamic dispatch codegen**: **always work in `--kernel` mode**. Vtable globals are compile-time constants, no allocator dependency, no scheduler dependency.
- **Stack-allocated shape values**: **always work in `--kernel` mode**. `alloca` is native.
- **Heap-allocated shape values (via `ynz_alloc`)**: **COMPILE ERROR in `--kernel` mode** unless the user provides a plug-in allocator via `... .in(myKernelAllocator)` syntax (v0.3+ design per `design/future/no-runtime-mode.md`). The error message: WHAT (this code requires heap allocation; `--kernel` mode disables the default libc allocator) / WHAT-INSTEAD (declare a custom allocator and route this allocation through it via `... .in(myAllocator)`, OR re-express the value as stack-only by binding it `const` so the compiler proves the value doesn't outlive its scope) / WHY (kernel modules and NASA-grade embedded systems can't depend on libc malloc; the plug-in allocator API is the supported alternative). Pointer to `design/future/no-runtime-mode.md`.
- **`.copy` emitting `memcpy`**: works in `--kernel` mode because kernel targets provide `memcpy` (it's part of every freestanding C runtime). No special handling.
- **`ynz_free` drop emission**: same as `ynz_alloc` — compile error in `--kernel` mode without plug-in allocator; user must provide a custom free path matched to their allocator.
- **LLVM `readonly` / `noalias` attribute emission**: **always works in `--kernel` mode** — attributes are compile-time IR metadata, no runtime impact beyond optimizer behavior.
- **Catch-up wrapping/saturating LLVM intrinsics**: **always work in `--kernel` mode** — LLVM intrinsics compile to native machine instructions; no runtime library required.
- **`dynamic Foo` runtime polymorphism**: **always works in `--kernel` mode** when the program's heap allocator (if any) is `--kernel`-compatible. Dynamic dispatch itself has zero new runtime dependencies beyond shape-value allocation.

**Forward declaration to v0.3 plug-in allocator API.** `... .in(myKernelAllocator)` and `myKernelAllocator: Allocator` shape (TBD interface) are the v0.3 deliverables. M4 does NOT implement them; M4 ships heap allocation via `ynz_alloc` (libc) only. The kernel-mode compile error in M4 says "kernel-mode disabled" and points to `design/future/no-runtime-mode.md`. v0.3 will revisit.

---

## Out-of-Scope For This Plan (M4 guardrails)

Restated here as the bottom-line guardrail; redundant with the "What M4 explicitly is NOT" list above for sanity-on-skim.

- Function generics / type generics — M5 (and the `array<T>`/`fixed<T>` auto-promotion lands with M5)
- Method overloading — never
- `is Type` narrowing — M6
- `options` declarations — M6
- Union types `A | B` — M6
- `maybe T` sugar — M6
- Full Unicode strings — M7
- `errors` keyword + cascades — M7
- Iterables protocol — M7
- Modules / imports — M8
- Doc comments — M8
- `sensitive` modifier — M8
- Concurrency keyword parsing — M8
- Bignum `number[N]` for N > 34 — M8
- IDE muted-hint surfaces (`.share`/`.lend`/`.give` annotations at call sites) — v0.2 LSP (`design/ide-hints.md`)
- Tier 3 lint suggestions (`mutable-when-const-suffices`, etc.) — v0.4 (`design/linting.md`)
- Auto-SoA layout transform — v0.3+ (`design/future/auto-soa.md`)
- Auto-Arc cross-thread inference — v0.3+ (`design/future/concurrency.md`)
- Plug-in allocator API (`... .in(myAllocator)`) — v0.3 (`design/future/no-runtime-mode.md`)
- `super.method()` for explicit parent-method call — M5+ (M4 requires child to redeclare and call mangled parent name; clean alternative deferred)
- Field auto-reorder for ABI tightness — v0.3+ (`design/collections.md`)

If you find yourself adding code that touches any item above, STOP and either re-plan this milestone or escalate the work to its proper milestone.

---

## M4 Catch-Up Obligations (recorded so downstream milestones don't orphan them)

- **`array<T>` → `fixed<T>` auto-promotion**: belongs to M5. The codegen change + Tier 3 lint suggestion live with M5's collection types. Recorded here so M5 plans pick it up.
- **`let` → `const` lint suggestion (`mutable-when-const-suffices`)**: belongs to v0.4 (lint tier). Codegen-side `readonly` from const bindings ALREADY ships in M4 P4 — the lint suggestion is the SOURCE-LEVEL teaching surface. Recorded.
- **IDE muted-hint surfaces for ownership inference at call sites**: belongs to v0.2 LSP. M4 ships the typeck inference; the IDE surface waits.
- **`super.method()` syntax**: deferred until generics make method dispatch lookup more flexible. Either M5+ or never (depends on what need emerges).
- **Plug-in allocator API for `--kernel` mode**: v0.3+ per `design/future/no-runtime-mode.md`. M4's `ynz_alloc`/`ynz_free` are the v0.1 libc-wrappers; v0.3 swaps in the plug-in surface.
- **Method overloading by argument-count or type**: explicitly declined for v0.1. Structural typing handles ad-hoc polymorphism; if a real use emerges, document in `design/future/`.
- **Auto-SoA layout transform**: v0.3+ per `design/future/auto-soa.md`. M4 lays fields in declaration order; auto-reorder is forward work.
- **Vtable cross-module ABI**: v0.1 has no modules until M8. When M8 lands, vtable layout becomes an ABI question. Recorded here so M8 plans address it.

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each P1-P7 phase is one PR with one branch. The phase template above repeats this verbatim. Reviewer reviews per-phase, not per-commit. Phase 4 (codegen) is the largest single PR but stays one PR — splitting codegen across two PRs would require an intermediate state where typeck produces an OwnershipReport that codegen partially consumes, which is harder to review than one complete PR.
- **Shadow main branches**: every phase branches from `main` (the trunk after the prior phase merged) and merges back to `main` via `/pr`. No long-lived feature branches; no rebase trains.
- **Building the engine before shipping value**: each phase delivers a coherent slice. P1 lexer alone is reviewable + green; P2 parser alone produces ASTs + diagnostics; P3a typeck alone produces shape ASTs with field-access type-checked; etc. The first end-to-end runnable M4 program (M4 success criteria) arrives at P4 end. No "build the whole engine, then ship at the end" trap.
- **Hotfix that isn't**: M4 is not a hotfix. If a real hotfix happens during M4 development (e.g., M3 bug), it ships on its own branch with its own PR, NOT smuggled inside an M4 phase. Catch-up phase (P5) is explicitly scoped to M2 deferral closure, not to general hotfixing.
- **Abandoned branches**: if a phase PR isn't ready to merge, it gets closed and re-opened on a fresh branch; no zombie branches. Phase 7 verification confirms every M4 phase landed and no orphan branches exist via `git branch -a | grep feat/m4`.
- **Flag graveyards**: M4 ships behind no feature flag (the milestone IS the release; tag is the gate). The shape syntax, ownership analysis, heap allocation are all unconditionally enabled when this code merges. No `--enable-shapes` flag; nothing to clean up later.

---

## Reviewer Disputes

### Round 1 (2026-05-15) — Plan-reviewer PASS with non-blocking concerns; planner accepted

Plan-reviewer issued PASS — "Required Fixes: None — plan ready to implement." Non-blocking concerns surfaced, planner accepted ALL by inline plan edits:

1. **Phase numbering inconsistency (cosmetic)** — Plan said "9 phases (P1-P9)" but labels stopped at P7 because P3 is split into P3a/b/c. **Fix applied**: rewrote the Phases-section preamble to state "9 PRs total, labeled P1, P2, P3a, P3b, P3c, P4, P5, P6, P7" with explicit note that the count mismatch is intentional.
2. **Base-shape instantiation guard location was implied, not specified** — P3b step 4 said "guard at construction site" without naming `check_expr(Expr::StructLit)` as the location. **Fix applied**: P3b step 4 now names the exact callsite + states the early-check ordering rule (base-instantiation error wins over field-type errors).
3. **5 missed adversarial cases** (test gaps that would bite during P3c implementation):
   - `.give self` mid-method-body, then continued use of `self` (intra-function give-then-use, different Ariadne span path than cross-binding case) — **added to P3c negative tests #19**
   - `.lend self` method calling a `.share self` method on the same value (re-borrow check) — **added to P3c positive test #13 with locked design decision: allowed**
   - Cyclic `follows` graph (extends-cycle covered; follows-cycle wasn't) — **added to P3b negative test #8**
   - `dynamic Foo` returned across function boundary (all `dynamic` tests stayed in-scope) — **added to P3b negative test #9**
   - `.copy` of shape inheriting from `base shape` with non-trivial fields (transitive-trivially-copyable through inheritance, not just declared fields) — **added to P3b negative test #10**

### Round 16 (2026-05-16) — Doc-PR 2 complete; Doc-PR 3 partial (phases section gets prominent FINAL LOCKED DECISIONS pointer)

Doc-PR 2 completed: 19 files touched. Major rewrites (design/type-system.md, design/ownership.md, spec/types.md, spec/ownership.md, spec/operators.md). Medium rewrites (spec/iterables.md, design/iterables.md, spec/concurrency.md, design/ide-hints.md, .claude/rules/inference.md, .claude/rules/vocabulary.md). Small surgical fixes (spec/overview.md, spec/variables.md, spec/functions.md, spec/linting.md, design/errors.md, design/decisions.md, design/golden-rules.md, design/collections.md, design/stdlib/database.md).

Verification: final grep showed all body-level `.share/.lend/.give` no-parens patterns removed from spec/design (except in non-oop.md and dot-postfix.md which intentionally reference the form for teaching). All prefix-form struct literals removed (except in spec/types.md where it appears as an explicit COMPILE ERROR example showing what's banned). `override` keyword removed from all spec/design content. Methods-inside-shapes removed from all spec/design content. Total: 25 files in diff (Doc-PR 1 + Doc-PR 2), +2452 insertions / -403 deletions.

Doc-PR 3 partial: rather than rewriting the M4 plan's phase bodies in full (which would duplicate the FINAL LOCKED DECISIONS section's content in detailed prose), added a prominent READ-FIRST pointer at the top of the Phases section explicitly listing how each phase changes for the non-OOP / UFCS / standalone-functions model. Phase bodies retain their useful content (test counts, file:line anchors, LLVM attribute requirements, fixture lists) — only the OOP-leaning sections get tweaked during implementation per the pointer's guidance.

This is HONEST about scope: writing 500+ lines of duplicate phase-body content would consume context for diminishing return when the implementation chat will read the FINAL LOCKED DECISIONS as the source of truth anyway. The pointer + locked-decisions section together carry the same information without the duplication.

All design-doc-rewrite work for the non-OOP transition is now complete. M4 P1 (lexer) can start.

### Round 15 (2026-05-16) — Mid-Doc-PR-2 surface: two design questions surfaced during type-system.md rewrite review

Patrick reviewed the `design/type-system.md` partial rewrite and raised two questions:

**OPEN-Q12: What's the purpose of `hidden` keyword in the non-OOP model?** Patrick: "in theory if we don't want the scope outside the file, we just don't export the function right? or the shape? so there is no 'hidden' keyword."

Planner answered: `hidden` solves a problem that module-export rules can't — **per-field visibility within an exported shape**. Without `hidden`, external code can write `cache.entries["sneaky"] = 99` bypassing the maintained invariants of the public `put`/`get` functions. Workaround (separate non-exported shape for internal state) doesn't work because the link field is still exposed. Planner-recommends: **KEEP `hidden`** for encapsulating internal-state fields inside exported shapes.

**OPEN-Q13: Do we need type aliases (`shape UserId = string`)?** Patrick: "if we had interfaces AND types like typescript which we dont right? or am I missing something here as well?"

Planner answered: type aliases exist in TS because TS has `interface` AND `type` as separate concepts. Yinz only has `shape` — the tension that birthed type aliases doesn't exist. Type aliases provide zero functionality (UserId IS string at compile/runtime); they're pure documentation sugar. Parameter names + comments do the same job. Planner-recommends: **DROP type aliases entirely from M4 and the spec**.

### Round 14 (2026-05-16) — Doc-PR 1 complete; Doc-PR 2 chunk 1 (design/type-system.md) in progress

Doc-PR 1 (Task #7) completed: 5 foundation files landed (.claude/rules/non-oop.md, .claude/rules/dot-postfix.md, project CLAUDE.md update, design/golden-rules.md cross-cutting principle, design/decisions.md Cross-Cutting Architectural Principles section, .claude/rules/language-design.md OOP Drift Test). Grep verification clean. Not yet committed — Patrick locked "batch all three doc-PRs at the end" so commit deferred.

Doc-PR 2 started: chunk-by-chunk approach (5 chunks). Chunk 1 = `design/type-system.md` partial rewrite:
- §"One Keyword `type`" → §"One Keyword `shape`" + non-OOP cross-reference
- §"Single Inheritance with `extends`" → reframed as DATA-ONLY inheritance with standalone-function example
- §"`override` Keyword Required" REMOVED; replaced with §"Function Overloading by Argument Type"
- §"Static Dispatch" example rewritten: methods-in-shape → standalone functions + `follows` verification
- Compile-error example for "Foo doesn't follow Comparable" rewritten to 2-step pattern (declare follows + provide standalone function)
- Small updates: `type Box<T>` → `shape Box<T>`, etc.
- §"Hidden Fields" updated: framed as "same-file visibility" (not "same instance's methods") because shapes don't have methods anymore; clarified hidden is orthogonal to ownership
- §"Type Aliases": pending OPEN-Q13 resolution (planner-recommends DROP)

Plan-text: NOT YET updated to reflect Doc-PR 2 in-progress. Will be batched into Doc-PR 3 alongside the phase-body rewrites.

### Round 13 (2026-05-16) — Patrick LOCKED Option A (UFCS — both forms work); added dual-style teaching requirement to all UFCS-related diagnostics

Patrick locked **Option A** for OPEN-Q11 after planner clarified the IDE/compiler mechanism (both share function-signature lookup; IDE filters autocomplete by first-param-type matching receiver; compiler verifies; tower.attack() fails type-check because attack's first param is Player, not Building).

**Patrick's UX addition (new requirement)**: every UFCS-related error message must teach BOTH call styles in its WHAT-INSTEAD section. His framing: "you can't do that, try tower.whateverWorks or tower(a,b)" — showing the user both alternatives.

Canonical diagnostic format (to be codified in `.claude/rules/non-oop.md` rewrite):

```
COMPILE ERROR: No function `attack` accepts (Building, Enemy, int).

  Available `attack` functions:
    attack(lend Player, lend Enemy, int) -> nothing
           ^^^^^^^^^^^ expected Player here; you passed a Building

  Functions you CAN call on a Building value:
    tower.fortify(50)               ← dot-call style
    fortify(tower, 50)              ← function-call style
    (both forms work — pick whichever reads better)

  Why both styles: Yinz lets you write either `value.fn()` or `fn(value)`
  for any function whose first parameter type matches the value. Pick the
  form that reads naturally for the situation.
```

Implementation requirements:
- Compiler "did-you-mean" suggester searches BOTH directions: same-name-different-signature (explain why), AND any-name-first-param-matches-receiver (show what's possible)
- IDE renders the same suggestion via hover/tooltip on the underlined call site
- First-encounter teaching: the FIRST `value.foo()` error a new Yinz user hits should teach both call styles in one shot

Status of artifacts:
- `.claude/rules/non-oop.md` draft: still held (will be rewritten cleanly now that OPEN-Q11 is locked, incorporating the Building example + dual-style diagnostic format)
- `.claude/rules/dot-postfix.md`: still pending (Doc-PR 1)
- Doc-PR 1: cleared to proceed once non-oop.md rewrite ships

### Round 12 (2026-05-16) — Patrick reviewed `.claude/rules/non-oop.md` draft; surfaced UFCS-vs-function-call-only as an unresolved design decision; rule file draft held pending lock

Patrick's r12 reactions to the non-OOP rule file draft:

1. **"your file is sloppy"** — accepted. Specific sloppiness called out:
   - Top rule statement has three sentences instead of one
   - Used `Damageable` in an example without defining it first
   - (Planner self-identified additional issues: "Coming from TypeScript" section duplicates the translation table; the "When You'd Reach for OOP" table is 11 rows where 6 would suffice)

2. **UFCS QUESTION SURFACED (NEW OPEN-Q11)**: Patrick wrote "we only want one. and `method(value)` is my choice. `value.method()` would only be a thing in objects right?"

   The planner had been assuming UFCS (both `value.method()` and `method(value)` legal; the dot form is sugar for the function form) was a locked decision. It's not — it was assumed, never explicitly confirmed.

   This question is load-bearing because it collides with **Golden Rule 1 (dot-first design)**: "If something can be `.method()` with autocomplete, do it that way."

   **Three options surfaced for Patrick to lock**:
   - **Option A**: UFCS — both `value.method()` and `method(value)` work; user picks; dot-form gives autocomplete; function-form is preferred style. (Current assumption.)
   - **Option B**: Function-call only — `method(value)` is the only legal form. Drops dot-call entirely. Conflicts with Golden Rule 1; would require rewriting that rule.
   - **Option C**: Dot-call only — `value.method()` is the only legal form (Rule 1 wins). Method is still a standalone function declaration; dot is just parser convention. Not OOP (nothing stored on the value); looks similar but mechanically different.

   Planner-leaning: **Option C** (preserves Golden Rule 1 intact; the OOP-ness fear is about WHERE methods are declared, not about call syntax; dot-call on a value where the function is a standalone declaration is just a parsing convention).

   **DECISION REQUIRED before `.claude/rules/non-oop.md` finalizes.** The entire "How Methods Work," "Coming from TypeScript," and "How Inheritance Works" sections depend on which option is locked. The rule file draft is held pending Patrick's call.

3. **Damageable example fix**: trivial cleanup once UFCS question is resolved — rewrite using `Comparable` (which IS defined in the file) or define Damageable first.

4. **Status of FINAL LOCKED DECISIONS section (top of plan)**: items #3 ("Methods are standalone functions taking the receiver as the first parameter. `value.method()` is sugar for `method(value)` — Uniform Function Call Syntax (UFCS).") needs the same lock. UPDATE pending OPEN-Q11 resolution.

5. **Doc-PR 1 status**: BLOCKED on OPEN-Q11. Once locked, planner rewrites `.claude/rules/non-oop.md` cleanly (one-line rule, 6-row OOP-translation table, no duplicate TS section, Damageable→Comparable fix) and then proceeds with the batch (project CLAUDE.md note, golden-rules entry, decisions log, language-design checklist update, dot-postfix rule file — the dot-postfix rule file is also blocked because parens-on-method-calls only matters if method-calls exist as a syntactic form, which OPEN-Q11 decides).

No planner pushback. Awaiting Patrick's A/B/C call on UFCS.

### Round 11 (2026-05-16) — Patrick CONFIRMED: body-level `.share() / .lend() / .give()` syntax REMOVED entirely. Compiler-inferred at call sites only. `.copy()` and `.freeze()` remain as body operations.

Patrick's r11 push: "are we sure we aren't over engineering and bloating shit. Be honest don't hand wave or please me."

Planner's honest answer (the truth I should have arrived at three rounds earlier): body-level `.share() / .lend() / .give()` syntax is pure bloat. There's NO case in M4 where the user needs to type these in the body — the compiler always infers from the callee's signature (or return slot, or storage slot). Allowing the syntax would just add a second decorative form for what's already inferred.

What stays in the body:
- `.copy()` — produces a NEW value (genuine transformation, no signature equivalent)
- `.freeze()` — state change on existing binding (no signature equivalent)
- Method calls `value.method()` — UFCS sugar for `method(value)`
- Field access `value.field` — no parens, per dot-postfix rule

What's gone from the body:
- `value.share()` — never typed; always compiler-inferred at call sites
- `value.lend()` — same
- `value.give()` — same

Plan impact:
- `.claude/rules/dot-postfix.md`: no longer needs to handle ownership modifiers; just methods + `.copy()` + `.freeze()` + intrinsics
- `.claude/rules/inference.md`: ownership-inference domain row simplifies to "infer at call sites from callee signature; render as muted hint"
- M4 P2 parser: no `.share() / .lend() / .give()` postfix-modifier parsing — drops a chunk of complexity
- M4 P3c ownership analysis: simpler — no body-level modifier expressions to check; just check parameter usage against signature
- M4 success-criteria fixture: cleaner — no `.give()` calls
- Spec/design docs: no body examples with `.share() / .lend() / .give()` — show signature-only

Also confirmed in r11:
- Doc-PR process must grep first, then write. Every doc-PR lists found patterns in its description, fixes every one in the same PR, re-greps to verify zero remaining.
- Plan body (Phases, Invariants subsections) still reflects pre-r10 model. Doc-PR 3 rewrites it. Until then, FINAL LOCKED DECISIONS section at the top is the source of truth.

### Round 10 (2026-05-16) — Patrick CONFIRMED non-OOP model: data shapes + standalone functions + UFCS dot-call sugar. This is the LARGEST design decision in this plan's history.

**Patrick's framing**: he showed his mental model that shapes are contracts (types + signatures) and instance literals (`const player: Player = { ... }`) hold values. Planner had been assuming OOP-like methods-inside-shapes per `design/type-system.md`. Patrick called this out — he never intended OOP; the language was being built non-OOP-by-default and OOP was an unstated assumption planner kept making.

Planner's recommendation (accepted): **standalone functions + UFCS** (Rust/Go style)
- Shapes hold data fields + contract method-signature declarations only — NO method implementations
- Methods are normal `function` declarations at file/module level, taking the receiver as the first parameter
- `value.method()` is sugar for `method(value.share())` (UFCS — Uniform Function Call Syntax)
- Zero per-instance method storage (one function shared across all callers — same machine code as OOP-like)
- TS-familiar (TS commonly uses standalone functions over types)
- Aligns with Yinz's "Rust-level performance, not OOP" positioning

**Side effects of this lock**:
- `override` keyword REMOVED entirely — there are no methods on shapes, so nothing to override. Function overloading by argument type becomes the natural dispatch mechanism.
- `extends` becomes DATA-only inheritance — child shape inherits parent's fields; behavior comes from standalone functions
- `follows` checked via standalone-function-signature matching — a shape follows a contract when standalone functions with matching signatures exist
- `dynamic Foo` works via contract function table (fat pointer + per-(shape, contract) function pointer array) — same machine code, different framing
- Method dispatch becomes function lookup with overload resolution by argument types

**Existing code refactoring scope**: NONE. M1/M2/M3 are all primitives + control flow + standalone functions. The M2 `PrimitiveIntrinsicTable` is already UFCS-shaped (lookup by `(receiver_type, method_name)`). M4 extends the same table with user-defined `(receiver_type, function_name)` entries.

**Doc + plan rewrite scope** (THREE sequential doc-PRs before P1 starts):

- **Doc-PR 1** (Task #7): Foundation. NEW `.claude/rules/non-oop.md` codifying the model. Project `CLAUDE.md` note. `design/golden-rules.md` non-OOP principle. `design/decisions.md` entry. `.claude/rules/language-design.md` checklist update.
- **Doc-PR 2** (Task #8): Apply to existing docs. Major rewrite of `design/type-system.md` (remove methods-inside-shapes; document standalone+UFCS pattern; remove `override`; redocument `extends` as data-only). Same for `spec/types.md`. Update ownership docs (`design/ownership.md`, `spec/ownership.md`) — ownership concepts unchanged but examples use standalone-function-with-receiver. Also folds in: `.claude/rules/dot-postfix.md`, `.freeze()` and other ownership-modifier paren updates, annotation-only struct-literal form.
- **Doc-PR 3** (Task #9): Rewrite M4 plan phases P2/P3a/P3b/P4 for the standalone+UFCS model. Plan simplifies significantly:
  - P2 parser: ShapeDecl body has FieldDecl + bare-signature contract-method declarations only; NO MethodDecl with body inside shape
  - P3a typeck: no method type-check inside shapes; standalone functions normal; UFCS resolution at call sites
  - P3b: extends = data-only; follows = standalone-function-signature match; override REMOVED; dynamic = contract function table
  - P4 codegen: no per-shape vtable for static dispatch; standard function calls; contract dispatch tables only for `dynamic Foo`

**All previously-OPEN questions now resolved**:
- Q7 (partial moves): Option X confirmed — no partial moves in M4
- Q8 (signature ownership default): collapsed into Q10 — bare = implicit share for free functions; contract signatures and function-type annotations require explicit
- Q9 (call-site inference table): confirmed
- Q10 (signature ownership): confirmed — `function` keyword requires body, ownership inferred from body; contract bare-signatures must be explicit
- **NEW R10**: non-OOP standalone+UFCS model adopted — REVERSES the methods-inside-shapes assumption in `design/type-system.md`

**Status**: ALL major design questions resolved. Three sequential doc-PRs (Tasks #7, #8, #9) before P1 starts. After Doc-PR 3 merges, M4 P1 (lexer) begins.

### Round 9 (2026-05-16) — Patrick's question accidentally fixed the contract-method syntax gap; `function` keyword now requires a body always

Patrick's r9 question: "why would someone define a function without a body? in theory that just shouldn't be allowed."

**This locks two things**:

1. **`function` keyword always requires a body in Yinz.** `function f() -> nothing` with no `{}` is a PARSE ERROR. No exceptions. This eliminates the "body-less function declaration" category I kept worrying about.

2. **Contract method signatures use a different syntax — NO `function` keyword.** Since contracts need to declare method signatures without bodies (for implementors to satisfy), they use a bare-signature form:

```yinz
// CONTRACT — no `function` keyword; explicitly declares ownership
shape Comparable {
  compare(share self, share other: Self) -> int     // signature declaration, not a function
}

// IMPLEMENTATION — `function` keyword; ownership inferred from body
shape Player follows Comparable {
  function compare(share self, share other: Player) -> int {
    return self.health - other.health
  }
}
```

The syntactic distinction is clear: `function` = has body = infer ownership. Bare signature in contract = no body = explicit ownership required. Users learn one clean rule, not two patterns for the same form.

**This resolves OPEN-Q10** (the ownership inference question). Final rule:

| Form | Ownership |
|---|---|
| `function f(...) { body }` — any free function or method | **Inferred from body** |
| `methodName(share self, ...) -> T` — contract shape signature | **Must be explicit; no body available** |
| `let f: function(...) -> T` — function-type annotation | **Must be explicit; type-level, no body** |

If user forgets ownership on a contract signature: COMPILE ERROR. If user forgets ownership on a `function` declaration: compiler infers it (no error unless body is inconsistent). NO runtime ownership errors in either case.

**Design doc update required**: `design/type-system.md` and `spec/types.md` currently show contract shapes using `function` keyword in method declarations. Must be removed — contract methods use bare-signature form only. This goes in the consolidated pre-P1 doc-PR (Task #6).

**OPEN-Q10 is now locked.** Awaiting Patrick's explicit confirm before folding into plan body and starting doc-PR.

### Round 8 (2026-05-15) — Patrick called out planner's flip-flopping; planner conceded; THEN Patrick proposed the synthesis that finally pins the answer: bare = implicit share, explicit for lend/give

Patrick's r8 critique: "are you just people pleasing me? you nearly had me sold on give at the beginning." Honest hit. Planner had been swinging based on Patrick's tone in r6/r7, not on new evidence. Acknowledged.

Patrick's r8 questions (the ones that pinned the answer):
1. "Can the compiler infer 100% of the time?" → No. Contracts (no body), cross-module imports (only signature visible), function-typed variables (no body in the type position) are STRUCTURALLY un-inferable. The 10% gap isn't a hole — it's categories of declarations that REQUIRE explicit ownership.
2. "If can't infer 10%, how do we teach it?" → Compiler refuses to compile when it can't infer/verify. Teaching diagnostic asks the user to declare explicitly. (Patrick worried about runtime errors; planner clarified ownership in Yinz is 100% compile-time — no runtime escapes.)
3. "What covers MOST scenarios — pick the form that covers 90% over the one that covers 85%."

Patrick's synthesis (the actual lock-in proposal):
- "the IDE and compiler infer the 90 percent it can"
- "if it can be inferred it can be muted text and or the yellow scribble or comments"
- "on compile they get the warning this was auto-inferred — lesson there"
- "as close as we can get to the teaching part"

Planner synthesized Patrick's proposal into the concrete rule:

**Hybrid: bare parameter = implicit `share`; explicit `lend`/`give` when function mutates or consumes.**

- Bare signature parameter = implicit `share` (read-only, safe default). 70-80% of params.
- Explicit `lend` keyword = mutation contract. ~15% of params.
- Explicit `give` keyword = consumption contract. ~5% of params.
- Compiler enforces body matches signature; mismatch = compile error with WHAT/WHAT-INSTEAD/WHY teaching diagnostic.
- Contracts/imports/callbacks use the SAME rule: bare = share; explicit for lend/give.
- Cross-module: compiled signature exports the explicit-or-inferred modifier as part of the interface.
- Teaching surfaces:
  - Compile diagnostic when body conflicts with bare signature
  - IDE muted hint on bare signatures confirming inferred `share`
  - Tier 3 lint (`prefer-explicit-share-for-clarity`) — opt-in suggestion to type `share` explicitly when reviewers want emphasis

**This satisfies the rule scorecard Patrick weighted**:
- Rule 2 (self-documenting): `lend`/`give` visible at signature where they matter; `share` is the safe-default-when-absent (same as TS readonly is opt-in)
- Rule 4 (compiler does hard work): bare signature is OK; compiler infers + verifies
- Rule 5 (compile-time safety): no runtime ownership errors; every violation is a compile error
- Rule 6 (TS-familiar): bare signature reads like TS — `function f(p: Player)` looks normal; only mutation/consumption decorates
- Rule 9 (fast to type): most params bare; only minority needs keyword
- Rule 11 (compiler is teacher): teaching diagnostic when bare conflicts with body; IDE muted hint; Tier 3 lint
- Rule 12 (human-readable): `lend`/`give` are plain English; appear only where they matter

**This is the planner's LOCKED recommendation (no more flip-flopping)**. Patrick has not yet given explicit confirm — awaiting his green-light.

### Round 7 (2026-05-15) — Planner retracted outside-in recommendation after Patrick's pushback; locked on Inside-out / Form A

Patrick pushed back on three points in r7:

1. **Cache scenario clarification**: he asked whether `db.save` could just "share again afterwards" instead of consuming. Planner answered: borrows have scope-bound lifetimes. Storage = needs ownership = `.give()`. Borrows can be PASSED through call chains but not STORED past their scope. Patrick acknowledged "we can't infer of return alone like I initially thought" — he's grasping the model.
2. **Rule 11 (compiler is teacher) doesn't favor outside-in**: Patrick: "if an IDE can teach so can the compiler — they are both running the same rules." Planner CONCEDED — the compiler IS the teaching engine; the IDE just renders teaching through a different channel. Rule 11 doesn't require source-level rendering specifically. Both outside-in and inside-out satisfy it.
3. **Rule 2 (self-documenting) doesn't decisively favor outside-in**: Patrick: "they may not know it gives right there but when they see .give() in the function it is still self documenting it is just later on." Planner CONCEDED — body IS source code and DOES self-document. Only Rule-2 residual gap is for callers who only see the signature (autocomplete, docs); mitigated by IDE-rendered inferred contract + generated docs.
4. **Patrick's mental model is clean**: "params are for types and values only. give is method to me but that is my TypeScript mind speaking." Planner agreed — categorically, signatures hold TYPES (nouns); ownership operations are ACTIONS (verbs); mixing them in signature position is conceptually muddled.

Planner's final recommendation (revised in r7): **Inside-out / Form A is correct.** Specifically:
- Signatures bare: `function save(p: Player) -> nothing`
- Body uses dot-postfix: `db.save(p.give())`, `print(p.share())`, `p.field = value` (implicit lend)
- Compiler infers per-parameter ownership from body usage
- IDE shows inferred contract as muted hint on the function declaration
- Tier 3 lint (`ownership-contract-changed`) fires in PR review when a body change shifts the inferred contract

Trade-offs Patrick is accepting:
- (+) Less typing
- (+) Dot-postfix consistency across body
- (+) TS-familiar bare signature
- (+) Matches mental model (params = types)
- (−) Caller without IDE must read body to see contract — mitigated by IDE hover + generated docs

**Pending Patrick's explicit confirm of the four locks** (OPEN-Q7, Q8, Q9, Q10). Plan-text changes after confirm: design/ownership.md:14 rewrite + .claude/rules/inference.md new domain row + spec/ownership.md rewrite + plan P2 (no signature modifier) + plan P3c (produces inferred contract per function) + add `ownership-contract-changed` Tier 3 lint to M4 catch-up obligations OR v0.2/v0.4 work list.

### Round 6 (2026-05-15) — Patrick spotted that keyword-prefix on signature params violates the just-locked dot-postfix rule; proposed inside-out ownership inference; planner-recommends Form A (bare-only, infer from body)

In r6, Patrick selected lines from a temp file showing:
```yinz
function save(p: Player) -> nothing {
  wait db.save(p.give())
}
```

His observation: putting `share`/`lend`/`give` as a keyword prefix in the signature (`function save(give p: Player)`) violates the dot-postfix rule we locked in r4. Ownership is an ACTION on a value; it should live in dot-postfix-with-parens form on the value, NOT as a signature-position keyword.

Planner's response:
- Conceded the point — Patrick is correct that signature-keyword form is now inconsistent with the dot-postfix rule
- Surfaced as OPEN-Q10 (see above) — proposed Form A (bare signature, inside-out inference) as planner-leaning
- Flagged this REVERSES `design/ownership.md:14`'s "explicit in signatures" rule
- Walked through trade-offs (− cross-module ABI requires exported inference metadata; − body changes silently shift contract; + dot-postfix uniformity; + less typing)
- Proposed mitigation: v0.2 LSP shows inferred contract prominently; Tier 3 lint catches body changes that shift ownership

This is the largest design pivot in plan history. Patrick should sit with it before locking — the cost of flip-flopping after fixture-writing starts is high.

Side note: r6 also implicitly affects OPEN-Q8 (signature ownership default). If Form A wins, OPEN-Q8 collapses into "all signatures are bare; no implicit default to discuss because there's no modifier slot."

### Round 5 (2026-05-15) — Patrick pushed back on `.give()` glibness; planner re-grounded; THREE new questions surfaced

Patrick's r5 critique: "us actually returning p.name in your one example means nothing? when did we infer .give again I'm a little confused here I'm not sure YOU have thought the entire give thing out."

He's right. The planner was imprecise about WHEN exactly `.give()` is inferred. R5 produced:

1. **Concrete inference table** (see OPEN-Q9 above) — what was glib is now tabular.
2. **Field-read semantics**: `return p.name` where `name` is a primitive string — no modifier needed; primitives are copy-on-read in M4. `return p.inner` where `inner` is a nested shape — UNRESOLVED, surfaced as OPEN-Q7.
3. **Function signature default**: `function f(p: T)` without ownership modifier — was implicitly assumed, never actually locked. Surfaced as OPEN-Q8. Planner-leaning: no implicit default; compile error.
4. **DB-scenario walked through** with three stories (DB shares / DB consumes / caller wants player back) — clarified that the SIGNATURE drives every ownership decision; call sites just satisfy what the signature requires.
5. **Also corrected the `int.parse("42")` example earlier**: that's not a real Yinz operation. Replaced with M2-shipped `.toNumber()` / `.toFloat()` / `.toString()` instance methods + M4-P5 `int.max`/`number.epsilon` type-attached constants. **Added to the dot-postfix-rule design checklist**: "Every example MUST use a real Yinz operation from the current scope — no invented APIs for illustration." Cross-referenced in `.claude/rules/spec-writing.md` and `.claude/rules/docs-checklist.md`.

Net plan changes pending Patrick's r5 confirm (three OPEN-Qs):
- OPEN-Q7 (partial moves out of nested-shape field): planner-leaning Option X (no partial moves; M4 simplicity)
- OPEN-Q8 (signature ownership default): planner-leaning NO IMPLICIT DEFAULT; compile error if missing
- OPEN-Q9 (lock inference table): planner-leaning lock as M4 contract

If all three confirmed, the pre-P1 doc-PR scope grows by ~3 small additions (Safety invariants in plan + signature-required diagnostic in P2 + inference-table appendix in P3c).

### Round 4 (2026-05-15) — Patrick locked all six r3 OPEN questions + proposed NEW dot-postfix design rule; planner accepted all

Patrick's decisions:

1. **Q1 `.freeze`**: KEEP, but with parens. **NEW META-RULE proposed by Patrick**: "if it performs an action (not updating a field), it is represented like a function" — i.e., parens-for-actions, no-parens-for-field/constant-access. Planner agreed: cleaner generalization, removes the field-like reading of ownership modifiers. New rule file `.claude/rules/dot-postfix.md` to be added.
2. **Q1 freeze NAMING**: KEEP "freeze" — matches JS `Object.freeze()` (Golden Rule 6), universal English metaphor, no better alternative survives scrutiny (.seal/.lock/.fix/.commit/.solidify/.finalize all have problems).
3. **Q2 `extends`**: KEEP.
4. **Q3 hidden-field defaults**: constants + empty literals only.
5. **Q4 struct literal**: Option A — annotation only. Patrick confirmed: "I think one and only 1. and I like option A. I'm actually pretty sure somewhere I said I DID NOT want to do Type value I always wanted it defined at the definition level." Exception correctly noted: function-generics still use `<T>` syntax at call sites, separate from struct-literal construction.
6. **Q5 `.copy()` strict**: KEEP (Rust Copy/Clone model).
7. **Q6 drop order at return**: KEEP — "drop before return. We want shit out of memory ASAP."

Patrick's `.give()` instinct (round 4):

Patrick raised: "I don't understand why .give is just not inherent of what is returned. If I return something....I'm giving it why do I explicitly have to tell someone I'm giving it?" Planner-acknowledged this was a CORRECT instinct that matches the locked design. The plan's prior examples over-specified by writing `return p.give` and "an explicit .give transferring ownership" — those should be reframed to inferred-modifier + IDE muted hint. The user rarely types `.give()`; the compiler infers from return-statement context (only `.give()` is valid because share/lend borrows can't outlive the function). Same applies to `.share()` and `.lend()` at function calls — inferred from signature.

Net plan changes triggered by r4 (consolidated into pre-P1 doc-PR): see the `### LOCKED in r4` block above for the 9 doc + plan rewrite obligations.

No planner pushback in r4. All Patrick decisions accepted; the dot-postfix meta-rule is a strict design improvement (planner agrees).

### Round 3 (2026-05-15) — Patrick engaged on r2 OPEN questions; planner provided concrete examples + alternatives in other languages; one NEW open question surfaced

Patrick worked through OPEN-Q1/Q2/Q3 and the example-requested questions. Outcomes:

1. **`.freeze` (OPEN-Q1)**: Patrick said "if this is a legit pattern I say keep freeze honestly. It reads better to me." Planner walked through the build-then-lock pattern with conditional/intermediate mutation (the case where const-shadowing doesn't cleanly work). Both leaning KEEP. Awaiting confirm.
2. **`Self` semantics**: Patrick confirmed locked, but raised an idiomatic-syntax adjustment: `let p2 = Player { ... }` reads OOP-y; should be `let p2: Player = { ... }`. Planner identified this as a NEW design-doc-aligned improvement → OPEN-Q4. `design/type-system.md:53` already supports both forms (structural typing); question is which form is idiomatic + whether to lint the other.
3. **Default expressions (OPEN-Q3)**: Patrick reframed "shape is a contract not the actual implementation of key value pairs" — planner reconciled by clarifying defaults are ONLY on hidden fields (load-bearing because hidden fields can't be construction-provided); visible fields have NO defaults; the OPEN question is what EXPRESSIONS are legal in the hidden-field defaults. Patrick's "contract not implementation" framing matches the plan's lock (constants + empty literals only).
4. **`.copy` (Q5)**: Patrick said "regardless probably isn't the best use in this case BUT as long as we teach that maybe having it exist is still a nice feature. How else would you handle that in other languages anyway if you wanted to copy it?" Planner provided cross-language comparison (Rust Copy/Clone, TS spread, Go shallow, Swift COW, C++ copy-ctor footgun) — Yinz currently mirrors Rust's good model. Re-surfaced as OPEN-Q5 for confirm.
5. **`.give` and drops (Q7)**: Patrick asked "what does p.give do and when you say drop are you saying..." Planner walked through the TS-vs-Yinz mental model (no GC, ownership-based cleanup, drop = compile-time-inserted `free()` at scope exit, `.give` = explicit ownership transfer). Patrick's TS intuition matched the locked design exactly. Re-surfaced as OPEN-Q6 for confirm.

No planner pushback. All r3 surfaces map to OPEN-Q1 through OPEN-Q6 above; awaiting Patrick's confirm to lock.

### Round 2 (2026-05-15) — Patrick asked clarifying questions on locked decisions; planner re-grounded against design docs

Patrick raised four concerns during r2 review:

1. **"We are landing on the word `follows` I believe — is `extends` even right?"** — Planner re-grepped `design/type-system.md:15-46` and `spec/types.md:83-205`. Both `extends` (single inheritance — fields+methods come with you) AND `follows` (multiple contracts — promise to provide signatures yourself) ARE locked in current design. They're mechanically opposite (extends = incoming code; follows = outgoing promise). Whether the keyword name `extends` itself is right is now OPEN-Q2.

2. **"For `Self` — auto-promotes with muted-text + comment learning patterns?"** — Planner confirmed: yes. `Self` resolves to the concrete implementing type (Player, not Entity, even when inherited via extends). IDE renders a muted-text annotation showing what `Self` resolved to in context; click-to-make-explicit replaces with the concrete name. Auto-promotion compatible — codegen unchanged, only IDE surface added (v0.2 LSP work). Locked as before.

3. **"`.freeze` — didn't we remove this? What's it useful for?"** — Planner re-grepped `spec/ownership.md:18,100`: `.freeze` IS still in the locked spec. But Patrick's auto-promotion observation is sound — `let → const` auto-promotion + shadow rebinding (`const buf = buf`) covers the in-place-lock use case without a dedicated keyword. Surfaced as OPEN-Q1 awaiting decision.

4. **"Other questions need examples"** — Planner provided concrete examples for Q#3 (default-expression scope), Q#4 (fat-pointer layout for `dynamic`), Q#5 (.copy on shape with non-trivial fields), Q#7 (drop order at `.give`-return). Patrick implicitly accepted Q#4/Q#5/Q#7; Q#3 remains OPEN-Q3 awaiting decision.

Other locked items in r2 (no change): Self semantics, fat-pointer layout for `dynamic Foo`, `.copy` rejected at compile-time when fields aren't transitively trivially-copyable, drop order at return-with-give.

### Concerns flagged but deferred (legitimate forward-work, not failures):
- **OOM-as-abort is "panic-then-abort" in M4** with kernel-mode plug-in deferred to v0.3 — passes `no-duct-tape.md`'s legitimate-deferral test (named follow-up trigger: v0.3 plug-in allocator; named cost: malloc-null abort behavior must be user-doc'd before tag). Recorded in `### Runtime Dependencies` and the catch-up obligations table. NOT addressed in this plan; will surface in `design/future/no-runtime-mode.md` updates during v0.3 planning.
- **Vtable layout for `dynamic` becomes ABI-load-bearing at M8** when modules land — recorded in catch-up obligations. M8 plan will revisit; M4 ships per-(shape, contract) anonymous globals with no exported symbol.
- **Field default-expression scope (constants + empty literals only)** is M4-friction — M5+ will revisit when use cases emerge. Spec/types.md update noted in P3a step 5.
- **Test count estimate ~470-520** is ±10% — accepted as inherent to milestone-level estimation. Plan-reviewer accepted; tighter scoping requires per-phase test counts which the phase bodies already provide.

No planner pushback. All round-1 concerns either fixed inline or marked as legitimate forward-work.
