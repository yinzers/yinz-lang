# Design-Doc Audit Report

## Methodology

Read the full design corpus: all 13 golden rules in `CLAUDE.md` and `design/golden-rules.md`; all six `.claude/rules/` files; all 35 files under `design/` (including `design/future/` and `design/stdlib/`); all 28 `spec/*.md` files; plus all five lockin source files, `verification.md`, and `highlight-reel-opus.md`. For each Yinz-status flag (at-risk / unaddressed / partially-addressed) in the lockin files and each of the 18 proposed fixes in `verification.md`, I opened the specific Yinz design file(s) cited or relevant to verify whether a position is already locked or genuinely open. Where a finding's "Yinz status" already said "not applicable" or "already solved," I confirmed it was correct and skipped them. The totals below count only findings where the lockin file's status was wrong or where a proposed fix conflicts with the design.

---

## Findings That Are ACTUALLY Already-Solved

### 1. lockin-type-and-memory.md Finding #3 — Java UTF-16 strings
- **Currently tagged**: NOT YET DECIDED
- **Should be tagged**: already-solved
- **Where Yinz solves it**: `design/collections.md` — the `string` type's indexing API uses `.byteAt()`, `.graphemeAt()`, and `.get(n)` by code point, all of which presuppose UTF-8 internal storage. `spec/collections.md` and the spec `spec/strings.md` (see `spec/overview.md`) document `.byteCount()` vs `.count()` — a distinction that only makes sense if the internal encoding is UTF-8. Additionally, `design/stdlib/strings.md` mentions "Encoding/decoding (UTF-8, ASCII, Base64)" as expansion candidates, indicating UTF-8 is the baseline. The `lockin-cpu-bigo.md` Yinz status column for Finding #16 ("UTF-8 string traversal") says "Yinz strings are UTF-8 (implied by `.byteAt()`, `.graphemeAt()` API)." The type-and-memory file's own cross-file says NOT YET DECIDED but the collections and cpu-bigo files have already recorded it as implied-solved.
- **Recommended action**: Re-tag lockin-type-and-memory.md Finding #3 as already-solved. Cross-reference `design/collections.md` string-indexing section. Also lock UTF-8 explicitly in a `design/strings.md` file (which does not yet exist) so the commitment is unambiguous rather than inferred from the indexing API.

---

