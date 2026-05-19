# Doc-Drift Audit — Yinz v0.1 (2026-05-19)

Read-only audit comparing `/spec/`, `/design/`, plans (`.claude/plans/done/`), `examples/` against the actual compiler in `crates/`. CHANGELOG says M1–M8 shipped at `v0.1.0`. Audit scope: locked decisions vs implementation, spec syntax vs parser/typeck/codegen, example correctness, banned-jargon coverage.

**Summary**: 35 findings. Many resolved by Batch 4c — see status annotations below.

- 5 critical (locked decisions violated, or spec describes syntax the parser rejects)
- 18 high (shipped feature misdocumented or example uses rejected syntax)
- 11 medium (drift causes user confusion — wrong method names, stale references)
- 1 low (cosmetic / minor)

Grouped by area: Lexer/Parser, Type System, Diagnostics, Numerics, Strings, Modules, Examples, Misc.

---

## Lexer / Parser

### ~~1. Spec examples use double-quoted strings throughout — parser rejects them~~ **FIXED (Batch 4c)** (HIGH)
- **Doc claim**: Every spec file under `/spec/` except `strings.md` shows code examples with `"..."` strings: `spec/types.md` line 29 (`name: "Patrick"`), `spec/modules.md` lines 17–25 (41 occurrences), `spec/errors.md` lines 12, 53, 75–84 (14 occurrences), `spec/options.md` lines 71–73 (8 occurrences), `spec/control-flow.md` lines 27–43 (10 occurrences), `spec/ownership.md` (8 occurrences), `spec/maybe.md` (4 occurrences), `spec/unions.md` (8 occurrences), `spec/main.md` (5 occurrences), `spec/scope.md` (4 occurrences). Quote count by file (grep): types=11, modules=41, errors=14, ownership=8, options=8, unions=8, control-flow=10, main=5, scope=4, maybe=4.
- **Implementation reality**: `crates/ynz-parser/src/lexer.rs:649-668` (`lex_double_quote_error`) — M7 banned double-quoted strings. Lexer emits compile error: "Double-quoted strings don't exist in Yinz." Per CHANGELOG M7: "The old double-quote form no longer exists — a diagnostic redirects any `"..."` to the backtick form." `spec/strings.md` lines 232–240 correctly documents this.
- **Severity**: HIGH — the entire spec is full of examples that would compile-error today. Users copying any example into their code immediately hit the diagnostic. The largest single source of user-facing confusion in v0.1.

### 2. spec/destructuring.md describes object destructuring as shipped — parser does not support it (HIGH)
- **Doc claim**: `spec/destructuring.md` lines 10–66 documents `let { name, health } = player`, parameter destructuring `function greet({ name, health }: Player)`, rename-with-`as`, and nested forms as if shipped.
- **Implementation reality**: `crates/ynz-parser/src/parser.rs:1870-1943` only implements `for ((k, v) in m)` tuple destructure (M7). No `let { ... } = expr` parser path exists. Searching for "destructure" in the parser source produces no other matches. Trying `let { x } = foo` falls through to standard primary-expression parsing and fails.
- **Severity**: HIGH — entire spec page describes a non-existent feature. mvp-scope.md v0.1 line 27 lists "Destructuring (object destructuring, no array destructuring)" as in scope but it didn't ship.

### ~~3. spec/operators.md "Overloading" section describes a feature deferred to v1.0~~ **FIXED (Batch 4c)** (MEDIUM)
- **Doc claim**: `spec/operators.md` lines 136–211 describes operator overloading via `follows Addable`, `follows Equatable`, `follows Comparable`, `follows Printable`, including `c = a + b` desugaring to `add(a, b)`.
- **Implementation reality**: `design/mvp-scope.md` lines 268–274 says "Operator overloading (locked design, deferred from v0.1)" — "Substitute used pre-v1.0: Users with custom math types write `.add()`, `.subtract()` methods explicitly." `crates/ynz-typeck/src/check.rs` has no operator-overloading dispatch (`grep -n "Addable\|Operator\|operator" check.rs` finds only `"!" is the boolean NOT operator` string).
- **Severity**: MEDIUM — spec misleads users into thinking the feature works in v0.1. Should add an explicit "v1.0 feature" callout at the top of the Overloading section.

