# Teaching & Golden-Rules Audit — 2026-07-11

Scope: every user-facing teaching surface — compiler diagnostics (lexer, parser, typeck,
driver, codegen), IDE surfaces (LSP hover, inlay hints, code actions, completion), and the
registry teaching text ([`registry/features.toml`](../../../registry/features.toml)) — audited
against Golden Rule 11 (WHAT/WHAT-INSTEAD/WHY), Golden Rule 12 (human-readable over jargon),
Golden Rule 13 (capital = type), the spec-writing audience bar (HS grad who knows JS),
[`inference.md`](../../../.claude/rules/inference.md), [`vocabulary.md`](../../../.claude/rules/vocabulary.md),
[`dot-postfix.md`](../../../.claude/rules/dot-postfix.md), and
[`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md).

Method: manual read of all ~394 `Diagnostic::` construction sites plus the LSP handlers and
registry teaching sections. Not audited in depth: `ynz-fmt`/`ynz-watch` CLI strings (covered by
existing jargon-audit tests), per-gallery coverage completeness (recommendation only, §F3).

Overall verdict: the teaching machinery is structurally excellent — three-part format enforced
in the type system, galleries exist, most WHYs are genuinely contextual. The defects below are
mostly **drift** (parallel copies of teaching text disagreeing) and **leakage** (internal
vocabulary/paths escaping into user-facing text). Findings ordered most-severe-first within
each class.

---

## A. Broken or contradictory teaching (highest priority)

### A1. Registry `[[diagnostic_template]]` table is a dead parallel copy — and it has already drifted
- **Where**: `registry/features.toml:1494–1681`; consumers: **none** (only
  `crates/ynz-registry/tests/` read `diagnostic_templates()` — no compiler or LSP code does).
- **What**: 26 templates hold canonical WHAT/WHAT-INSTEAD/WHY text, but `check.rs`/`parser.rs`
  emit hand-written inline strings. Two sources of truth for the same teaching text.
- **Proven drift**: `BackgroundLargeStructCopy` (features.toml:1617–1621) tells the user to
  write `background fn(value.give)` — **`.give` is not typeable body syntax** (inference.md:
  ownership modifiers exist only in signatures; `inlay_hint.rs:293–296` even documents that the
  live diagnostic was reworded away from `.give` for exactly this reason). Its WHY also claims
  "Auto-detection of unused-after-call ships in v0.3-M3; until then, the choice is yours" —
  while the live diagnostic (`check.rs:2936–2958`) teaches the opposite: the compiler already
  transfers ownership automatically. Anyone who wires the registry template up (or reads it as
  canon) ships wrong teaching.
- **Class**: authoritative-derivation.md violation *inside the teaching subsystem* — the exact
  parallel-implementation disease the rule exists to kill, with a live drifted victim.
- **Fix options** (pick one, don't leave both): (a) make firing sites render through
  `diagnostic_template_lookup` the way `lint_rule_diagnostic_parts` already works for lints —
  one home, mechanical parity; or (b) delete the table and record that per-site strings are the
  authority (then the LSP `Diagnostic.code` mapping keeps `kind_name` only). Either way, fix
  the `BackgroundLargeStructCopy` text immediately.

### A2. Inlay-hint hover fallback teaches users to type non-typeable text
- **Where**: `crates/ynz-registry/src/lib.rs:187–193` (`lsp_inlay_hint_hover_for` WHAT-INSTEAD
  fallback: "``{example_hint_rendered}`` — write this to make the decision explicit in source.").
- **What**: only 3 of 13 muted-hint domains define explicit `hover_what_instead`
  (`parallel_groups`, `channel_capacity`, `auto_arc`). Every other **firing** domain gets the
  fallback, which renders garbage teaching:
  - `ownership_call_site` (Informational, firing) → hover says *write*
    `share (read-only — matches foo's signature)` in source. Not Yinz. inference.md explicitly
    says there is NO body-level ownership syntax; the correct WHAT-INSTEAD is "nothing to type —
    click jumps to the signature".
  - `background_routing` (Informational, firing) → hover tells the user to write
    `// routed to I/O pool — calls sleep (may suspend)` as source.
  - `copy_points` (firing) → hover tells the user to write `.copy (8 bytes, trivially copyable)`.
  - `variable_type` → fallback uses the registry example `: int (from 42)` — `(from 42)` is not
    typeable either.
  The function's own doc comment (lib.rs:171–175) admits Informational domains "MUST set
  `hover_what_instead`" — and the three firing ones don't. The failure mode Golden Rule 11 calls
  out ("muted annotations the user cannot learn from") is here inverted: annotations the user
  learns the *wrong thing* from.
- **Fix**: add explicit `hover_what` / `hover_what_instead` / `hover_why` to every FIRING domain
  (`variable_type`, `ownership_call_site`, `copy_points`, `wait_points`, `background_routing`,
  `array_to_fixed_promotion`, `let_to_const_promotion`), and add a registry consistency test:
  any domain reachable from `inlay_hint.rs` must not rely on the generic fallback for
  WHAT-INSTEAD. The existing `no_infer_jargon_in_lsp_inlay_hint_hover_why_source` test shows
  the pattern.

### A3. The compiler contradicts itself on import-path quoting
- **Where**: `parser.rs:636–640` ("Import paths must be **backtick strings**." with a
  backtick-string example) vs. a dozen other import/export diagnostics whose examples teach
  **double-quoted** paths: `parser.rs:305–317` (`export { name } from "path"`),
  `parser.rs:360–364`, `parser.rs:372–377`, `parser.rs:418–423`, `parser.rs:455–459`,
  `parser.rs:475–479`, `parser.rs:533–538`, `parser.rs:663–668`, `parser.rs:705–710`, and
  `resolve_import.rs:226/298` (`import ns as otherName from "..."`).
- **What**: whichever form is canonical, half the teaching examples are wrong — and Yinz has
  "one string form — backtick strings" per the lexer's own double-quote error
  (`lexer.rs:777–783`). A user following the `Expected `from`…` error's example will
  immediately hit the double-quote error. (The parser currently *accepts* `Token::StringLit`
  in `parse_import_path` — parser.rs:579 — which itself contradicts the lexer's "double-quoted
  strings don't exist" teaching; decide the canon and align both code and examples.)
- **Fix**: sweep every import/export diagnostic's example to backtick paths (or lock
  double-quote paths in the spec first — but one answer everywhere).

### A4. Escape-sequence teaching lists `\0` as valid while a dedicated error rejects it
- **Where**: `lexer.rs:884–886` (incomplete-escape WHY lists `` \0 `` among valid escapes) and
  `lexer.rs:929–931` (unknown-escape WHAT-INSTEAD: "Valid escape sequences: … `\0`.") vs.
  `lexer.rs:912–917` ("`\0` (NUL byte) is not a valid escape in Yinz strings.").
- **What**: two teaching surfaces advertise `\0`; a third rejects it with a good explanation.
  A user following the "valid escapes" list gets an error on the very next compile.
- **Fix**: drop `\0` from both "valid escapes" lists (or phrase as "recognized but rejected —
  see the `\0` error"). One-line fixes.

### A5. Spec files teach `maybe T` syntax the parser rejects
- **Where**: `docs/reference/REF-doc-comments.md:28` (`-> maybe User errors`),
  `REF-variables.md:106` ("Use `maybe string`"), `REF-iterables.md:38,42` (`-> maybe T`),
  `REF-golden-rules.md:108`; also `.claude/rules/naming.md` + `vocabulary.md` ("`maybe T`").
  The parser requires angle brackets: `parse_maybe_type` errors without `<`
  (`parser.rs:1041–1048`: "`maybe` requires a type argument. Write `maybe<T>`…"), and
  `REF-maybe.md` + all diagnostics consistently use `maybe<T>`.
- **What**: the user-facing spec (the HS-grad-facing truth) teaches syntax that does not
  compile. Either the language grew `maybe<T>`-only and four spec/rule files went stale, or
  bare `maybe T` was intended and never implemented. Every code example in a REF file must
  compile (spec-writing.md); these don't.
- **Fix**: decide canon (likely `maybe<T>`, matching the compiler), sweep the REF files and the
  two `.claude/rules` tables, and note the decision in `IMP-type-system.md`.

### A6. Mixed messaging on when to write `wait`
- **Where**: `[[keyword]] wait` hover (`features.toml:170–172`): "Suspension at I/O points is
  automatic and does not require `wait` … you never write `wait` for that." vs.
  `prefer-yielding-sleep` lint (`features.toml:2324`): "Write `wait sleep({ms})`…" and
  `check_sleep_call` (`check.rs:3933–3958`): "Write `wait sleep(200)`…".
- **What**: the keyword hover teaches "never write `wait` for suspension"; two other surfaces
  teach `wait sleep(ms)` as the canonical form. If bare `sleep(ms)` is correct under the
  transitive-inference model (as the retired `UnawaitedSleepAsync` record says), the lint and
  the sleep errors should show bare `sleep({ms})` — or the hover needs a clause explaining why
  `sleep` examples still carry `wait`. A new user reading all three gets three answers.
- **Fix**: pick the canonical spelling for sleep examples and align all three surfaces; the
  design answer lives in `IMP-no-function-coloring.md`.

---

## B. Too technical — jargon/implementation leakage into user-facing text (Golden Rule 12, spec audience bar)

### B1. Banned word `infer` in keyword hovers — and the jargon audit doesn't cover those fields
- **Where**: `features.toml:172` (`wait` hover_why: "causal ordering the compiler cannot
  **infer**"), `features.toml:179` (`background` hover_what: "The compiler **infers** whether
  to move or copy…").
- **What**: `infer`/`inference` is banned in user-facing text (vocabulary.md, inference.md
  dual-audience rule); keyword hovers are user-facing IDE surfaces. The existing
  `jargon_audit.rs` covers diagnostic strings, LSP-rendered messages, deferred-feature fields,
  and the `lsp_inlay_hint_hover_for` source — but NOT `[[keyword]]`
  `hover_what/hover_what_instead/hover_why`, which is exactly where these two live.
- **Fix**: reword ("ordering the compiler can't figure out on its own"; "the compiler works out
  whether to move or copy"), and extend the jargon audit to iterate every registry field that
  renders in an editor (keyword hovers, muted-hint hover fields, lint templates).

### B2. Rust/runtime implementation details in user-facing WHYs
- `check.rs:3115` — "`sleep` requires the **Tokio** runtime (started by **`ynz_rt_init`**)".
  Tokio is the compiler's implementation detail; `ynz_rt_init` is an internal symbol. Also
  `features.toml`-mirrored kernel messages and `check.rs:3361–3363`, `check.rs:3702` repeat
  `ynz_rt_init`. Say "the scheduler runtime that Yinz starts for you" — the user can't act on
  "Tokio".
- `check.rs:790–793` — "the bounds would need to be read from the range's **frame-backed
  alloca**". `alloca` is LLVM vocabulary. The neighboring errors (1096–1107) prove this can be
  said plainly ("the entry's key-value data lives on the function's stack, which is freed…").
- `check.rs:717–723` — "needs a **variable-size frame staging slot entangled with the
  shape-return base fix** — that work is deferred." This is a commit message, not a WHY. Say:
  "Returning a whole shape from a function that can pause isn't supported yet — support ships
  in a later version."
- `check.rs:807–814` — "producing **undefined behavior**" — borderline; "reads memory that has
  already been freed — garbage or a crash" (the wording 1102–1106 already uses) is the house
  style.
- `check.rs:4310–4315` — `%`-on-`number` WHY cites "**IEEE 754-2008 §5.3.1**", "rounding modes
  (half-even, truncation)". Right decision, PhD citation. The teaching point fits the audience
  as: "different rounding rules give different answers for the same inputs, so Yinz refuses to
  guess." Keep the spec citation in `IMP-numerics`/design docs.
- `features.toml:1643` / `check.rs:263–269` — "calls a **may-block intrinsic**", "is an
  **ordering barrier** — it ends any **parallel group**". Two jargon terms and one
  un-introduced concept in one sentence, in a *warning aimed at someone who wrote a redundant
  `wait`* (i.e., a learner). "…only matters when the call can pause. This call never pauses, and
  the extra `wait` also stops the compiler from running nearby independent calls at the same
  time" carries the same content.
- `parser.rs:2486–2490` — nesting-limit WHY: "The parser uses one **stack frame** per nested
  expression… **adversarial inputs**". Low-stakes (256 levels deep), but easy to soften.

### B3. Internal doc paths cited in user-facing WHYs — three different spellings, all unreachable
- **Where** (sample): `check.rs:723,793,814,837,1107,1246` (`design/concurrency.md '…'`),
  `check.rs:2731,2748,2768,2785,3117,4854` (`design/no-runtime-mode.md`),
  `check.rs:3363,3702` and `features.toml:1609,1615` (`IMP-no-runtime-mode.md` /
  `docs/internal/implementation/IMP-no-runtime-mode.md`).
- **What**: (a) end users don't have the repo — a compiler error citing an internal design doc
  is a dead reference for its audience; (b) the `design/` spellings are stale since the
  2026-07-01 docs migration, so they're wrong even for contributors; (c) the same doc is cited
  three different ways across surfaces.
- **Fix**: user-facing WHYs should either drop the citation or use a stable feature-registry
  name ("see `background-handle-nonsuspending-callee` in the feature registry" — one message
  already does this well, `check.rs:1871–1875`). If doc pointers stay until a docs site ships,
  standardize on the real current path and swap to public URLs when
  `codeDescription` lands (`diagnostic_transform.rs:78–79` already reserves the slot).

### B4. Internal milestone identifiers in user-facing errors
- `parser.rs:1171` — "**M5** unified all generic syntax…" — a user has no idea what M5 is.
- `check.rs:4213–4215, 4795–4797` — "**v0.3-M2** requires static resolution… ships in a future
  version." The surrounding sentence is fine; the milestone tag is noise. House style elsewhere
  ("ships in a later milestone/version") is the right form — make it uniform and versionless.

---

## C. Non-conformant to the three-part format (structure present, slot misused)

### C1. WHY slot used as a stderr dump
- `build.rs:459–464` and `build.rs:640–644` — linker failure: WHY = `"Linker stderr:\n{…}"`.
  The raw output belongs in the message body or a related note; the WHY slot should say why the
  user is seeing it ("the system linker rejected the object file the compiler produced — that's
  a compiler bug, not a problem with your code"). Also: "Please report it" has no destination —
  give the `gh` URL or a `ynz bug` hint once one exists.

### C2. WHAT-INSTEAD slot holding a rule restatement instead of an action
- `parser.rs:1552–1557` — duplicate parameter: WHAT-INSTEAD = "Each parameter in a function
  must have a unique name." (that's a WHY); the actionable fix ("Rename one of them, e.g.
  `{param_name}2`") is missing. The WHY slot then repeats the same idea.
- `parser.rs:3829–3834` and `parser.rs:3841–3846` — shape-body errors: WHAT-INSTEAD = "Shape
  bodies contain fields and optional bare method signatures." — a description, not an action.
- `shapes.rs:439–447` (duplicate field) — same shape: instruction lives in WHAT-INSTEAD as a
  rule sentence.
- **Fix**: mechanical rewording pass; the slot test is "can the user copy/do this?".

### C3. Circular WHY
- `check.rs:1296–1305` — missing-return WHY: "A path that falls off the end produces no value,
  **which is a bug**." Circular. The template version (features.toml:1551) has the better WHY —
  ironic given A1. Minor.

---

## D. Redundant / drifted duplicate teaching

### D1. Kernel-mode rejection text: ~7 hand-written near-copies, already diverging
- **Where**: `check.rs:2727–2731` (wait), `2765–2769` (background), `3110–3117` (sleep),
  `3235–3239` (generic suspend), `3355–3363` (channel methods), `3698–3702` (channel ctor),
  `4850–4854` (UFCS method) + the two registry templates.
- **What**: same lesson ("no scheduler runtime in `--kernel`"), 7+ implementations; citations
  already diverge (§B3), and one names Tokio while others don't (§B2). One helper
  (`kernel_mode_rejection(feature: &str)`) makes drift impossible — the codebase already does
  exactly this for `wait_on_non_may_block_warning` (`check.rs:241–270`) and
  `reject_share_param_mutation`.

### D2. Two linker-diagnostic stacks, already drifted
- **Where**: `build.rs:397–465` (project path) vs `build.rs:569–644` (single-file path) — five
  near-identical diagnostics each. The no-linker error drifted: `build.rs:418–423` lacks the
  macOS advice and the "clang-18 is the lightest option" WHY that `build.rs:590–597` has. A
  single-file Mac user gets the worse teaching.
- **Fix**: extract the shared link-step diagnostics (reusability gate 2); keep the better text.

### D3. `[[diagnostic_template]]` dead table — see A1 (the flagship instance of this class).

Not-a-finding, for the record: "This function can fail, but the failure is not handled here."
appears twice (`check.rs:3803–3808`, `4225–4230`) with byte-identical text — that's the
documented shared-wording discipline working, though a shared helper/const would lock it.

---

## E. Naming & convention consistency inside teaching examples (Golden Rules 12/13)

### E1. snake_case rename suggestions in a camelCase language
- `check.rs:1918–1920` — "declare a new variable: `` `let my_{target} = {target}` ``" and the
  WHY's `let my_name = name`. House convention is camelCase (`myName`).
- `check.rs:1167–1169` (`inner_{crossing_name}`), `1195–1197` (`{crossing_name}_after`),
  `1244–1246` (`inner_{pname}`), `1271–1273` (`{pname}_val`) — every shadowing/rename
  suggestion manufactures snake_case identifiers. The compiler is teaching juniors the wrong
  naming style at exactly the moment they'll copy it verbatim.
- **Fix**: camelCase templates (`inner{PascalName}` is awkward with interpolation; simplest:
  `{name}2`, `updated{PascalName}`, or prose "rename one of them" plus a camelCase example).

### E2. `MAX_HEALTH` example vs Golden Rule 13's scan rule
- `parser.rs:740, 758` — const-decl errors teach `const MAX_HEALTH = 100`. GR13 says "capital
  letter = type" with zero ambiguity; naming.md defines camelCase variables and never blesses
  SCREAMING_SNAKE constants. Either the convention has an undocumented constants exception
  (then document it in naming.md) or the example should be `const maxHealth = 100`. Today the
  parser teaches an unratified style.

### E3. `.copy` vs `.copy()` — parens inconsistency (dot-postfix rule)
- `check.rs:2851` teaches `background fn(value.copy())` (correct per dot-postfix.md) while
  `check.rs:2860` — the twin `lend` diagnostic four lines later — teaches
  `background fn(value.copy)`. Registry copies: `features.toml:1602` (`value.copy`),
  `features.toml:2279` (auto_arc hover: "pass `.copy`"). Body operation ⇒ parens, everywhere.

### E4. "PascalCase" jargon vs the house phrasing
- `parser.rs:3550` — "Write `options Status { … }` with a **PascalCase** name" vs
  `parser.rs:3652` — "the name must start with a **capital letter**" (house style, GR13
  phrasing). Use the capital-letter phrasing in both.

### E5. Inline `.copy (N bytes, trivially copyable)` label
- `inlay_hint.rs:240` + `features.toml:2224` — "**trivially copyable**" is C++/Rust standardese.
  The concept fits the audience as "small enough that copying is free". Note this wording is
  inherited from inference.md's own table — fix rule + registry + label together.

---

## F. IDE-surface findings (beyond A2)

### F1. Inline hints ship weaker than the inference.md contract
- inference.md mandates informative-at-a-glance labels: `: int (from 42)`,
  `share (read-only — matches foo's signature)`, `lend (function will mutate — see bar's
  signature)`. Shipped labels (`inlay_hint.rs:207, 224`) are `: int` and ` share` — the
  one-clause WHY is dropped, which was the load-bearing half ("show WHAT was inferred AND
  WHY without hovering"). The registry `example_hint_rendered` fields still promise the full
  form, so registry, rule, and implementation are three-way inconsistent.
- **Fix**: either enrich the labels to the spec'd form (watch editor-noise tradeoffs — maybe
  gate the parenthetical behind a config) or amend inference.md + registry examples to the
  shipped form via a real decision. Don't leave the spec claiming teaching that doesn't render.

### F2. Fragile prose coupling: the `.give` IDE hint fires on a string prefix
- `inlay_hint.rs:298–314` — Domain 6 detects the large-copy warning via
  `d.what.starts_with("Copying ")`. Reword that diagnostic and the IDE hint silently stops
  firing (a teaching surface dying with no test failure). This is the mirror of
  authoritative-derivation: thread a `DiagnosticKind` (e.g. `BackgroundLargeCopy`) instead of
  matching prose. Also note the label ` .give (transfers ownership; no copy)` renders
  non-typeable syntax in an Addition-style position — fold into the A2/E3 cleanup.

### F3. `copy_points` placement-category mismatch
- Registry says `Addition` (`features.toml:2219–2224`); `inlay_hint.rs` doc table (line 12) says
  Informational; the rendered label is not typeable, so per inference.md's category test it
  cannot be Addition as rendered. Pick the category, fix the two records, and give it real
  hover fields (A2).

### F4. Gallery coverage for hint/lint surfaces — recommendation
- `examples/primantis-orders/` covers compile errors well (one file per milestone). There is no
  equivalent human-eyes-on artifact for the *IDE* teaching surfaces (muted hints, lint
  squiggles, hovers) — the layer inference.md calls the central teaching mechanism. Consider a
  small `examples/<burgh-name>/` gallery whose README lists which hints/lints each snippet
  should fire, so Patrick can review IDE UX per milestone the way he reviews error UX. (Not a
  defect; a gap in the review loop.)

---

## G. What's genuinely good (so the fixes don't regress it)

- `Diagnostic::new` panics on any empty WHAT/WHAT-INSTEAD/WHY (`diagnostic.rs:142–153`) — GR11
  encoded in the type system. Keep.
- Channel/backpressure teaching (`check.rs:3506–3521`, capacity errors) — explains backpressure
  as "that is backpressure working, not a deadlock". Model text.
- The suspension "one name must mean one value" family (`check.rs:1160–1279`) — hard compiler
  internals translated honestly. Model text (modulo the snake_case suggestions, E1).
- `array-using-soa-layout` lint WHY (`features.toml:2346`) — cites two honest measurements and
  distinguishes shipped vs future optimizer behavior. Best WHY in the codebase.
- `parallel_groups` hover triplet (`features.toml:2200–2202`) — plain-English data-dependence
  teaching. Model for the missing hover fields in A2.
- Dual-style UFCS diagnostics, the `{ ... }`-vs-`[...]` literal confusions, the map-dot-access
  error, `boolean.toInt()` refusal, lexer's `#`/`;`/`$`/`?`/quote redirects — all exactly the
  teaching mission working.

---

## H. Enforcement architecture — why the drift happens and how to make it structurally impossible

*(Added 2026-07-11 after review of the findings above with Patrick. This section is the
prevention brief: it diagnoses the failure mode, assesses the teaching rule itself, and
specifies the guards. A follow-up audit/plan should consume this section as its recon input.)*

### H0. The diagnosis — memory-based enforcement vs build-based enforcement

The finding distribution above is the whole diagnosis. Across ~394 diagnostic construction
sites there were **zero** missing-WHAT/WHAT-INSTEAD/WHY findings — because that one property is
enforced by the type system (`Diagnostic::new` panics on any empty field,
`ynz-diagnostics/src/diagnostic.rs:142–153`). Every violation found lives where the enforcement
mechanism is "a human or AI remembers the rule while writing": jargon leakage, parallel-copy
drift, stale doc paths, style slips in examples.

> **Principle: a teaching rule that fires hundreds of times cannot be enforced by memory —
> yours, an AI's, or a reviewer's. It must be enforced by the build (only path / tested path)
> or it will fail at a few percent forever.** The empty-field panic has a perfect record; the
> six rule documents enforced by recall have ~30 violations. Build more of the former.

Two diseases account for nearly every finding:
1. **Parallel copies drifting** — the same teaching text hand-maintained in 2+ homes (inline
   strings vs registry templates vs hover fields vs REF docs vs rule files) with no test
   forcing agreement. (A1, A3–A6, D1–D3, F1, F3.)
2. **Internal vocabulary leaking** — jargon/paths/symbols from the contributor world escaping
   into user-facing text where audit coverage has a hole. (B1–B4, E1–E5.)

### H1. Assessment of the teaching rule itself

The rule content is **good — keep it**. The WHAT/WHAT-INSTEAD/WHY triple, the
contextual-not-generic WHY requirement, and the HS-grad-who-knows-JS audience test are
well-designed and, where followed, produce the best diagnostics in the codebase (§G). Its two
real weaknesses:

- **It is smeared across six documents** — `REF-golden-rules.md` (R11/R12), `REF-teaching-mission.md`,
  `.claude/rules/inference.md`, `vocabulary.md`, `spec-writing.md`, `dot-postfix.md`. Any
  writer of a new diagnostic must hold all six in context simultaneously; nobody does, so
  slips are structural. Even `inference.md` itself carries banned-register jargon
  ("trivially copyable") — the rule authors slip too, which proves the point.
- **It enforces form, not quality.** The empty-check cannot catch a circular WHY (C3), a
  rule-restatement in the WHAT-INSTEAD slot (C2), or a stderr dump in the WHY (C1). Quality
  is judgment work and needs a review lens (H2.7), not a stronger assert.

### H2. The prevention catalog (each item: what, mechanism, kills which class)

1. **One home, threaded — resolve A1 first.** Teaching text lives in the registry ONLY;
   firing sites render through a helper. The pattern already exists and works:
   `lint_rule_diagnostic_parts` (`ynz-registry/src/lib.rs:123`) — and the three lint rules it
   powers were the cleanest teaching text in the entire audit. Either extend that exact
   mechanism to diagnostics (firing sites call `diagnostic_template_lookup` + placeholder
   vars) or delete the `[[diagnostic_template]]` table and record inline-strings-as-authority.
   When there is one copy, drift stops being a discipline problem and becomes impossible.
   *Kills: A1, D3, and the future recurrence of D1-class duplication.*
2. **Compile the spec — doctest the language.** A test that extracts every ` ```ynz ` fenced
   block from `docs/reference/REF-*.md` and runs it through the real compiler; blocks
   demonstrating errors carry an annotation (e.g. `// EXPECT-ERROR: <key phrase>`) and assert
   the diagnostic fires. Rust-doctests, but for the Yinz spec. The single biggest lever nobody
   has pulled. *Kills: A5 (`maybe T`) and every future docs-teach-syntax-that-doesn't-compile
   drift.*
3. **Widen `jargon_audit.rs` coverage to every registry field that renders in an editor**:
   `[[keyword]]` hover_what/hover_what_instead/hover_why, `[[muted_hint_domain]]` hover_* +
   description + example_hint_rendered, `[[lint_rule]]` what/what_instead/why templates,
   `[[banned_declaration_keyword]]` what_instead/why. The audit mechanism is good; its
   coverage map has holes — B1's two live `infer` instances sit exactly in one. *Kills: B1
   class permanently.*
4. **Parity tests wherever two surfaces must agree.** The template already exists in-repo:
   the `DEFAULT_CHANNEL_CAPACITY` parity test (`ynz-lsp/tests/inlay_hint.rs`, cited at
   features.toml:2247–2249) "breaks loudly if this entry and the constant ever drift." That
   pattern exists for exactly ONE value — extend it: registry `example_hint_rendered` vs the
   label format each `inlay_hint.rs` domain actually emits (F1/F3), registry
   placement_category vs the handler's category table (F3), and — until item 1 lands — each
   live `[[diagnostic_template]]` vs its inline twin. *Kills: F1, F3, slows A1-class drift.*
5. **Kinds, not prose, for cross-surface coupling.** Any downstream consumer (LSP hint firing,
   code actions, tooling) keys off a `DiagnosticKind` variant — never off message text.
   Replace `d.what.starts_with("Copying ")` (`inlay_hint.rs:300`) with a
   `DiagnosticKind::BackgroundLargeCopy`. A rewording must never be able to silently kill a
   teaching surface. *Kills: F2 class.*
6. **Bouncer/graveyard grep-gates** (mechanical, diff-detectable — fits the existing
   `.claude/graveyard.md` system):
   - `design/[a-z-]+\.md` or `IMP-[a-z-]+\.md` inside a string literal in `crates/**/*.rs`
     diagnostic text (B3);
   - `Tokio|ynz_rt_init|alloca|vtable` in diagnostic/hover strings (B2);
   - milestone tags `v0\.\d+-M\d?[a-z]?` or bare `M\d` in user-facing strings (B4);
   - snake_case in *suggested identifiers* inside diagnostic examples — `` `let [a-z]+_[a-z] ``
     shape (E1);
   - `\.copy\b` without `()` in teaching examples (E3).
7. **Consolidate the rule + add a review lens.** Collapse the six-document smear into ONE
   checklist rule (`.claude/rules/teaching-surfaces.md`): the three-slot test (WHAT states the
   problem; WHAT-INSTEAD is copyable/actionable — "can the user DO this?"; WHY is contextual,
   non-circular, cites no internals), the audience test (18-year-old JS dev, no Googling), the
   banned-vocab pointer, naming conventions inside examples (camelCase, `.copy()` parens, no
   SCREAMING_SNAKE unless naming.md ratifies it), and the no-internal-paths/no-milestone-tags
   rules. Then wire it into the existing plan-built review fleet: whenever a diff touches
   `Diagnostic::`, `registry/features.toml` teaching fields, or `inlay_hint`/`hover` code, a
   reviewer loads THIS rule and grades the new strings against it. This is the judgment tier
   (H1's second weakness) that no mechanical check can cover — today no reviewer charter
   explicitly owns it, which is why plan-built work still slips.
8. **Make the right path the easy path for new diagnostics.** Slips happen when doing it right
   requires remembering six documents. Shared helper constructors for recurring lesson
   families (the kernel-mode rejection family D1 is the worked example — one
   `kernel_mode_rejection(feature)` helper, like the existing
   `wait_on_non_may_block_warning` and `reject_share_param_mutation` precedents) mean the
   correct, current wording is what you get by default and drift requires *effort*.

### H3. Fix-now vs plan (disposition)

- **Plan-worthy (structural — the actual cure)**: H2.1 (A1 decision + threading), H2.2
  (spec doctests), H2.4 (parity tests), H2.7 (rule consolidation + reviewer lens), plus the
  bulk text-fix sweep of §§A–F riding on those rails. Multi-phase; touches registry, tests,
  `.claude/rules`, and reviewer charters; front-loaded by one design decision (A1). This is a
  standalone plan, and per plan-invariants it must itself carry a `### Teaching` subsection —
  pleasingly recursive.
- **Cheap and independent (could ship as an early phase or a small pre-plan fix batch, but
  should still ride the plan for review coverage)**: A4 (`\0` list), B1 rewords (two strings),
  H2.3 (audit widening), H2.6 (Bouncer regexes), the `BackgroundLargeStructCopy` template text
  correction (even if A1 later deletes the table, the lying text shouldn't sit in a file
  cited as SSOT).
- **Do NOT hand-fix ahead of the A1 decision**: any wording sweep of the inline strings
  (B2/B3/B4/E*) — sweeping text that might be about to move homes is double work.