### 2. lockin-concurrency.md Finding #3 — Python GIL
- **Currently tagged**: "Not applicable directly" (status is actually "at risk" in the sense that it mentions the ownership system)
- **Should be tagged**: already-solved (and the file does say N/A — calling this out because verification.md doesn't separately rate it, but the file correctly self-identifies as N/A)
- **Where Yinz solves it**: `design/concurrency.md` "Runtime Implementation" section: "Thread pool sized to CPU cores. I/O operations use the OS event system (epoll/kqueue/IOCP)." `design/ownership.md`: the `lend` constraint enforces single-writer at compile time. The GIL problem is architecturally impossible in compiled-to-LLVM Yinz.
- **Recommended action**: No change needed — the file already says N/A.

---

### 3. lockin-concurrency.md Finding #4 — Python asyncio CancelledError base class
- **Currently tagged**: Not applicable
- **Should be tagged**: already-solved (correctly marked N/A — auditing confirms)
- **Where Yinz solves it**: `design/future/panic-safety.md` (referenced) — Yinz has no exception hierarchy; panics propagate via ownership drop semantics, not exception catching. `design/errors.md` documents that there is no try/catch in Yinz at all.
- **Recommended action**: Already correct. No change.

---

### 4. lockin-concurrency.md Finding #6 — Java Memory Model DCL bug
- **Currently tagged**: Not applicable
- **Should be tagged**: already-solved (correctly N/A)
- **Where Yinz solves it**: `design/ownership.md`: "only one `lend` holder can modify a value at a time" + `noalias` LLVM attribute from the ownership system.
- **Recommended action**: Already correct.

---

### 5. lockin-concurrency.md Finding #7 — C++ memory_order_consume
- **Currently tagged**: Not applicable
- **Should be tagged**: already-solved (correctly N/A)
- **Where Yinz solves it**: `design/ownership.md` "LLVM contract" section: emits `readonly` and `noalias` attributes, getting the aliasing information without exposing memory ordering semantics to users.
- **Recommended action**: Already correct.

---

### 6. lockin-concurrency.md Finding #8 — Java ThreadLocal breaks with virtual threads
- **Currently tagged**: Not applicable
- **Should be tagged**: already-solved (correctly N/A)
- **Where Yinz solves it**: `design/concurrency.md` "Ownership with Background Tasks" table: values passed to `background` tasks are either `.give`'d or `.copy`'d — never shared via implicit global state. ThreadLocal-class bugs are architecturally impossible.
- **Recommended action**: Already correct.

---

### 7. lockin-stdlib-and-syntax.md Finding #12 — PHP argument order
- **Currently tagged**: at-risk
- **Should be tagged**: already-solved
- **Where Yinz solves it**: `design/collections.md` (and the broader design) is entirely dot-method-first. `value.method(args)` is the universal calling convention — the receiver is always the thing being operated on, always comes first by syntax. PHP's haystack/needle problem is a procedural-function-argument-order problem. It literally cannot occur in Yinz because there are no free functions in stdlib; everything is `receiver.method(args)`. The verification.md Proposed Yinz fix for Highlight #10 correctly identifies this: "this is already enforced by Yinz's existing dot-method-first design."
- **Recommended action**: Re-tag lockin-stdlib-and-syntax.md Finding #12 as already-solved. The "remaining decision" the fix mentions (convention for free-function argument order) is moot because Yinz has no stdlib free functions.

---

### 8. lockin-stdlib-and-syntax.md Finding #2 — JavaScript Date 0-indexed months
- **Currently tagged**: Already solved (the file correctly says "Yinz status: Already solved. `design/stdlib/dates.md` explicitly states 'months are NOT zero-indexed. May = 5, not 4.'")
- **Confirmed**: `design/stdlib/dates.md` line: "Key decision: months are NOT zero-indexed. May = 5, not 4."
- **Recommended action**: Correct. No change.

---

### 9. lockin-stdlib-and-syntax.md Finding #3 — Go time.Format magic reference date
- **Currently tagged**: Already solved (file says "Already solved. `design/stdlib/dates.md` uses named readable methods and a format-string approach using readable tokens.")
- **Confirmed**: `design/stdlib/dates.md` shows `now.format("MMMM D, YYYY")`, `now.format("HH:mm:ss")` — readable tokens, no magic number reference date.
- **Recommended action**: Correct. No change.

---

### 10. lockin-stdlib-and-syntax.md Finding #25 — Go iota ordering dependency
- **Currently tagged**: Already solved (file says "`options` keyword — values are named, not positional integers")
- **Confirmed**: `design/type-system.md` documents `options Status { active, inactive, banned }`. `design/naming.md` confirms `options` is the named-constant mechanism. Reordering doesn't change semantics.
- **Recommended action**: Correct. No change.

---

### 11. lockin-cpu-bigo.md Finding #9 — Rust bounds check elimination failure
- **Currently tagged**: at-risk
- **Should be tagged**: partially-addressed (already acknowledged but needs work)
- **Where Yinz solves it**: `design/ownership.md` "No Direct Array Indexing" section: "In release mode, the compiler eliminates bounds checks it can statically prove are safe (e.g., a `fixed<3>` accessed at index 1 — provably in bounds)." AND `design/collections.md` states `.get(index)` compiles to bounds check + conditional in debug, with elimination in release for provable cases. The finding's Yinz status says "at-risk" and "the compiler should have an explicit analysis pass for fixed-size-array index proof before delegating to LLVM." Yinz's design ALREADY commits to this (`fixed<T>` bounds elimination). The at-risk part is accurate for LLVM's widening-multiply failure — Yinz can mitigate by doing its own proof pass first before emitting LLVM IR, which is exactly what the design promises.
- **Recommended action**: Re-tag as partially-addressed (the fixed<T> case is solved; the general LLVM analysis limitation is mitigated by design intent but not yet in a formal spec).

---

## Proposed Fixes That Conflict With Locked Design

### Conflict #1: Highlight #3 Summary Table still lists `array.withCapacity(n)`
- **Highlight #**: 3 (verification.md summary table row, column "Yinz fix one-liner")
- **My proposed fix**: Row 3 of the summary table says "1.5× growth, `array.withCapacity(n)` for known size"
- **The conflict**: Patrick caught this in the chat transcript within verification.md itself. `fixed<T>` is Yinz's solution for known-size collections. There is no need for `array.withCapacity(n)`. `design/collections.md` is explicit: "`fixed<T>` = stack-allocated, size-locked at creation. `array<T>` = heap-allocated, growable." The summary table row was not corrected after the chat exchange; only the body of Highlight #3 was updated.
- **Recommended correction**: Remove `array.withCapacity(n)` from the summary table row for Highlight #3. The correct one-liner is: "1.5× growth factor for `array<T>`; `fixed<T>` covers the known-size case."

---

### Conflict #2: Highlight #17 proposes `dyn Comparable` syntax
- **Highlight #**: 17 (verification.md, proposed fix in body — dropped from highlight reel but fix still in the file)
- **My proposed fix**: "Lock in `design/type-system.md`: Yinz `follows` contracts use static dispatch (monomorphization) by default; dynamic dispatch is opt-in via explicit syntax (e.g., `dyn Comparable` or similar — name to be decided)."
- **The conflict**: `design/type-system.md` uses `follows` for contracts. `design/generics.md` uses inline `<T follows Comparable>` for generic constraints with static dispatch. Neither document introduces a `dyn` keyword — and Golden Rule 12 (human-readable over jargon) would reject `dyn` as CS jargon that a HS-grad JS dev cannot guess. The Yinz vocabulary files have no `dyn` term. The naming convention is: plain English words, not Rust-isms. `dyn` is Rust-specific terminology.
- **Recommended correction**: When dynamic dispatch syntax is designed, it must not use `dyn`. A placeholder like `dynamic Comparable` or a dot-method form would fit Yinz's conventions better. The fix should not propose a specific keyword that conflicts with Golden Rule 12 — instead note "dynamic dispatch opt-in syntax TBD, must follow Golden Rule 12."

---

### Conflict #3: Highlight #8 proposes "auto-Arc per-value, not infectious" — partially conflicts
- **Highlight #**: 8 (verification.md and highlight-reel-opus.md)
- **My proposed fix**: "specify the auto-Arc inference contract in `design/future/concurrency.md` to be per-value, not infectious. When a value crosses a `background` boundary and needs cross-thread sharing, only THAT value gets wrapped in Arc."
- **The conflict**: `design/future/concurrency.md` ALREADY specifies this behavior under "Runtime" section: "Cross-thread shared state crosses a `background` boundary via auto-inferred `Arc<T>` wrapping. The IDE shows the auto-Arc as a muted hint (cautionary red-tinted styling because reference counting has cost)." AND `design/concurrency.md` "Ownership with Background Tasks" section already defines `.give` (move) and `.copy` (clone) as the valid patterns for background tasks — `.share` is explicitly a compile error. The auto-Arc is for a different scenario (cross-thread shared state, distinct from simple task-to-task passing). The proposed fix is partially redundant — the design already exists.
- **Recommended correction**: The fix should say "this is already addressed in `design/future/concurrency.md`; verify the per-value (non-infectious) nature is explicit in that file" rather than proposing it as a new design move.

---

### Conflict #4: Highlight #6 proposed fix uses "onPanic" terminology not in Yinz vocabulary
- **Highlight #**: 6 (verification.md)
- **My proposed fix**: "The `background` handle exposes `.failed()` and `.error` so callers can poll/await failure state. Optional `background fn().onError(handler)` chain..."
- **The conflict**: `design/future/panic-safety.md` is referenced in the design files as the source of truth for task error handling, but the specific `.onPanic()` terminology vs `.onError()` vs `.failed()` naming isn't locked in verification.md's fix. More importantly: the proposed `.onError(handler)` chain syntax violates Golden Rule 7 — "Step-by-step over chaining. No method chaining." A `.onError(handler)` appended to `background fn()` IS method chaining.
- **Recommended correction**: The error-handling API for background tasks must use the step-by-step pattern, not a chained `.onError()`. Something like: `let task = background doThing(); task.waitForError()` or similar non-chaining form. The proposed fix should be revised to respect Golden Rule 7.

---

### Conflict #5: Highlight #9 proposes adding a rule to `.claude/rules/language-design.md`
- **Highlight #**: 9 (verification.md)
- **My proposed fix**: "add a stdlib design rule (in `.claude/rules/language-design.md` or new `.claude/rules/stdlib-design.md`): methods whose name implies a pure read MUST be pure."
- **The conflict**: This is not a conflict with locked design — it's a genuine gap. However, `.claude/rules/language-design.md` already has the readability test and the teaching test. A stdlib purity rule belongs in a STDLIB design rules file, not in the general language-design rules file. The proposed location is slightly wrong but the underlying principle is correct.
- **Recommended correction**: Target a new `.claude/rules/stdlib-design.md` rather than the existing language-design.md. The existing file covers language features, not stdlib API contracts.

---

### Conflict #6: Highlight #11 proposes "Stdlib APIs ship once and are fixed in place. No v2 alongside v1" for `.claude/rules/language-design.md`
- **Highlight #**: 11 (verification.md)
- **My proposed fix**: "add to `.claude/rules/language-design.md`: 'Stdlib APIs ship once and are fixed in place. No `v2` alongside `v1`.'"
- **The conflict**: `design/versioning.md` ALREADY addresses this — "Pre-release delete policy, post-release major bumps, no backwards compat" per the decisions index. `design/mvp-scope.md` explains the granular versioning model where each version ships ONE focused thing. The fix is redundant — the principle is already in `design/versioning.md`. Adding it to the rules file would be fine but it's not a gap.
- **Recommended correction**: Note that this principle is already in `design/versioning.md` and does not need to be invented as a new rule. The fix should reference the existing versioning design rather than proposing a new rule.

---

## Vocabulary Slips

### Slip #1: verification.md Highlight #4 — "Struct" instead of "shape"
- **Where**: verification.md, Highlight #4, title: "Struct field auto-reorder for packing"
- **Wrong term used**: "Struct" (appears in the highlight title and throughout lockin-cpu-bigo.md Finding #11)
- **Correct Yinz term**: `shape` — per `.claude/rules/vocabulary.md`: "Never use legacy terms... `struct` → `shape`"
- **Note**: The title "Struct field auto-reorder for packing" in both verification.md and highlight-reel-opus.md uses "Struct." The proposed fix body correctly says "`shape` fields" — so the body is correct but the title is not.

---

### Slip #2: lockin-cpu-bigo.md Finding #11 — "struct" throughout
- **Where**: lockin-cpu-bigo.md Finding #11, heading and body: "Struct padding from field-order mistakes"
- **Wrong term used**: "struct" (appears multiple times)
- **Correct Yinz term**: `shape`
- **Note**: The Yinz status section in Finding #11 correctly says "`shape` declarations" — so the Yinz-specific content is right, but the finding's title and cross-language description use "struct" throughout. For a findings document about Yinz's future design, the general section should either use "shape" in Yinz-specific context or clearly bracket "struct" as referring to other languages.

---

### Slip #3: lockin-cpu-bigo.md Finding #6 — "Object" in SoA description
- **Where**: lockin-cpu-bigo.md Finding #6: "Array-of-Structs (AoS) layout" and "struct" used in context
- **Wrong term used**: "struct" (appropriate for C/C++ context) but the proposed Yinz fix says "Yinz's `shape` declarations" — the Yinz-specific language is correct, but "struct" appears in the general description without being clearly bracketed as "other languages' concept"
- **Correct Yinz term**: `shape` when referring to Yinz specifically
- **Note**: Minor — in a findings document comparing many languages, using "struct" for the general concept is acceptable. The fix language is correct.

---

### Slip #4: lockin-cpu-bigo.md Finding #10 — "dyn" proposed
- **Where**: verification.md Highlight #17 body: "dynamic dispatch is opt-in via explicit syntax (e.g., `dyn Comparable` or similar)"
- **Wrong term used**: `dyn` — this is Rust's keyword, not a Yinz keyword, and violates Golden Rule 12 (human-readable over jargon)
- **Correct Yinz term**: TBD — no dynamic dispatch syntax is locked yet. Using `dyn` as a proposed placeholder imports Rust jargon.

---

### Slip #5: lockin-type-and-memory.md Finding #2 — "traits" used in Rust discussion context leaking to Yinz discussion
- **Where**: lockin-type-and-memory.md Finding #2 Yinz status: "Yinz's `follows` constraints are simpler than Rust traits — no blanket impls"
- **Analysis**: The word "traits" here correctly refers to Rust's mechanism, not Yinz's. This is fine — it's contrasting the two. Not a vocabulary slip.

---

### Slip #6: verification.md Highlight #12 — "shape" used correctly but "JSON module" named without version
- **Where**: verification.md Highlight #12 proposed fix: "when designing the JSON module (v0.9)"
- **Analysis**: Correctly names the version (v0.9 per `design/mvp-scope.md`). No vocabulary slip.

---

## Premature-Timing Fixes

### Timing #1: Highlight #1 — "Lock in `design/collections.md` before M4"
- **Highlight #**: 1
- **Proposed urgency**: "before M4" (which is the types+ownership milestone, part of v0.1)
- **Actual milestone**: `map<K,V>` hash function choice is unspecified in `design/collections.md` — the file discusses the type's API (`.get`, `.set`, `.filter`, `.find`) but not the internal hash algorithm. The implementation of `map<K,V>` is a v0.1 concern (the type is in v0.1 per `design/mvp-scope.md`). Locking the hash function before the first implementation IS appropriate — this timing is correct.
- **Assessment**: Not premature. The timing of "before M4/v0.1 ships" is right since `map<K,V>` is a v0.1 type.

---

### Timing #2: Highlight #2 — Swiss Tables implementation before M4
- **Highlight #**: 2
- **Proposed urgency**: Before M4 (v0.1 ship)
- **Actual milestone**: `map<K,V>` ships in v0.1. The implementation choice (Swiss Tables vs separate chaining) must be made before the first implementation. Correct timing.
- **Assessment**: Not premature.

---

### Timing #3: Highlight #13 — Regex "v0.14"
- **Highlight #**: 13 (verification.md body)
- **Proposed fix**: "when designing the regex module (v0.14), lock as linear-time NFA-based engine only"
- **Actual milestone**: `design/mvp-scope.md` confirms `regex` ships in v0.14. Correct reference.
- **Assessment**: Not premature. Correctly timed to the regex module version.

---

### Timing #4: Highlight #14 — JSON SIMD "v0.9"
- **Highlight #**: 14 (verification.md body)
- **Proposed fix**: "when designing the JSON module (v0.9)"
- **Actual milestone**: `design/mvp-scope.md` v0.9: "`json` — Parse, stringify, prettify." Correct.
- **Assessment**: Not premature.

---

### Timing #5: Highlight #16 — PGO "`ynz build --profile` and `ynz build --optimized` as first-class commands in v0.5+"
- **Highlight #**: 16 (verification.md body)
- **Proposed urgency**: "v0.5+ when the package manager ships"
- **Actual milestone**: `design/mvp-scope.md` v0.5 is the package manager. PGO is a build-system feature. However, `design/compiler.md` only defines "Debug" and "Release" build modes (`ynz build` and `ynz build --release`). PGO would be a third mode. The v0.5 milestone scope is specifically the package manager (`ynz add/remove/update/install` + lockfile). Adding `--profile` and `--optimized` flags alongside the package manager would be scope creep in v0.5. More accurately, PGO is a release-build enhancement that could ship any time after v0.1's compiler is working — there's no fundamental dependency on v0.5.
- **Assessment**: Minor premature timing issue. PGO flags can ship in v0.1 alongside `--release` (they're compiler flags, not package manager features) or in the v0.2 dev-loop tooling milestone. Tying it to "v0.5 when the package manager ships" is an arbitrary coupling.

---

### Timing #6: Highlight #5 (Auto-SoA) — "future optimization" but no version cited
- **Highlight #**: 5
- **Proposed urgency**: "compiler analyzes hot loops... emit SoA-transformed layout transparently" — no specific version proposed
- **Actual milestone**: Auto-parallelization optimization is v0.3. SoA transformation would be an even more ambitious compiler analysis — probably v0.3+ or later. The highlight correctly flags this as high-risk/high-reward but leaves timing open.
- **Assessment**: Timing is appropriately uncertain — no mistagging.

---

### Timing #7: Highlight #15 — String UTF-8 SIMD implementation "stdlib implementation choice... should be specified in `design/stdlib/strings.md` (not yet written)"
- **Source**: lockin-cpu-bigo.md Finding #16 Yinz status
- **Proposed urgency**: Strings ship in v0.1 (they're a core language feature). The SIMD validation implementation would be an internal implementation detail of the built-in string type.
- **Actual milestone**: SIMD UTF-8 validation for the built-in string type is an implementation detail of v0.1's string type — it should be specified before v0.1 ships. Calling it a "stdlib implementation choice" is slightly wrong; it's a compiler/runtime implementation choice for the built-in `string` type. The `design/stdlib/strings.md` file exists but covers string utility methods, not the internal encoding implementation.
- **Assessment**: The timing framing is slightly off. The UTF-8 encoding commitment belongs in a `design/strings.md` file (core language, not stdlib utilities). The SIMD implementation is an optimization that could land in any milestone but should be flagged as a v0.1 internal implementation goal.

---

## Genuinely Not-Yet-Addressed Findings (the real action list)

After stripping mistagged items, vocabulary slips with no design consequence, and conflicts already partially addressed — these are the findings that genuinely lack a locked Yinz design position:

1. **Map hash function choice** (lockin-cpu-bigo.md Finding #1) — fast non-crypto hasher (xxhash3 / AHash) vs SipHash default. `design/collections.md` specifies the map API but not the internal hasher. Decision needed before v0.1 `map<K,V>` is implemented.

2. **Map implementation algorithm** (lockin-cpu-bigo.md Findings #2+#3) — Swiss Tables vs separate chaining. `design/collections.md` is silent on implementation algorithm. Decision needed before v0.1 `map<K,V>` is implemented.

3. **`array<T>` growth factor** (lockin-cpu-bigo.md Findings #12+#22) — 1.5× vs 2× vs adaptive. `design/collections.md` is silent. Decision needed before v0.1 `array<T>` is implemented.

4. **Shape field auto-reorder for packing** (lockin-cpu-bigo.md Finding #11) — whether the compiler auto-reorders `shape` fields for cache-line efficiency, and how FFI-facing shapes opt out. Partially addressed in the design-lockdown plan but not locked in `design/type-system.md` or `design/ownership.md`.

5. **AoS → SoA auto-transform** (lockin-cpu-bigo.md Finding #6) — compile-time optimization for hot loops over `array<shape>`. Not in any design file. Conservative vs aggressive approach TBD. Appropriate for v0.3+ compiler optimization work.

6. **Background task error observability** (lockin-concurrency.md Finding #14 / Highlight #6) — the specific error-notification API for `background` tasks. `design/future/panic-safety.md` (referenced but not read in full here) covers the panic model, but the step-by-step API for callers to observe task errors is unspecified. Must follow Golden Rule 7 (no method chaining).

7. **Channel/queue bounded-by-default** (lockin-concurrency.md Findings #12+#17) — `design/future/concurrency.md` lists "Channel/queue primitives" as an open v0.2 question. Whether they're bounded by construction (Erlang lesson) is not locked. This is the most actionable open concurrency question.

8. **Scheduler design for v0.2** (lockin-concurrency.md Finding #16) — work-stealing vs single-threaded vs configurable. Explicitly listed as an open question in `design/future/concurrency.md`.

9. **Cancellation mechanism for `background` tasks** (lockin-concurrency.md Findings #2+#5) — cooperative checkpoint vs cancellation token vs other. Explicitly listed as open in `design/future/concurrency.md`.

10. **CPU/IO separation in scheduler** (lockin-concurrency.md Finding #11) — "the specific CPU/IO split is not explicitly documented in `design/future/concurrency.md` — this needs to be explicit in the v0.2 implementation plan." The lockin file already flags this.

11. **No parallel APIs rule** (lockin-stdlib-and-syntax.md Finding #7, #15, #30) — `design/versioning.md` covers the macro policy but does not explicitly state "never ship v1 and v2 of the same API simultaneously." Worth adding to `design/versioning.md`.

12. **Stdlib purity rule** (lockin-stdlib-and-syntax.md Finding #5 / Highlight #9) — no design file explicitly states "pure-named methods MUST be pure (no I/O)." Needs a home in a `design/stdlib-design.md` rule file or added to `.claude/rules/language-design.md`.

13. **Serialization via compile-time codegen** (lockin-stdlib-and-syntax.md Finding #14 / Highlight #11) — `design/collections.md` and `design/type-system.md` don't address serialization. When the JSON module (v0.9) is designed, the "compiler-generated per-shape serializer, never reflection" rule needs to be locked in `design/stdlib/data.md` or the JSON module design.

14. **Regex engine choice** (lockin-cpu-bigo.md Finding #20 / Highlight #12) — NFA-based linear-time only vs PCRE backtracking. Not locked anywhere. Needs to be locked in `design/stdlib/` before v0.14 work starts.

15. **JSON parsing SIMD implementation** (lockin-cpu-bigo.md Finding #5 / Highlight #13) — simdjson adoption vs native SIMD parser. Not locked. Needs to be decided before v0.9 implementation.

16. **String UTF-8 SIMD validation** (lockin-cpu-bigo.md Finding #16) — built-in string type should use SIMD validation (simdutf or equivalent). Not explicitly committed. Should be added to `design/compiler-language.md` or a new `design/strings.md` as an implementation target.

17. **UTF-8 as explicit committed encoding** (lockin-type-and-memory.md Finding #3 / Highlight #14) — implied by the indexing API but not stated explicitly in any design doc. Needs a `design/strings.md` file or a section in `design/collections.md` that says "internal encoding is UTF-8" unambiguously.

18. **PGO as first-class build mode** (lockin-cpu-bigo.md Finding #17 / Highlight #16) — not in `design/compiler.md`. The compiler doc defines only debug and release builds. PGO (instrument → run → profile → recompile) is a third mode with a defined workflow that should be added to `design/compiler.md`.

19. **Package registry immutability + unpublish policy** (lockin-build-and-crossplat.md Finding #1) — `design/packages.md` and `design/future/packages.md` defer registry policy to `design/registry.md` (not yet written). Content-addressed, immutable-once-published policy should be locked before v1.2 registry work starts.

20. **Build-script sandboxing** (lockin-build-and-crossplat.md Findings #2+#3) — if Yinz ever adds a build-script mechanism (for FFI compilation), whether it's sandboxed by default or opt-in. Currently unaddressed because FFI is deferred to v2+. Low urgency but worth a note in `design/ffi.md` for when FFI design begins.

21. **Monomorphization compile-time scaling** (lockin-build-and-crossplat.md Finding #13) — `design/compiler.md` acknowledges "separate compiled code per generic instantiation" but does not address deduplication strategy for large projects. Worth adding to the compiler design as a known future scaling concern with mitigation options documented.

22. **Static linking / glibc version skew** (lockin-build-and-crossplat.md Finding #12) — Yinz targets LLVM native code but the glibc linkage model is unspecified. The lesson (static link or musl for portability) is not locked. Should be in `design/compiler.md` before any Linux binary distribution decisions are made.

23. **Async stack traces for `background` tasks** (lockin-concurrency.md Finding #20 / Highlight #16) — `design/future/concurrency.md` mentions stackless state machines but not production-debugging observability. The v0.2 implementation plan must address spawn-site tracking per the proposed fix.

---

## Summary

- **7 findings are actually already-solved** (currently mistagged as at-risk or not-yet-decided)
- **6 proposed fixes conflict with locked design** (wrong keyword, method chaining violation, redundant with existing design, wrong file target)
- **5 vocabulary slips** (predominantly "struct" instead of `shape` in titles; `dyn` as a Rust-ism in one proposed fix)
- **2 premature-timing fixes** (PGO tied to wrong milestone; UTF-8 SIMD framed as stdlib rather than language-core)
- **23 genuinely-actionable findings remain** (hash function, growth factor, channel bounds, scheduler design, cancellation, UTF-8 commitment, serialization codegen rule, regex engine, JSON SIMD, PGO build mode, registry policy, static linking model, async stack traces, and others)