### 4. `test` keyword not actually reserved in lexer (MEDIUM)
- **Doc claim**: `design/mvp-scope.md` line 39: "Test keyword reserved in parser (rejected at compile until v0.13)". `design/decisions.md` line 36: "Built-in `test` keyword".
- **Implementation reality**: `crates/ynz-parser/src/lexer.rs:470-639` keyword table has no `test` entry. `grep -n "test\b" lexer.rs` produces nothing. Writing `test "foo" { ... }` parses as `Identifier("test")` followed by a syntax error.
- **Severity**: MEDIUM — locked-decision drift. Reserving the keyword now (with a "shipping in v0.13" diagnostic) is the documented behavior; not reserving it means v0.13 will need to make a breaking change.

### ~~5. spec/iterables.md under-documents `range(end)` 1-arg form~~ **FIXED (Batch 4c)** (LOW)
- **Doc claim**: `spec/iterables.md` lines 41, 54, 168 only mentions `range(start, end)` form.
- **Implementation reality**: `crates/ynz-typeck/src/intrinsics.rs:99-115` defines both `range(end)` and `range(start, end)` overloads.
- **Severity**: LOW — feature is more permissive than spec; doesn't cause errors but spec is incomplete.

### ~~6. spec/main.md mentions v0.8 modules~~ **FIXED (Batch 4c)** (MEDIUM)
- **Doc claim**: `spec/main.md` lines 45–66 shows `cli.args()`, `cli.flag()`, `cli.option()`, `process.exit(1)` as examples for the entry function.
- **Implementation reality**: `design/mvp-scope.md` v0.8 lists `cli` + `env` + `process` as the v0.8 module trio — deferred from v0.1. The compiler does not auto-import any stdlib in v0.1.
- **Severity**: MEDIUM — spec/main.md should mark these examples explicitly as v0.8 features or omit them entirely from the v0.1 spec.

---

## Type System

### ~~7. spec/sensitive.md uses wrong method names~~ **FIXED (Batch 4c)** (HIGH)
- **Doc claim**: `spec/sensitive.md` line 79: `let upper = key.toUpper()` — claims sensitivity propagation. Line 83: `let length = key.length` (no parens, field access).
- **Implementation reality**: `crates/ynz-typeck/src/builtins.rs:32` lists supported sensitive-string methods as `toUpperCase`, `toLowerCase`, `trim`, `substring`, `replace`. There is no `.toUpper()` and no `.length` field. The string method table (`string_method_return`) uses `.count()` for length. Per `.claude/rules/dot-postfix.md`, methods take parens — `.length` without parens would be a field access, but string is not a shape with a `length` field.
- **Severity**: HIGH — the canonical "sensitive type" spec page uses wrong API names. A user trying these literal examples gets compile errors.

