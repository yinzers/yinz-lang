# Conflict Audit Report — Round 2 Design Changes

## Methodology

Read all six changed/new files in full, then traced every cross-reference they cite. For each change, read all design files that share the topic area: `design/collections.md`, `design/linting.md`, `design/iterables.md`, `design/concurrency.md`, `design/future/concurrency.md`, `design/future/panic-safety.md`, `design/future/supervisor.md`, `design/future/auto-soa.md`, `design/ownership.md`, `design/ide-hints.md`, `design/mvp-scope.md`, `design/deferrals.md`, `design/versioning.md`, `design/type-system.md`, `design/decisions.md`, `design/golden-rules.md`, `design/compiler-errors.md`, `design/stdlib/filesystem.md`, `design/stdlib/strings.md`, `design/stdlib/concurrency.md`, `design/stdlib/data.md`, `design/stdlib/cli.md`, `design/stdlib/overview.md`, `spec/collections.md`, `spec/strings.md`, `.claude/rules/inference.md`, `.claude/rules/stdlib-design.md`, `.claude/rules/vocabulary.md`, `.claude/rules/naming.md`. Checked for behavioral contradictions, vocabulary slips, golden rule violations, cross-reference errors, and milestone timing issues. Did NOT audit files outside the topic area of the six changes.

---

## Conflicts Found

### Conflict 1 — `map<K,V>.adversarial` Uses Method-Chaining-Style Dot Notation on a Type Name (blocker)

**Affected files**: `design/collections.md` (new section "Four-Tier Hashing"), `design/collections.md` (existing section "No Method Chaining")

**The contradiction**:

`design/collections.md` existing section "No Method Chaining" (lines 19-24) states: "Each operation gets its own line with a named variable." Golden Rule 7 bans method chaining entirely.

The new "Four-Tier Hashing" section (line 131) introduces the syntax `map<K,V>.adversarial` as a type modifier — a dot notation appended to the type itself at the declaration site. This isn't method chaining at runtime, but the dot-on-type syntax is novel and undocumented anywhere in the language. No other type uses a dot modifier on the type name (e.g., there is no `array<T>.immutable` or `fixed<T>.typed`). The section also writes `map<K,V>.unordered` in the insertion-order section (line 164).

More critically, these are written as type-position syntax (`map<K,V>.adversarial`) but the language has no defined syntax for dot modifiers on type names — dot modifiers on VALUES are the ownership system (`.share`, `.lend`). The type-position use is not defined in `design/type-system.md`, `spec/collections.md`, or anywhere else.

Additionally, the linting section in `design/linting.md` line 98 references a rule `use-type-for-static-keys` that "suggest[s] a `type`" — but should suggest a `shape` (this is a pre-existing vocabulary slip that intersects with the new content).

**Severity**: blocker

**Recommended resolution**: The `map<K,V>.adversarial` syntax needs to be defined as actual Yinz syntax before being locked. Options: (a) `adversarial map<K,V>` as a keyword-prefix form (like `background`), (b) a construction-time flag `map<K,V>(security: .adversarial)`, (c) defer naming to M4 implementation (which the section itself says is "TBD during M4 implementation" — so don't introduce the placeholder syntax in a lock document). At minimum, add a note that the dot-on-type-name syntax requires a language design decision before M4.

---

### Conflict 2 — `design/future/panic-safety.md` Cites `design/stdlib-design.md` Rule 3 for a Corollary That Rule 3 Does Not Make (major)

**Affected files**: `design/future/panic-safety.md` (new "Background Task Error Observability" section, line 144), `.claude/rules/stdlib-design.md` (Rule 3)

**The contradiction**:

`panic-safety.md` line 144 says: "the runtime emits a structured event to stderr by default. JSON format when stdout is non-TTY (per `design/stdlib-design.md` Rule 3 corollary for log defaults)."

`.claude/rules/stdlib-design.md` Rule 3 is "No Silent Configuration Defaults That Vary By Platform." It is about encoding defaults (always UTF-8), time zone (always explicit), locale (always explicit), and path separators. It says nothing about log format, JSON vs text output, TTY detection, or stderr defaults. There is no "Rule 3 corollary for log defaults" anywhere in that file.

The actual log format rule — if it exists — would live in `design/stdlib/overview.md` or a log-specific design doc, neither of which mentions TTY-sensitive JSON formatting. The v0.11 `log` module is not yet designed (only "ship with this version" is decided). There is no existing decision this citation can correctly point to.

This is a cross-reference that cites a real file for a decision that file does not contain.

**Severity**: major

**Recommended resolution**: Either (a) remove the citation and state the JSON-on-non-TTY behavior is a new decision being made in this section (requires locking it here explicitly), or (b) move it to `design/open-questions.md` since the log module is not yet designed, or (c) cite it as "TBD when v0.11 log module is designed" and remove the false attribution to stdlib-design.md Rule 3.

---

### Conflict 3 — `process.setBackgroundErrorHandler()` Is Introduced Without a `process` Module Design, But `process` Is v0.8 (major)

**Affected files**: `design/future/panic-safety.md` (new section, lines 148-155), `design/mvp-scope.md` (v0.8 scope), `design/stdlib/overview.md` (v0.8 entry)

**The contradiction**:

The new section introduces `process.setBackgroundErrorHandler(handler)` as an API callable by users. The `process` module is v0.8. `design/mvp-scope.md` v0.8 scope lists `process` as shipping with: `process.exit(code)`, `process.pid`, `process.parentPid`, `process.startedAt`, `process.uptime`, `process.args`, `process.workingDirectory`, `process.onShutdown(handler)`, `process.isRunning()`. `setBackgroundErrorHandler` is not on this list.

More importantly, `panic-safety.md` is a v0.2 design doc. It introduces an API (`process.setBackgroundErrorHandler`) that belongs to a v0.8 module, in a v0.2 document, without noting the milestone dependency. If v0.2 ships the background error observability system, the reconfiguration API cannot exist until v0.8.

The section also introduces `process.diagnostics()` (line 155) — also not in the v0.8 scope list.

**Severity**: major

**Recommended resolution**: Mark `process.setBackgroundErrorHandler()` and `process.diagnostics()` as "v0.8 additions to the `process` module" with a cross-reference note. The v0.2 behavior (default stderr logging with no reconfiguration) is the initial behavior; the reconfiguration hook lands with the `process` module at v0.8. Add these to the v0.8 scope in `design/mvp-scope.md` when that milestone is designed.

---

### Conflict 4 — `design/future/panic-safety.md` New Section's `task.onError` Conflicts With the Existing `onPanic` Pattern (minor)

**Affected files**: `design/future/panic-safety.md` (new section lines 172-175), `design/future/panic-safety.md` (existing "Supervisor pattern" section lines 110-116)

**The contradiction**:

The new section introduces `task.onError = (e) => ...` (line 174) as the way to attach an error observer for the non-panic errors case. The existing supervisor section (lines 110-115) shows `task.onPanic = (e: Panic) => { ... }` for panics.

The two are parallel: `task.onPanic` and `task.onError` are both field-assignment patterns on the task handle, which is consistent step-by-step style (not chaining). This is internally consistent within `panic-safety.md`.

However, `design/future/supervisor.md` line 83 documents the callback as `.onPanic(callback)` — a METHOD call, not a field assignment (`task.onPanic = handler` vs `task.onPanic(handler)`). These are different syntaxes for the same concept. The new panic-safety section uses the field-assignment form; supervisor.md uses the method-call form.

**Severity**: minor

**Recommended resolution**: Decide on one syntax and apply it consistently across both files. Field-assignment (`task.onPanic = handler`) aligns better with Yinz's step-by-step principle since it reads as "set this property on this handle." Method-call (`handle.onPanic(handler)`) creates more API surface. Pick one.

---

### Conflict 5 — `design/future/auto-soa.md` IDE Hint Claims Click-to-Make-Explicit for a Transform That Has No Explicit Syntax (minor)

**Affected files**: `design/future/auto-soa.md` (lines 77-78, 140-144), `.claude/rules/inference.md` (The Rule, line 24)

**The contradiction**:

`.claude/rules/inference.md` states: "The muted hint completes to syntactically-valid Yinz the developer COULD have typed. Click-to-make-explicit must produce real Yinz syntax."

`design/ide-hints.md` line 56 states: "The muted text MUST complete to syntactically-valid Yinz the developer COULD have typed."

`design/future/auto-soa.md` shows a muted hint: `// SoA layout: hot loop accesses only x — saved 24 bytes/entry on cache reads`. The hover tooltip (lines 140-144) explicitly says: "**WHAT INSTEAD**: There's no syntax to opt OUT in the array declaration."

So the hint is applied but cannot be clicked to make explicit — there is no syntax the developer could type to produce SoA. The hover tooltip directly contradicts the inference rule's requirement that muted hints complete to typeable Yinz.

This is a direct conflict with the inference rule's load-bearing requirement.

**Severity**: minor (the SoA hint is a v0.3+ concern, but the design should be consistent)

**Recommended resolution**: Either (a) change the IDE surface for auto-SoA from a muted hint (which must be click-to-make-explicit) to a different visual treatment — perhaps a margin annotation or a special "compiler optimization" indicator that doesn't promise typeability, or (b) define an opt-in syntax for SoA (`soa array<Player>` or similar) so the hint CAN be made explicit. Option (a) is simpler; option (b) aligns with Rule 4 (compiler does the hard work) and user empowerment. Note the `array→fixed` promotion in `design/collections.md` explicitly handles this correctly: the lint suggestion suggests `fixed<T>` which IS typeable, while the auto-codegen is separate from the hint.

---

### Conflict 6 — `design/collections.md` Auto-Promotion Section's Hover Tooltip Says "If you keep the `array<T>` form, no big deal" vs. Lint Rule Behavior (minor)

**Affected files**: `design/collections.md` (auto-promotion section, lines 189-194), `design/linting.md` (`prefer-fixed-when-immutable` rule description)

**The contradiction**:

The hover tooltip in `design/collections.md` line 193 says: "If you keep the `array<T>` form, no big deal — the compiler emits the optimized layout for you."

`design/linting.md` classifies `prefer-fixed-when-immutable` as a Tier 3 SUGGESTION (line 99). Tier 3 suggestions are "IDE-visible hints, lowest urgency" — consistent with "no big deal."

This is actually consistent (both say it's optional). But the tooltip text says "no big deal" which undercuts the WHY field's purpose of teaching. If it's "no big deal," the developer has no reason to change the code — defeating the lint suggestion. The teaching mission says every WHY should be specific and motivating. "No big deal" is the opposite of that.

This is a soft conflict with Golden Rule 11 (compiler is a teacher) rather than a hard behavioral contradiction.

**Severity**: minor

**Recommended resolution**: Rewrite the "no big deal" sentence to something that completes the teaching: "If you keep the `array<T>` form, the codegen is identical — but explicit `fixed<T>` signals to reviewers that growth is intentionally prohibited, and prevents a future `.add()` from silently switching the codegen back to heap allocation."

---

## Vocabulary Slips

### Slip 1 — `design/collections.md` New Section Uses "tier" for Map Hash Strategy Tiers Without Defining the Term

The new "Four-Tier Hashing" section uses "tier" as an internal technical term for hash strategy levels. This is an internal design doc (fine to use), but the IDE teaching surface description says "the IDE shows which tier was used as a muted hint" (line 126). This surfaces the word "tier" to users. `design/compiler-errors.md` jargon ban-list does not include "tier," so this is not a hard ban, but it is unexplained jargon in a user-facing surface. Minor.

### Slip 2 — `design/linting.md` Line 98 Says `use-type-for-static-keys` Suggests a `type`, Not a `shape`

`design/linting.md` line 98: `use-type-for-static-keys | map<string, V> literal with all-string-literal keys — suggest a type`

The Yinz term is `shape`, not `type`. The rule name itself (`use-type-for-static-keys`) uses the banned declaration keyword. This is a pre-existing slip, but the new `design/collections.md` content cross-references `design/linting.md` and neither flags this. The correct rule name should be `use-shape-for-static-keys` and the description should say "suggest a `shape`."

Note: `spec/collections.md` line 266 and line 252 also use `type Scores { ... }` and suggest "`type` instead" — these are pre-existing slips that the new content in `design/collections.md` does not fix or flag.

### Slip 3 — `design/future/panic-safety.md` New Section Correctly Avoids Vocab Slips

No vocabulary slips in the new "Background Task Error Observability" section. Uses `errors`, `background`, `onPanic`, `onError` correctly. One potential issue: `BackgroundErrorEvent` (line 149) is a PascalCase type name — correct per Rule 13. Clean.

### Slip 4 — `design/strings.md` Cross-References `design/stdlib/strings.md` for Future Locale Content (line 61)

`design/strings.md` line 61 says: "open question for the stdlib design (see `design/stdlib/strings.md` when written)." The file exists at `/workspaces/ynz/design/stdlib/strings.md` but contains only method listings (`.toUpper()`, `.split()`, etc.) — not locale design. The parenthetical "when written" acknowledges this, but the cross-reference is partially misleading since the file exists but does not cover the promised content. Minor.

---

## Golden Rule Violations

### Violation 1 — `design/future/supervisor.md` (Pre-Existing, Intersects With New Content) Uses Explicit Method Chaining

`design/future/supervisor.md` lines 27-33 document explicit method-chaining syntax for the supervisor API:

```
supervise.alwaysRestart(processOrders)
  .withBackoff(initial: 100.ms, max: 30.seconds, multiplier: 2)
```

The file itself acknowledges this on line 44: "The API is fluent chainable (this is one of the rare places fluent makes sense)."

This is a pre-existing Golden Rule 7 violation — not introduced by the recent changes. However, the new `design/future/panic-safety.md` section adds supervisor usage examples (lines 178-179) that use `supervise.alwaysRestart(processOrder, onError: ...)` without chaining, which is consistent. The new content does NOT introduce new chaining violations; it navigates around the pre-existing supervisor chaining by using the non-chained form.

**Action**: Not caused by the round-2 changes. Pre-existing issue for a separate fix.

### Violation 2 — `design/future/auto-soa.md` "No Opt-Out From Shape Declaration" Claim vs. `design/type-system.md`

`design/future/auto-soa.md` tooltip text (line 143) says: "There's no syntax to opt OUT in the array declaration." This means a user cannot tell the compiler "do NOT apply SoA to this array." The only escape valve mentioned is FFI (v2+).

This conflicts with Rule 4 (compiler does the hard work) — not as a violation, actually supporting it. But it conflicts with the user's ability to have a predictable memory layout for non-FFI purposes (testing, debugging, embedded without FFI). The design says SoA is invisible (no user control) except through FFI, but embedded/kernel-mode users (v0.3 no-runtime mode is also v0.3) may need layout control without FFI.

`design/future/no-runtime-mode.md` (which targets v0.3, same milestone as auto-SoA) likely has opinions about compiler transforms in kernel mode. The auto-SoA doc does NOT cross-reference `design/future/no-runtime-mode.md`. This is a missing cross-reference, and a potential conflict: auto-SoA may need to be disabled or configurable in `--kernel` mode.

**Severity**: minor for the audit, but should be tracked as an open question for the v0.3 plan.

---

## Cross-Reference Errors

### XRef Error 1 — `design/future/panic-safety.md` Cites `design/stdlib-design.md` for Non-Existent "Rule 3 Corollary"

Already documented as Conflict 2 above. The citation is false — Rule 3 does not contain a corollary about log defaults.

**File**: `design/future/panic-safety.md` line 144
**Fix**: Remove or replace the citation.

### XRef Error 2 — `design/future/auto-soa.md` Cross-References `design/concurrency.md` for Auto-Parallelization Being "Same v0.3 Milestone Family" — Correct But Incomplete

`design/future/auto-soa.md` line 180 cites `design/concurrency.md` for "auto-parallelization — same v0.3 milestone family." `design/concurrency.md` describes auto-parallelization. `design/deferrals.md` lines 67-74 also documents this. The cross-reference is correct.

However, auto-SoA is NOT mentioned in `design/deferrals.md` even though it's a v0.3+ deferred feature. The deferral doc lists: auto-parallelization, test parallelization, public registry, FFI, GPU, ML, Markets, operator overloading, custom iterables, self-hosted compiler, deprecation marking, ynz doc/repl, lint customization. Auto-SoA is absent.

This is a documentation completeness issue: the deferral ledger should have an entry for auto-SoA per its own instructions ("When a feature is decided to defer: Add an entry to this file").

**Severity**: minor (no behavioral contradiction, just a missing ledger entry)

**Fix**: Add a deferral entry for auto-SoA in `design/deferrals.md` pointing to `design/future/auto-soa.md`.

### XRef Error 3 — `.claude/rules/stdlib-design.md` Cross-References `design/collections.md` for "Rule 6 — codegen serialization referenced for JSON v0.9" (line 132)

`.claude/rules/stdlib-design.md` line 132 says "design/collections.md (Rule 6 — codegen serialization referenced for JSON v0.9)." Reading `design/collections.md` in full: there is no mention of JSON, v0.9, or codegen serialization. Collections.md covers `fixed<T>`, `array<T>`, `map<K,V>`, brackets, and string methods. The cross-reference points to the wrong file — the JSON/codegen serialization discussion belongs in `design/stdlib/data.md` or a future `design/stdlib/json.md`.

**Severity**: minor

**Fix**: Update stdlib-design.md line 132 to cite `design/stdlib/data.md` (or note "see future `design/stdlib/json.md` when written") instead of `design/collections.md`.

---

## Milestone Timing Issues

### Timing Issue 1 — `design/strings.md` Claims SIMD Validation Is "a v0.1 Implementation Goal" (major)

**File**: `design/strings.md` lines 38-39: "This is a v0.1 implementation goal, not a future enhancement."

`design/mvp-scope.md` v0.1 scope lists what ships: variables, functions, types, control flow, strings (with interpolation, indexing), ownership, etc. String SIMD is not listed. More importantly, `design/deferrals.md` and `design/mvp-scope.md` make no claim that LLVM-level SIMD intrinsics for UTF-8 validation are a v0.1 concern. The compiler in v0.1 is already handling lexer+parser+typeck+codegen — adding a custom SIMD UTF-8 runtime for string validation is significant scope.

The statement "v0.1 implementation goal" directly contradicts `design/mvp-scope.md`'s philosophy: "Programs you can write in v0.1: hello world, math demos, pure-computation programs" — the string runtime's performance characteristics are not load-bearing for any of those programs.

**Severity**: major

**Recommended resolution**: Change to "v0.1 DESIGN goal — the SIMD implementation target is locked now so the v0.1 string runtime is written against this goal from the start, not retrofitted later." This preserves the intent (don't ship scalar and rewrite) while being honest that the v0.1 milestone scope doesn't include a polished SIMD runtime. Alternatively, explicitly add "UTF-8 SIMD validation" to the v0.1 language features list in `design/mvp-scope.md` if it IS truly required for v0.1.

### Timing Issue 2 — `design/future/panic-safety.md` Mentions `process.setBackgroundErrorHandler` Which Is v0.8 in a v0.2 Doc

Already documented as Conflict 3. The timing conflict is: v0.2 document introduces v0.8 API without cross-version flagging.

### Timing Issue 3 — `design/future/auto-soa.md` Missing From `design/deferrals.md`

Already documented as XRef Error 2. The deferral ledger is incomplete for this v0.3+ feature.

---

## Clean Items

The following checks found no conflicts:

**Change #1 (collections.md):**
- Swiss Tables choice does NOT conflict with `design/compiler-language.md` or `design/compiler.md` — those files describe Rust+Salsa+inkwell but make no claims about runtime data structures, which are a separate concern.
- 1.5× growth factor does NOT conflict with anything in `design/numeric-types.md` — numeric types cover `number`, `float`, `int` precision; they make no claims about runtime growth factors.
- Insertion-order iteration does NOT conflict with `design/iterables.md` — `Iterable<T>` contract defines `next()` semantics but imposes no iteration-order requirement on maps. `map<K,V>.entries()` is listed as `Iterable<Entry<K,V>>` with no order claim; locking insertion order here is compatible.
- The four-tier hashing strategy does NOT conflict with `design/concurrency.md` cross-thread maps — concurrency.md discusses ownership semantics (`.share`/`.lend`) but makes no claims about map implementation.
- The hybrid auto-promotion model (codegen + lint suggestion) is consistent with the `mutable-when-const-suffices` Tier 2 warning described in `design/linting.md` line 82. The linting.md description (`prefer-fixed-when-immutable`) correctly describes the hybrid model and cross-references collections.md. The fix Patrick caught in Round 1 (removing the muted-hint row from inference.md) IS correctly reflected in the current state of `.claude/rules/inference.md` — the Note at line 46 explicitly explains why auto-promotion is NOT a muted hint.
- `.claude/rules/inference.md` final state is internally consistent — the "Note" correctly describes the lint-suggestion vs muted-hint distinction, and no orphan references to the removed `array→fixed` row remain.

**Change #2 (inference.md):**
- The Note added at lines 46-47 does NOT contradict `design/ide-hints.md` — ide-hints.md's "What gets hinted" table lists the same domains as inference.md's table, and neither includes `array→fixed` promotion (correctly, since they use the lint-suggestion model, not muted hints).
- The final state of inference.md is internally consistent with the rest of the file.

**Change #3 (strings.md):**
- UTF-8 internal encoding does NOT conflict with `spec/strings.md` — spec/strings.md describes the user-facing API (indexing, methods) without specifying internal encoding. The design/strings.md decision is underneath the API.
- UTF-8 internal encoding does NOT conflict with `design/collections.md` "String Methods" section — that section documents `.byteAt()`, `.graphemeAt()`, and companions, all of which presuppose UTF-8 storage (`.byteCount()` only makes sense for a byte-oriented encoding). Fully consistent.
- `design/stdlib/filesystem.md` does NOT specify an encoding default for `file.read()` — it just shows the API signature `file.read("data.txt") -> string errors`. The `design/strings.md` decision (UTF-8 default for file I/O) therefore does not conflict with filesystem.md; it fills in a detail that was unspecified.
- `design/stdlib/strings.md` covers only method listings (`.toUpper()`, `.split()`, etc.) — no encoding decisions there to conflict with.

**Change #4 (stdlib-design.md):**
- Rule 1 (pure-named methods) — checked `design/stdlib/` files. `design/stdlib/filesystem.md` shows `file.size("data.txt") -> int errors` and `file.lastModified("data.txt") -> Date errors` — these do disk I/O but they're marked `errors`, so the name itself (`.size`, `.lastModified`) could trigger Rule 1 concern. However, the file-system operations are universally understood to do I/O, and the `errors` keyword surfaces the failure mode at every call site. Rule 1's examples (`file.length()` doing I/O) map to `file.size()` — this is a pre-existing design choice in filesystem.md that stdlib-design.md Rule 1 would flag. This is not introduced by the new stdlib-design.md (which merely documents the rule), but it does mean the existing filesystem.md API has a potential Rule 1 conflict that the new rules file surfaces. Flagging as pre-existing, not a conflict introduced by round-2 changes.
- Rule 2 (no parallel APIs) — consistent with `design/versioning.md` which establishes the macro policy of no backwards-compat shims.
- Rule 4 (bounded queues) — does not conflict with `design/future/concurrency.md` or `design/stdlib/concurrency.md`. The stdlib concurrency doc is a stub; the future concurrency doc discusses `background` tasks and scheduling but doesn't specify channel/queue API design. Rule 4 fills a gap, not a conflict.
- Rule 5 (receiver-first) — consistent with `.claude/rules/naming.md` and the dot-first design throughout the language.
- Rule 6 (codegen serialization) — `design/stdlib/data.md` shows `json.parseAs<Config>(content)` which implies typed deserialization. This is consistent with compiler-generated specialized serializers (the generics parameter gives the compiler the type at call time). No conflict.

**Change #5 (panic-safety.md new section):**
- The step-by-step syntax in the new section correctly avoids method chaining. `task.onPanic = ...` and `task.onError = ...` on separate lines are field-assignment statements, not chained method calls. Rule 7 is satisfied.
- The "never silent" contract does NOT conflict with `design/concurrency.md`'s `background` description. The concurrency.md "background" section discusses fire-and-forget vs long-running patterns and ownership semantics, but makes no claims about error observability. The new panic-safety section fills a gap in the observability story.

**Change #6 (auto-soa.md):**
- Does NOT conflict with `design/concurrency.md` auto-parallelization timing. `design/concurrency.md` says "auto-parallelization optimization" is deferred to v0.3 (confirmed in `design/deferrals.md`). `design/future/auto-soa.md` says "Locked (commitment), v0.3+ implementation." Both are v0.3+ features in the same milestone family — no contradiction.
- Does NOT conflict with `design/ownership.md`'s aliasing claims. `design/ownership.md` establishes that `noalias` is emitted via the ownership system. auto-soa.md explicitly cites this as the prerequisite enabling the transform (line 59: "the ownership system already proves field aliasing statically"). Consistent.
- Does NOT conflict with `design/collections.md`'s `array<T>` API guarantees. The user-facing API (`arr[i].health`) is unchanged by SoA; only the memory layout differs. The spec-level contract is preserved.
- The milestone statement "v0.3 is the auto-parallelization milestone" (line 116) is confirmed by `design/mvp-scope.md` v0.3 entry. Consistent.

---

## Summary

- **6 conflicts found**: 1 blocker, 3 major, 2 minor
- **4 vocabulary slips**: 2 pre-existing (linting.md `use-type-for-static-keys`; spec/collections.md `type Scores`), 1 incomplete cross-reference (stdlib/strings.md), 1 minor unexplained-jargon-in-user-surface issue
- **2 Golden Rule violations**: 1 pre-existing (supervisor.md chaining), 1 new (auto-soa.md muted-hint cannot be made explicit)
- **3 cross-reference errors**: 1 false citation (panic-safety.md → stdlib-design.md Rule 3), 1 missing deferral ledger entry (auto-soa.md), 1 wrong-file citation (stdlib-design.md → collections.md for JSON codegen)
- **3 milestone timing issues**: 1 major (strings.md SIMD as v0.1 "implementation goal"), 1 major (process.setBackgroundErrorHandler is v0.8 in a v0.2 doc), 1 minor (auto-soa.md missing from deferrals.md)