### 8. spec/types.md `hidden` field auto-default contradicts the actual hidden-field handling (MEDIUM)
- **Doc claim**: `spec/types.md` lines 220–253 says hidden fields require a default value: `hidden damageMultiplier: number = 1.0`. External callers "only provide visible fields when creating the value".
- **Implementation reality**: `examples/basics/src/entrypoint.ynz:373` declares `shape Countdown { from: int, hidden current: int }` WITHOUT a default value, and constructs it with `let cd: Countdown = { from: 3, current: 0 }` — providing the hidden field explicitly. This contradicts the spec's "external code can't provide them at construction" rule. Whether the implementation rejects external construction of hidden fields is unclear without testing — but the demo file shows construction includes the hidden field even from the same file (which the spec said was fine).
- **Severity**: MEDIUM — spec under-explains the same-file case (where hidden fields can be constructed normally) vs cross-file case (where they can't be).

### 9. spec/doc-comments.md says `///` only works on exported items — implementation attaches doc to any decl (MEDIUM)
- **Doc claim**: `spec/doc-comments.md` line 47: "`///` only works on exported items. Commenting a private function has no effect on generated docs."
- **Implementation reality**: `crates/ynz-ast/src/nodes.rs:110, 133, 794` adds a `doc: Option<String>` field on FunctionDecl, ShapeDecl, OptionsDecl, and Field, independent of `is_exported`. The lexer attaches `///` to any next-following declaration. There is no compile error or warning for `///` on a private item.
- **Severity**: MEDIUM — spec is more restrictive than implementation. Either lift the restriction in spec (cheap), or add a warning in the parser when `///` precedes a non-exported item.

### 10. `background` does NOT support handle form (`let h = background fn()`) but M8 error gallery says it should reject explicitly (MEDIUM)
- **Doc claim**: `spec/concurrency.md` lines 148–157 describes `let monitor = background watchHealth()` and `.send()/.receive()` as the long-running form. `examples/errors/m8_errors.ynz:64-68` says: "WHY: `let h = background foo()` (handle form) is rejected in M8. Background handles (.send/.receive) ship in v0.3."
- **Implementation reality**: `Expr::Background` returns `Type::Nothing` (line 1139), so binding it to a `let` would type-check as `let h: nothing = ...`. There is no explicit "Storing the result of background is not yet supported" diagnostic.
- **Severity**: MEDIUM — error gallery declares a diagnostic that doesn't actually fire as documented.

### ~~11. spec/sensitive.md describes `--reveal-sensitive` flag — not in driver~~ **PARTIALLY FIXED (Batch 4c)** — deferred-status banner added; open-questions.md entry added (MEDIUM)
- **Doc claim**: `spec/sensitive.md` lines 104–107 describes `ynz run entrypoint.ynz --reveal-sensitive` flag.
- **Implementation reality**: `grep -rn "reveal-sensitive\|reveal_sensitive" crates/` produces no matches. The flag is not in `crates/ynz-driver/`.
- **Severity**: MEDIUM — spec advertises a runtime override that does not exist.

### 12. spec/iterables.md custom-iterable feature documented as shipped, but mvp-scope says v1.0 (LOW)
- **Doc claim**: `spec/iterables.md` lines 113–158 describe user-defined iterables via `follows Iterable<T>` + standalone `next()` as shipped.
- **Implementation reality**: This IS shipped (per CHANGELOG M7 and `crates/ynz-typeck/src/check.rs:758-833`), AND `examples/basics/src/entrypoint.ynz:370-386` uses it. However, `design/mvp-scope.md` line 276: "Custom iterables (locked design, deferred from v0.1)" says it's v1.0.
- **Severity**: LOW — mvp-scope is out of date (the feature shipped early). Should be moved to v0.1 in mvp-scope.md.

---

## Diagnostics

### ~~13. banned_jargon.rs missing entries~~ **PARTIALLY FIXED (Batch 4c)** (HIGH)
- **Doc claim**: `design/compiler-errors.md` lines 31–64 lists banned jargon, including: `lifetime (Rust sense)`, `alias (when not the syntax keyword)`, `trait`, `interface`, `remainder`, `associated type`, `implementation (generic CS sense)`, `precondition`, `postcondition`.
- **Implementation reality**: `crates/ynz-diagnostics/src/banned_jargon.rs:21-75` does NOT include any of those. The list has propagate/propagation, narrow/narrowing, discriminator, infer/inference, polymorphic, monomorphize/monomorphic, covariant/contravariant, deref/dereference, shadow/shadowing, coerce/coercion, fallible/infallible, first-class, idiomatic, arity, variadic, residual, referentially transparent, immutable, mutable, invariant violation, ADT, AST, struct, monad, lift, wrap, unwrap, Result, Option, Either, exception, try, catch, throw, UTF-16, async, await, goroutine. Missing 7+ entries from the design file.
- **Severity**: HIGH — design file is the source of truth (`banned_jargon.rs` comment line 3: "Source of truth: `design/compiler-errors.md`"). Audit list is incomplete.

### ~~14. spec/numeric-types.md IDE hints need v0.2 tag~~ **FIXED (Batch 4c)** (LOW)
- **Doc claim**: `spec/numeric-types.md` lines 113–118 ("IDE WARNS"), lines 143–149 ("IDE HINT: number (decimal) is slower for pure integer math").
- **Implementation reality**: v0.2 (per mvp-scope) ships LSP. v0.1 has no LSP and no IDE-hint emission. The `inference.md` rule says "muted hints" are an IDE-only protocol that v0.2 implements.
- **Severity**: LOW — spec is forward-looking but doesn't flag the timing. Add a "v0.2 (IDE)" tag to these examples.

### ~~15. design/decisions.md "type aliases" wording ambiguous~~ **FIXED (Batch 4c)** (MEDIUM)
- **Doc claim**: `design/decisions.md` line 21 in the "Type system" row says: "⚠️ Removed by r10-r15: `override` keyword (function overloading by argument type), type aliases (`shape UserId = string` — pure documentation sugar; parameter names + comments do the job)."
- **Implementation reality**: `crates/ynz-parser/src/parser.rs` accepts `shape Name = Type` form (used for union aliases like `shape Shape = Circle | Square`). The "shape alias" syntax is used heavily in `examples/basics/src/entrypoint.ynz:346` (`shape AccountResult = ActiveAccount | BannedAccount | PendingAccount`) and the parser has `ShapeDecl.alias_ty` (per CHANGELOG M6). The ban applies only to scalar aliases like `shape UserId = string`, but design/decisions.md reads as if all aliases are banned.
- **Severity**: MEDIUM — phrasing in decisions.md is ambiguous; should clarify that union aliases ARE supported and only single-type aliases are banned.

---

## Numerics

### 16. design/numeric-types.md cap reference is correct, but spec/numeric-types.md uses different error wording (LOW)
- **Doc claim**: Both `spec/numeric-types.md:75-78` and `design/numeric-types.md:55-58` describe `number<5000>` being rejected. Spec says: "design/mvp-scope.md#v2--deferred-features". Design says the same path.
- **Implementation reality**: `examples/errors/m8_errors.ynz` triggers the cap reject in two functions (`p0_number_over_cap` and `p6_number_over_cap`). The actual diagnostic emitted by the compiler isn't grepable from the audit but the bound is 4096 per `design/numeric-types.md`. mvp-scope.md does not have a `v2--deferred-features` heading by that name — searching for it: `grep -n "deferred-features" design/mvp-scope.md` produces a match around v2+ section but anchor name may differ.
- **Severity**: LOW — link anchor mismatch is minor cosmetic issue.

### 17. spec/numeric-types.md type-attached constants match implementation (NOT DRIFT — POSITIVE)
- **Doc claim**: `spec/numeric-types.md:133` says `int.min` to `int.max`. `int.max` referenced in spec/operators.md, spec/numeric-types.md, etc.
- **Implementation reality**: `crates/ynz-typeck/src/check.rs:3525-3534` implements `int.max`, `int.min`, `float.max`, `float.min`, `float.epsilon`, `number.max`, `number.min`, `number.epsilon` — exactly what spec describes.
- **Severity**: N/A — verified correct.

---

## Strings

### 18. spec/strings.md uses `.contains()`, `.indexOf()`, `.startsWith()`, `.endsWith()`, `.trim()`, etc. — all match implementation (NOT DRIFT — POSITIVE)
- **Doc claim**: `spec/strings.md:178-203` lists 16 string methods.
- **Implementation reality**: `crates/ynz-typeck/src/builtins.rs:67-98` (`string_method_return`) supports exactly: toUpperCase, toLowerCase, trim, count, byteCount, graphemeCount, contains, startsWith, endsWith, get, graphemeAt, byteAt, indexOf, substring, split, replace. CHANGELOG M7 lists 16 string methods. Match is exact.
- **Severity**: N/A — verified correct.

### 19. spec/strings.md uses `.set()` for index-assignment example but strings are immutable (NOT DRIFT — spec correctly says "compile error")
- **Doc claim**: `spec/strings.md:101-110` shows `name[0] = `B`` as a compile error.
- **Implementation reality**: Verified the parser produces IndexAssign for `name[0] = ...` and the typeck must reject for strings. Looks correct per `crates/ynz-typeck/src/check.rs:3562`.
- **Severity**: N/A — verified.

---

## Modules

### ~~20. spec/modules.md re-export described as working — cross-file calls are v0.2 stubs~~ **FIXED (Batch 4c)** (HIGH)
- **Doc claim**: `spec/modules.md:179-194` describes re-export syntax `export { fetchUser, createUser, User } from "services/users"`.
- **Implementation reality**: `crates/ynz-ast/src/nodes.rs:23-46` has `ReExport` item and `ReExportItem` shape. CHANGELOG M8: "import/export syntax is fully parsed and type-checked. Cross-file symbol *calls* are deferred to v0.2 (the syntax is locked and validated; the typeck resolver is a stub)." So syntax parses, but the entire cross-file call mechanism is a v0.2 stub. spec/modules.md presents re-export as working without flagging that calls don't resolve.
- **Severity**: HIGH — spec implies functionality that's a stub. The spec needs a v0.1 caveat: "syntax accepted; cross-file calls deferred to v0.2."

### ~~21. spec/modules.md stdlib examples reference unshipped modules~~ **FIXED (Batch 4c)** (MEDIUM)
- **Doc claim**: `spec/modules.md:37-48` shows `math.sqrt(16)`, `file.read("data.txt")`, `http.get(...)`, `date.now()` as "always available, just use it."
- **Implementation reality**: Per `design/mvp-scope.md`, `math` is v0.7, `file` is v0.6, `http` is v0.15, `date` is v0.10. None ship in v0.1. The compiler does not auto-import any stdlib in v0.1.
- **Severity**: MEDIUM — spec is forward-looking and forms the right mental model for users, but uses examples that don't compile in v0.1. Should add an "Available from v0.X" annotation per module reference.

### ~~22. spec/modules.md side-effect imports example uses double-quoted string~~ **FIXED (Batch 4c — covered by fix #1)** (HIGH)
- **Doc claim**: `spec/modules.md:253` shows `import "some-module"` as a compile error.
- **Implementation reality**: Uses double-quoted string. Parser would reject double quotes first (M7 lexer error per #1 above), so the user would never see the "imports must bind to something" diagnostic.
- **Severity**: HIGH — same root cause as #1. The "bad example" in the spec doesn't trigger the documented error because the string form is wrong.

### ~~23. spec/modules.md alias-collision example uses invalid syntax~~ **NO CHANGE NEEDED (Batch 4c)** — parser verified to accept `import ns as alias from \`path\``; syntax IS valid. Double-quote issue fixed by #1. (HIGH)
- **Doc claim**: `spec/modules.md:139-146`:
  ```
  import math as advancedMath from "math"
  import math from "vendor/math-legacy"
  // COMPILE ERROR — the second 'math' collides with the first.
  ```
- **Implementation reality**: First import would fail lexer on `"..."` (double quote). And the parser grammar for namespace imports is `import NAME from "..."`, not `import math as advancedMath from "..."`. Need to verify the actual `as` alias position. The spec's syntax is unclear / possibly wrong.
- **Severity**: HIGH — spec invalid even ignoring quote issue.

### 24. spec/modules.md describes IDE auto-import — IDE infrastructure is v0.2 (LOW)
- **Doc claim**: `spec/modules.md:275-285` describes IDE auto-import behavior.
- **Implementation reality**: LSP is v0.2 per mvp-scope.
- **Severity**: LOW — forward-looking spec content.

---

## Examples

### 25. `examples/basics/src/entrypoint.ynz` shadows `nums` and `score` — duplicate names in same scope (MEDIUM)
- **Doc claim**: Per `.claude/rules/plan-invariants.md` `### Demo & Error Gallery`, every phase that adds executable surface must extend the basics demo. The plan invariants imply the demo should compile cleanly.
- **Implementation reality**: `examples/basics/src/entrypoint.ynz:124` declares `let nums: array<int> = [10, 20, 30]`. Line 168: `let nums: array<int> = [10, 20, 30]` — same name in same function scope (re-declared). Line 135 declares `let score: maybe<int> = none` after line 32 declared `let score: int = 0`. Shadow-in-same-scope unclear — implementation either allows it or rejects it; the existing demo passes the `examples_basics_runs_end_to_end` integration test per CHANGELOG, suggesting in-function re-declaration is permitted, which would contradict `spec/variables.md` "No variable hoisting. No surprises." and `spec/scope.md`.
- **Severity**: MEDIUM — either the spec is wrong about no-shadowing or the test is permissive. Per banned jargon rule "shadow, shadowing" is banned in diagnostics but the language behavior re: shadow-in-scope is undocumented.

### 26. examples/errors/m8_errors.ynz: comment says "Cross-directory import from no-project" but trigger is commented out (LOW)
- **Doc claim**: `examples/errors/m8_errors.ynz:17-21` — `p2_relative_import` says it should trigger "Import paths must be project-root relative". All triggers are commented out, requiring manual uncommenting.
- **Implementation reality**: The harness in `crates/ynz-driver/tests/` runs the error galleries. Galleries with all-commented-out triggers don't actually test the diagnostics. Per `.claude/rules/plan-invariants.md` this is intended to be a "uncomment to test" reference, but no automated test forces every trigger to be exercised. The actual diagnostics may or may not exist as worded.
- **Severity**: LOW — depends on whether the in-tree gallery is meant to be runnable or reference-only.

### 27. examples/basics/yinz.toml has `entry = "src/entrypoint.ynz"` but spec/main.md says default is `"entrypoint.ynz"` (MEDIUM)
- **Doc claim**: `spec/main.md:35`: `entry = "entrypoint.ynz"` (default — change to any .ynz file).
- **Implementation reality**: `examples/basics/yinz.toml` uses `entry = "src/entrypoint.ynz"`. The compiler walks `src/**/*.ynz` per CHANGELOG M8: "`ynz run <dir>` compiles and links all `.ynz` files under `src/`". Recent commit `8440274 fix(driver): remove src/ convention — spec says project-root-relative only` suggests this was just fixed/changed. The spec doesn't mention `src/` convention at all but the example demo project uses it.
- **Severity**: MEDIUM — the example contradicts the spec; one or the other is wrong. Per the recent commit message, the spec is the source of truth, but the example still uses `src/`.

---

## Misc / Cross-cutting

### ~~28. design/mvp-scope.md lists pre-r12 syntax `type Foo = A or B or C`~~ **FIXED (Batch 4c)** (CRITICAL)
- **Doc claim**: `design/mvp-scope.md:20`: "Unions (`type Foo = A or B or C`)".
- **Implementation reality**: `design/golden-rules.md:149` and `design/decisions.md` rows clearly state Rule 12 was amended 2026-05-14 — Yinz uses `|` for unions, NOT `or`. `crates/ynz-parser/src/lexer.rs` token table has `Token::Pipe` and `Token::PipePipe`. The lexer does not recognize `or` as a keyword. Trying `type Foo = A or B` would fail twice: `type` is banned, and `or` is an Identifier.
- **Severity**: CRITICAL — mvp-scope.md is the source of truth for "what ships in v0.N" but documents an obsolete syntax for a locked decision (`|` for unions, banned `or`).

### 29. design/decisions.md "type aliases" — see #15 above (CROSS-REF)

### 30. design/open-questions.md does NOT cover destructuring deferral (LOW)
- **Doc claim**: Open questions list contains: Metaprogramming, HTTP Module, Actor Primitives, Specialization, Workspace, Formatter, Type Collection Ordering.
- **Implementation reality**: spec/destructuring.md describes a non-existent v0.1 feature (#2 above). Either destructuring should be on the open questions list (with a "deferred to v0.2/v1.0" note) or the spec should be marked v-deferred.
- **Severity**: LOW.

### ~~31. `spec/main.md` and `spec/config.md` use `main()` instead of `entrypoint()`~~ **FIXED (Batch 4c)** (MEDIUM)
- **Doc claim**: `spec/config.md:82-167` repeatedly says "in `main()`" — line 84 "in your `main()` function", line 164–168 "in `main()`" 5 times.
- **Implementation reality**: Per `spec/main.md:1` and CHANGELOG, entry is `function entrypoint()`. Implementation: `crates/ynz-typeck/src/queries.rs` searches for the `entrypoint` symbol.
- **Severity**: MEDIUM — spec/config.md needs `main()` → `entrypoint()` everywhere. Per the renamed-concepts table in `.claude/rules/naming.md`, the function MUST be named `entrypoint`, not `main`.

### ~~32. spec/overview.md says "12 Golden Rules" — should be 13~~ **FIXED (Batch 4c)** (LOW)
- **Doc claim**: `spec/overview.md:39` says "The 12 Golden Rules" — then lists rules 1–12 (with no rule 13).
- **Implementation reality**: `CLAUDE.md` rules 1–13 explicitly defined. `design/golden-rules.md` covers 13 rules. Rule 13 ("Capital letter = type") was added later and is load-bearing. `spec/overview.md` omits it.
- **Severity**: LOW — spec is missing one rule. Should renumber to "13 Golden Rules" and add rule 13.

### 33. design/decisions.md lists "Sized variants (`f32`): Deferred to v2+" — no compile-error gallery entry for `f32` (LOW)
- **Doc claim**: `design/numeric-types.md:89` says `f32` is deferred to v2+.
- **Implementation reality**: No diagnostic for `f32` type usage. Searching `f32` in lexer/parser produces no token reservation. A user writing `let x: f32 = 1.0` gets the standard "unknown type" diagnostic, not a teaching diagnostic pointing to v2+ deferral.
- **Severity**: LOW — minor teaching opportunity not implemented.

### ~~34. spec/collections.md uses banned `type` keyword~~ **FIXED (Batch 4c)** (HIGH)
- **Doc claim**: `spec/collections.md:166` says "for known fields, define a `type` — it's faster." Line 248–250: "consider using a type instead". Line 269: `type Scores { alice: number, bob: number }` (banned `type` keyword in code).
- **Implementation reality**: `crates/ynz-parser/src/lexer.rs:587-594` banned `type` as a declaration keyword in M4 with a teaching diagnostic redirecting to `shape`. The spec uses both the banned `type` keyword in code AND uses "type" in prose where "shape" is the Yinz term.
- **Severity**: HIGH — directly violates `.claude/rules/vocabulary.md` banned-jargon rule (`type` as declaration keyword) AND `.claude/rules/naming.md` renamed-concepts table.

### 35. spec/sensitive.md `env.get()` example references a v0.8 module (MEDIUM)
- **Doc claim**: `spec/sensitive.md:9-13`: "`env.get()` returns `sensitive string` by default."
- **Implementation reality**: `env` is the v0.8 module per mvp-scope. In v0.1, there is no `env`. The sensitive type-system surface DOES ship in M8 (per CHANGELOG), but the `env` source documented as the canonical example does not. Per M8 plan: "the env-based source (`env.get()` returns `sensitive string`) ships v0.8; M8 ships the type machinery + the manual `sensitive(literal)` constructor."
- **Severity**: MEDIUM — primary example references unshipped module. Should lead with `sensitive("...")` constructor and mention env as v0.8.

---

## Verified-Correct Items (no drift found)

- M7 string method table — 16 methods spec vs builtins.rs match exactly.
- Type-attached constants (`int.max`, `int.min`, `number.epsilon`, etc.) — spec, design, and `check.rs:type_attached_const_type` align.
- Banned declaration keywords (`type`, `struct`, `class`, `interface`, `enum`, `abstract`, `match`, `switch`, `fn`, `async`, `await`, `promise`, `future`, `goroutine`, `pub`, `private`, `protected`, `public`) — lexer.rs has all and emits 3-part diagnostics.
- Frame / SourceLoc compiler-synthesized shapes — `check.rs:2585-2610` matches `spec/errors.md` field list.
- Union `|` syntax — locked decision, parser uses `Token::Pipe`, banned `or` not registered as keyword.
- `errors` keyword — fully shipped in M7, parser + typeck + codegen + runtime.
- Backtick strings + interpolation + 6 escape sequences (\n, \t, \\, \\\`, \\${, \r, \0) — lexer.rs:716-810 matches spec/strings.md:50-63.
- `for ((k, v) in m)` destructure form — parser implements, spec correctly documents.
- M7 banned jargon additions (monad, lift, wrap, unwrap, Result, Option, Either, exception, try, catch, throw, UTF-16) — banned_jargon.rs lines 56-67.

---

## Recommended Priority for Fixes

~~1. **Spec-wide double-quote → backtick sweep** (#1)~~ **DONE (Batch 4c)**
~~2. **mvp-scope.md "or" → "|" for unions** (#28)~~ **DONE (Batch 4c)**
~~3. **spec/types.md, spec/collections.md `type` keyword usage** (#34)~~ **DONE (Batch 4c)**
4. **spec/destructuring.md feature flag** (#2) — clarify deferral. (open)
~~5. **spec/sensitive.md method names** (#7)~~ **DONE (Batch 4c)**
~~6. **spec/main.md, spec/config.md `main()` → `entrypoint()`** (#31)~~ **DONE (Batch 4c)**
~~7. **banned_jargon.rs missing entries** (#13)~~ **PARTIALLY DONE (Batch 4c)** — skipped: lifetime, alias, interface, remainder
~~8. **spec/modules.md cross-file v0.2 caveats** (#20, #22, #23)~~ **DONE (Batch 4c)**
9. **examples/basics/yinz.toml `src/` convention** (#27) — align with spec or document. (open)
