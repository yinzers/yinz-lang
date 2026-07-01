---
name: "m7-strings-errors-iterables"
plan-id: "2026-05-18-m7-strings-errors-iterables"
status: "done"
roadmap-id: "2026-05-12-v0-1-compiler"
session-id: []
created_at: "2026-05-18"
updated_at: "2026-05-18"
metadata:
  type: "plan"
legacy:
  note: "Fields below are preserved verbatim from the pre-migration .claude/plans/ ledger-format frontmatter (2026-07-01 migration to .claude/planning/). session-id history was not tracked pre-migration."
  slug: m7-strings-errors-iterables
  owner: patrick
  status: done
  roadmap: v0-1-compiler
  milestone: m7-strings-errors-iterables
  files:
    - Cargo.toml
    - crates/**
    - design/strings.md
    - design/errors.md
    - design/iterables.md
    - spec/strings.md
    - spec/errors.md
    - spec/iterables.md
    - examples/pirates-roster/**
    - examples/primantis-orders/**
    - .claude/plans/active/v0-1-compiler.md
    - .claude/state.md
    - .claude/todos.md
  created: 2026-05-18
  last_updated: 2026-05-18-r2
  depends_on: [v0-1-compiler, m6-options-unions]
---


# Plan: M7 — Full Strings, `errors` Keyword, Iterables Protocol

Created: 2026-05-18
Status: done

## Context & Why

**Goal.** Ship M7 of the v0.1 compiler — the third-largest milestone, bundling three interlocking concerns: full Unicode strings (UTF-8 with SSO + SIMD), the `errors` keyword with flow-sensitive auto-propagation ("cascades"), and the `Iterable<T>` / `FallibleIterable<T>` protocol that unifies for-loop dispatch across built-ins and user-defined iterables. Plus the four `REPLACE-AT M7` markers M5 left behind and the M3 catch-up obligations (`range` first-class, NFC string equivalence).

**Why now.** M6 closed all M3 narrowing / options / union catch-ups. M5 closed all M4 ownership / generics / collection catch-ups. The remaining v0.1 catch-up surface is concentrated in M7. After M7 ships, v0.1 has exactly one milestone left (M8 — modules, imports, doc comments, sensitive modifier, concurrency keyword parsing, bignum reservation).

**Why these three concerns belong together, not split.** They interlock:
- `FallibleIterable<T>` needs the `errors` keyword to express `next() -> maybe T errors`.
- New string methods like `.toInt()` (M6 closed `.toInt()` on strings) and the future `.replace(regex, fn)` need `maybe T` / `T errors` return types — strings and errors share the same vocabulary.
- The for-loop generalization to `Iterable<T>` is the natural place to add string iteration (`for c in "café"`), which forces string codegen to mature past M1's `puts` skeleton.
- File I/O (v0.5+) is the canonical motivator for `FallibleIterable<T>`; ship `errors` + iterables together so the v0.5 design doesn't require a re-plumbing pass.

Splitting (e.g., "M7a strings, M7b errors+iterables") would force a transient state where strings can't express their fallible methods cleanly — net more work, not less.

**Background.** Where we are at v0.1.0-m6 (tag pushed, 631 tests):
- Strings exist as i8* null-terminated C-strings allocated as LLVM globals. Concatenation lowers to `ynz_string_concat` (runtime returns a heap-allocated `*u8`). Equality uses `ynz_string_eq` (byte-by-byte). No interpolation. No SSO. No code-point/grapheme indexing. No SIMD. `s[0]` parses (M5 bracket sugar) but typeck rejects: "string bracket access arrives in M7."
- Errors don't exist. Every function is either `-> T` (must succeed) or returns `maybe T` (M5). There is no `errors` keyword, no auto-propagation, no `.failed()` method, no runtime error struct, no `.message` / `.suggestions` / `.trace` / `.source`. Functions that should fail today either return `maybe T` (loses the "why") or panic via `ynz_panic` (terminates).
- For-loops are special-cased in typeck and codegen for `range(start, end)`, `array<T>`, `fixed<T, N>`, and `map<K, V>` (the latter producing `MapEntry<K, V>` per iteration). `Type::Range` is a typeck-only type — using `range(...)` outside a `for`-loop iter position is rejected with an "arrives in M7" diagnostic. User-defined types cannot be iterated. Strings cannot be iterated.
- Four `REPLACE-AT M7` markers in source:
  - `crates/ynz-ast/src/nodes.rs:526` — `Type::Range` AST variant
  - `crates/ynz-typeck/src/types.rs:35` — `Type::Range` typeck variant
  - `crates/ynz-typeck/src/check.rs:631` — for-loop special-case dispatch
  - `crates/ynz-runtime/src/lib.rs:195` — `ynz_string_eq` uses byte-equality, not Unicode canonical equivalence (NFC normalization). M3 programs don't produce NFD strings so byte-eq is correct for all current programs — but the M3 catch-up obligation says this must be fixed when full strings ship.
- Three codegen for-loop sites in `crates/ynz-codegen/src/emit.rs` (~line 1053 array, ~1109 map, ~1177 range). Fixed is folded into the array path with a slight tweak. M5's plan documented "six total (three typeck + three codegen)" — the actual count is 4 typeck/AST/runtime + 3 codegen = 7 unwind sites total. Pre-condition for plan acceptance per M5 carry-over.

**Constraints.**
- Rust stable, LLVM 18, salsa, inkwell — no toolchain changes.
- All M4 ownership invariants carry forward. New iterator wrapper shapes must respect `share`/`lend`/`give` rules. `errors`-capable values participate in the consume-tracking machinery.
- All M5 generics + monomorphization invariants carry forward. `Iterable<T>` is a generic shape; instantiation per concrete `T` happens at every site where a collection is iterated.
- All M6 narrowing rules carry forward. `errors`-capable values are flow-narrowed (just like `maybe<T>`); the same machinery is reused.
- Banned-jargon list extended: no "fallible/infallible" in user-facing diagnostics (already banned in M6 — must stay banned); no "monad", "lift", "wrap" — even though `errors`-capable IS conceptually a monad, user-facing language is "might fail" / "auto-propagates" / "cascades".
- Cargo.toml version bumps from `0.1.0-m6` to `0.1.0-m7` at M7 ship.
- M7 ships behind no feature flag. All features unconditionally enabled when merged.

**Success criteria for M7 (this milestone's contract):**
1. `errors`-capable functions compile, run, and auto-propagate per `design/errors.md` flow-sensitive rules (matching M6's narrowing infrastructure).
2. Strings ship with SSO (23-byte inline) and SIMD-accelerated UTF-8 validation/search. `.byteAt`, `.get`/`[n]` (code points), `.graphemeAt`, `.contains`, `.indexOf`, `.startsWith`, `.endsWith`, `.toUpperCase`, `.toLowerCase`, `.substring`, `.trim`, `.split`, `.replace` work. Interpolation works (backtick + `${expr}`). Multi-line strings work.
3. `Iterable<T>` and `FallibleIterable<T>` contract shapes exist as built-in primitives. Built-in collections (`array<T>`, `fixed<T, N>`, `map<K, V>`, `Range`, `string`) follow `Iterable<T>` via synthesized iterator wrapper shapes (`ArrayIter<T>`, `FixedIter<T, N>`, `MapIter<K, V>`, `Range` itself, `StringCodePointIter`). User-defined shapes can `follows Iterable<T>` by writing a standalone `function next(lend self: Foo) -> maybe T`. The for-loop dispatches uniformly. `range()` is first-class (storable, passable, returnable). `MapEntry<K, V>` remains a built-in shape carrying iteration entries.
4. NFC canonical equivalence for `ynz_string_eq` (M3 catch-up closed).
5. `file.lines(path)` — or a stub thereof — returns a `FallibleIterable<string>`. For-loop over a fallible iterable inside an `errors` function auto-propagates per-step errors. `.orSkipFailures()` and `.withErrors()` adapters work.
6. All four source `REPLACE-AT M7` markers removed; all three codegen for-loop special-cases unwound; M3 `range()` typeck special-case unwound.
7. `examples/pirates-roster/entrypoint.ynz` extended to demo strings + errors + iterables in context. `examples/primantis-orders/m7_errors.ynz` triggers every new diagnostic class.
8. Tag `v0.1.0-m7` cut on main. ~750 tests target (up from 631).

---

## FINAL LOCKED DECISIONS (pre-draft, confirmed by Patrick)

1. **SSO ships in M7** (23-byte inline threshold per `design/strings.md`). String runtime built right from day 1. String value is a 24-byte struct: `{ tag_or_len: u8, data: 23 bytes }` (inline) OR `{ tag: u8 = 0xFF marker, len: i64, ptr: *u8, cap: i64, padding: ... }` (heap, fitting in 24 bytes via packed layout). Exact layout locked in P0.
2. **SIMD UTF-8 ships in M7** (validation + search). Rust-native SIMD via `std::simd` (portable) OR a vetted crate (`simdutf8` — Rust port of simdjson UTF-8 validator). Choose specific crate vs handrolled in P0; if a crate, vendor or pin exact version. Fallback path on architectures without SIMD: scalar implementation.
3. **Synthesized iterator wrapper + muted-hint teaching surface** for `Iterable<T>` dispatch. Built-in collections formally follow `Iterable<T>` through compiler-synthesized wrapper shapes (`ArrayIter<T>`, `FixedIter<T, N>`, `MapIter<K, V>`, `StringCodePointIter`). `Range` is a user-visible shape that follows `Iterable<int>` directly. User-defined iterables write a standalone `next()` function. M7 ships the codegen; the muted-hint surface showing the implicit `.items()` / `.entries()` insertion defers to v0.2 LSP per `inference.md`. M7 emits the data salsa side-table for v0.2 to consume.
4. **Full base error shape ships in M7**: `.message: string`, `.suggestions: array<string>`, `.trace: array<Frame>`, `.source: SourceLoc`. Trace capture: each `errors` function entry pushes a `Frame { file: string, line: int, function: string }`; auto-propagation captures the call site. Source position emitted at every `errors` call site via salsa-tracked debug-info side-table. Runtime: `ynz_error_new`, `ynz_error_drop`, accessor functions for each field. Stack-walking is salsa-emitted compile-time data, NOT libunwind dynamic capture — keeps M7 hermetic from system unwinder ABI.

These four decisions answer the four Pre-Draft questions confirmed in chat 2026-05-18.

---

## Research Findings

- `design/strings.md` is the authoritative source: UTF-8 internal encoding (locked), SSO 23-byte (locked), SIMD goal (locked), locale-invariant case (locked), `.byteAt` / `.get` / `.graphemeAt` API surface (locked).
- `design/errors.md` is the authoritative source for `errors` semantics: flow-sensitive auto-propagation under-the-hood, eager-feel for users, base error shape `{ message, suggestions, trace, source }`, identical method set in errors-and-non-errors contexts. The "Implementation Note (for M7)" section explicitly scopes the work to this milestone.
- `design/iterables.md` is the authoritative source for `Iterable<T>` and `FallibleIterable<T>` contracts. The `next(lend self) -> maybe T` (or `maybe T errors`) shape signature is locked. The `.orSkipFailures()` and `.withErrors()` adapter API is locked.
- `design/narrowing.md` (M6) locks the flow-sensitive narrowing rules. M7 extends this machinery to `errors`-capable values — same rules table, new binding flavor.
- `design/collections.md` "String Methods" section is the M7 string method surface area. `design/stdlib/strings.md` (if it exists; should be created in P0 if not) hosts the per-method documentation.
- `unicode-segmentation` (1.13.2) is already in `Cargo.toml` workspace deps — handles graphemes. M7 wires it up.
- `unicode-normalization` is NOT yet in `Cargo.toml`. Pin in P0 (latest stable: 0.1.x as of 2026-01; verify exact version at P0 time). Needed for NFC normalization in `ynz_string_eq`.
- SIMD UTF-8 validation crate selection: `simdutf8` (Rust port of simdjson's UTF-8 validator, MIT/Apache-2.0, ~700 LOC, no dependencies, runtime CPU feature detection with scalar fallback). Decision in P0; default leaning is `simdutf8`.
- Stack unwinding for `.trace`: NOT using libunwind / backtrace crate. Compile-time-emitted frame data via salsa debug-info table. Each `errors` function gets a `static FRAME: Frame = { file: __FILE__, line: __LINE__, function: __NAME__ }` global; entry IR pushes onto a thread-local frame stack via `ynz_frame_push` / `ynz_frame_pop`; `ynz_error_new` snapshots the current stack into the error.
- SSO layout precedent: libc++ uses 22 bytes (one less than what we lock). Rust's `compact_str` crate uses 24-byte struct with inline 24 bytes-1 = 23 inline. Yinz's 23-byte choice aligns with `compact_str`. Storage decision in P0.
- M5 `MonomorphizationTable` and `GenericShapeTable` carry forward unchanged. M7's new generic instantiations (`Iterable<int>`, `Iterable<Player>`, `FallibleIterable<string>`, the iter wrapper shapes) flow through M5's machinery — no new generics infrastructure.
- M5's `BuiltinArray`, `BuiltinFixed`, `BuiltinMap`, `Maybe` types stay. M7 ADDS `BuiltinString` (or extends the existing `String` primitive type) with the new layout, and adds synthesized-wrapper types `ArrayIter<T>`, `FixedIter<T, N>`, `MapIter<K, V>`, `StringCodePointIter`, `Range`. Each iter wrapper is a real `shape` (gets `ShapeTable` entry, monomorphizes per concrete T).
- M6's narrowing infrastructure for `maybe<T>` is the template for `errors`-capable narrowing. The salsa-tracked `NarrowingTable` is extended; no new infrastructure file.
- Multi-error cap (50/compile per `design/compiler-errors.md`) is already implemented and carries over.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| SSO layout ABI break | Medium | Every binary linking against Yinz strings needs recompile after a layout fix | Lock exact bit layout in P0 with a written ABI spec doc. Reproducibility test: SHA-256 the IR of a known-fixture string to catch layout drift between commits. |
| SIMD UTF-8 portability | Medium | CI passes on x86_64 / ARM64, breaks on i686 or RISC-V contributors | Use `simdutf8` (has scalar fallback baked in via runtime feature detection). CI runs on x86_64 (current) PLUS a scalar-fallback CI run with SIMD intrinsics disabled to verify the fallback path. |
| Stack-trace frame allocation in hot paths | High | `errors` function entry pushes a frame on every call — measurable overhead on tight call loops | Frame push/pop is a 2-instruction sequence (load thread-local pointer, write frame). Cost is ~10ns/call. Programs that don't error pay only this; programs that DO error pay the full error allocation. If measurements show >2% regression on a representative benchmark, switch to compile-time tail-call elision for `errors` functions whose only fallible op is the last call. |
| NFC normalization on every string equality | High | `ynz_string_eq` previously was 1-cycle/byte loop; NFC normalization is allocating | Two-tier strategy: (a) fast path — if both strings have the `is_nfc_known` cache bit set, byte-compare. (b) slow path — normalize both to NFC into a small stack buffer (≤256 bytes) or fall back to heap; cache the result on the source strings if mutable, ELSE just do it. P0 locks the cache bit semantics. |
| `Iterable<T>` wrapper allocation per for-loop site | Medium | Hot loops allocate a 32-byte iter wrapper on every entry; degrades cache | Wrapper is stack-allocated (alloca). LLVM optimizer should eliminate it in most loops (inlinable `next()`). IR-snapshot tests assert no `ynz_alloc` call for built-in iterations. If the optimizer misses an inlining opportunity, mark `next()` for built-ins as `alwaysinline` codegen attribute. |
| Auto-propagation IR emission breaks salsa cache | Medium | Every change to an `errors` function call site re-flows the auto-propagation graph through the whole module | Per-function auto-propagation is salsa-isolated: changes to function A's body don't re-flow function B unless B calls A. Verified via salsa query trace test. |
| Interpolated string codegen complexity | Medium | `` `${a} + ${b}` `` produces N+1 concatenations + N `.toString()` calls — generates a lot of IR | Lower to a single `ynz_string_builder` call sequence: pre-size a buffer, append each segment, finalize. One heap allocation per interpolated string. IR-snapshot asserts. |
| MapEntry<K, V> destructuring in for-loop | Medium | `for ((k, v) in m)` requires pattern-destructure binding — new parser surface | Lock the pattern syntax in P0: `for ((k, v) in m)` desugars to `for (entry in m) { let k = entry.key; let v = entry.value }`. Single-binding `for (entry in m)` also works. P3b implements both forms. |
| `errors` keyword conflicts with `errors` identifier in user code | Low | Some user variable named `errors` would have triggered the M2-onward "banned identifier" check; verify it still does | M2 banned `errors` as a reserved word at lexer level. Verify in P1 via existing test. No new risk. |
| String SSO inline vs heap discriminator ambiguity | Medium | Reading `tag` byte vs reading `len` byte — wrong interpretation = wrong access | Bit-pattern locked in P0: top bit of byte-0 = 1 means inline (other 7 bits = length 0..23); top bit = 0 means heap (the byte is the start of an aligned pointer field). Compiler emits the discriminator check at every string access. |
| Trace capture overhead under recursion | Medium | Deeply recursive `errors` functions push/pop hundreds of frames | Frame stack is a thread-local `array<Frame>` capped at 1024 frames; overflow truncates with a "trace truncated" sentinel. Cap chosen to fit recursion depth of any reasonable real-world program (Rust default recursion limit is 128; we accept 1024 as 8× headroom). |
| Iterable<T> Wrapper monomorphization explosion | Medium | Every concrete `array<T>` for-loop instantiates a new `ArrayIter<T>` — same combinatorial as M5 generics | Reuse M5's `MonomorphizationTable` directly. M5's dedup logic ensures `ArrayIter<int>` is one specialized shape across all uses. IR-snapshot asserts. |
| String runtime ABI break vs M1-M6 stored strings | High | Shape fields of type `string` were M1's i8*. M7 changes to 24-byte struct. Every existing fixture has stored strings | M7 P4b ships the new layout. All existing fixtures using `string` re-compile under the new ABI (the source doesn't change; only the codegen output does). Verification: rerun every M1-M6 fixture under M7 and confirm identical stdout (golden-file comparison on the snapshot harness). |
| FallibleIterable adapter `.orSkipFailures()` swallows errors | High | Same trap the design rejected (silent stop on I/O error returning truncated data) | `.orSkipFailures()` MUST log each skip via the panic-but-don't-exit infrastructure — the design says "silently logs and continues" but "silently" is for the user code, NOT for system observability. Log goes to stderr at a configurable threshold; documented in `design/iterables.md` deep-dive in P0. |
| `.trace` array<Frame> requires `Frame` shape | Medium | Frame is a new built-in shape — must be defined for users to read via `err.trace[i].file` | Define `Frame` as a public built-in shape in P0; document in `spec/errors.md`. Fields: `file: string, line: int, function: string`. |
| Cycle in iterator state (CountDown's hidden current field) interacting with M5 cycle-leak | Low | M5 documented that `maybe<Node<T>>` cycles leak. Iterators with hidden fields holding references could similarly cycle | M7 iterator wrappers hold `share` borrows of the source, not owned references. No cycle possible. P0 documents the borrow contract. |

---

## Questions (for Patrick to answer at plan-review time if not covered)

1. **Multi-line string escape rules**: backtick-quoted strings — does `` `\n` `` produce a newline (interpreted) or the literal two-char `\n` (raw)? `spec/strings.md` shows multi-line examples without addressing escapes. Recommendation: interpret escapes inside backticks (TS-compatible). Lock in P0.
2. **String iteration default — code points or graphemes?** `for c in "café"` — does each `c` step a code point or a grapheme cluster? Recommendation: code points (matches `s.get(n)` default per `spec/strings.md`). Graphemes available via `s.graphemes()` returning `Iterable<string>`. Lock in P0.
3. **`Frame.line` zero-based or one-based?** Both have precedent. Recommendation: one-based (matches editor / compiler error line numbering convention). Lock in P0.
4. **Stack-trace truncation behavior — at 1024 frames** — silent truncation with sentinel frame, or panic?  Recommendation: silent truncation; the user can still see what's at the top + bottom of the stack which is what matters for debugging. The truncation point gets a sentinel `Frame { file: "<truncated at depth 1024>", line: -1, function: "<...>" }`.
5. **`file.lines(path)` — ships as M7 stub or full v0.5 implementation?** Recommendation: ship a one-page minimal `file.lines()` implementation in M7 (just `BufReader::lines` wrapped to expose `next() -> maybe string errors`) so we have a real fallible iterable to test. Full v0.5 file system module supersedes it later with same surface.

### LOCKED 2026-05-18 — String syntax: unified backticks, one form

**Final decision**: Yinz strings have ONE form — `` `...` ``. Every string is a backtick-quoted string. ALWAYS supports `${expr}` interpolation. Multi-line is just an embedded newline. No double-quoted form, no single-quoted form exist in v0.1.

**Decision rationale (locked)**:

1. **English-collision analysis is the deciding factor.** Of the three candidate quote characters, only backtick is collision-free with natural English text:
   - `"` collides with dialogue / quotation in every chat log, novel, comment, error message
   - `'` collides with apostrophes in every possessive (`Patrick's`) and contraction (`don't`, `it's`) — basically every English sentence
   - `` ` `` collides only with code-context references (markdown code-spans) which are themselves meta-level and rare in user-content strings
   This makes backtick the ONLY default that doesn't impose constant escape pain on natural-language strings.

2. **One form = one rule.** No "which form interpolates?" decision tree. No "this string surprised me by evaluating an expression" confusion. Golden Rule 2 (self-documenting) wins clearly.

3. **Lexer simplicity.** ONE state machine for ONE form. ~40% smaller P1 lexer surface vs the original dual-form draft. No "track which open-quote we used" state. No "did this string-form trigger interp?" branching.

4. **JS template-literal familiarity** (Golden Rule 6). Every JS/TS developer has muscle memory for backtick-with-`${}`. Yinz inherits this for free.

5. **Multi-line is free.** Newlines just work inside backticks. No `"""..."""` mode needed. Single form covers single-line AND multi-line.

6. **Non-US keyboard ergonomics cost is real but bounded** (acknowledged tradeoff). Backtick is AltGr or dead-key on French AZERTY / German QWERTZ / Italian / Spanish layouts. JS dev community has lived with this for ~10 years writing template literals; survivable. The cost-of-escape on `"`/`'` would be COMPOUNDING (every English string forever); the cost-of-backtick-keystroke is FIXED (one keystroke per string, learnable muscle memory).

**Locked escape table**:
- `` \` `` — literal backtick character
- `\${` — literal `${` sequence (only needed when writing docs about Yinz interpolation syntax)
- `\n`, `\t`, `\\`, `\r`, `\0` — standard control chars
- NOT `\u{...}` — deferred to v0.5+ stdlib expansion

**Implementation impact on M7 phases**:
- **P0 step 15 (escape rules)**: rewrites to the table above. Single-form lexer; `\`` and `\${` are the new entries vs M6's existing string-escape behavior.
- **P0 step 14 (string iteration default)**: unchanged — `for c in someString` walks code points.
- **P1 lexer**: ONE state machine for backtick-strings. Enters `${...}` expression mode on `${`; exits on matching `}`; closes string on `` ` ``. Removes any planned dual-form lexer logic. Per-string state-machine complexity drops vs the original draft.
- **P1 AST**: still `InterpolatedString(Vec<StringPart>)` — every string IS an interpolated string at the AST level (parts list may be a single Lit chunk if no `${}` appeared).
- **P2 parser**: simpler — no "is this `"..."` or backtick?" branching at string entry. Every string-literal call site produces an InterpolatedString.
- **spec/strings.md**: section rewrite — remove all references to double-quote strings. All examples use backticks. Update the "two quote forms" table to a one-form description.
- **examples/pirates-roster/entrypoint.ynz**: sweep — convert any remaining `"..."` literals (M1-M6 era) to backtick form. May need a migration phase as part of P5 demo-extension OR a one-time conversion in P0.
- **All M1-M6 fixtures**: must convert `"..."` to `` `...` `` since double-quote form no longer exists. THIS IS A BREAKING CHANGE TO EVERY EXISTING FIXTURE. Migration approach: one-time sed-pass in P0 step (new step) converts `"..."` to `` `...` ``; verify by re-running every M1-M6 fixture and confirming identical stdout.
- **P1 MUST include unterminated-`${` negative fixture**: `` `hi ${name` `` (forgot `}`) produces a three-part diagnostic; lexer error recovery doesn't run off into the next 50 lines.
- **Banned-jargon list**: no new entries needed for the syntax decision itself.

**Risk added by this decision**: every existing M1-M6 fixture must be migrated. ~50+ source files touched in one sweep. Mitigated by: deterministic mechanical conversion (regex `"..."` → `` `...` ``); verification = every fixture's stdout is byte-identical post-conversion.

**Status**: LOCKED. P0 begins with the migration sweep as a new explicit step. P1 lexer is unblocked.

---

## Risk Assessment & Rollout Strategy

**Risk level: MEDIUM (architectural).** Not a production rollout — this is compiler dev work. The MEDIUM rating reflects: SSO ABI lock-in, NFC perf cost on every comparison, trace capture in hot paths.

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | — |
| Touches auth/permissions | No | — |
| Raw SQL / literals | No | — |
| Modifies existing data | Yes | String runtime ABI changes — every M1-M6 fixture using stored strings goes through new layout |
| Third-party integration | Yes | Adds `unicode-normalization`, `simdutf8` crates |
| Changes existing endpoints | No | Compiler internal |
| Wrong foundational choice cascades | Yes | SSO threshold, NFC strategy, trace mechanism — all expensive to undo |

**Mitigations applied:**
- Comprehensive test coverage (every M1-M6 fixture re-runs under M7 ABI; new positive+negative fixtures per phase) → MEDIUM
- Salsa-isolation of auto-propagation analysis → cache invalidation bounded → MEDIUM
- IR-snapshot tests on every new codegen path → MEDIUM → LOW
- ABI spec doc locked in P0 → SSO drift caught fast

**Rollout plan:** N/A — compiler dev tool, no users yet. Each phase ends with `/pr` per Step 4a Project Shipping Conventions. M7 ships via `/release` cutting tag `v0.1.0-m7`.

---

## Invariants This Milestone Must Preserve

### Safety

- **`errors`-capable values must be checked-or-propagated before re-binding**: assignment to an `errors`-capable variable clears the prior auto-prop state. M7 P3a enforces; negative fixture `m7_errors_reassign_before_handle.ynz`.
- **`.message` access requires the binding to be flow-narrowed to the failed branch via `.failed() === true`**: accessing `.message` on an errors-capable binding is permitted ONLY inside a block where the compiler has narrowed the binding to the "failed" state — i.e., the body of an `if (x.failed())` true-branch, or after an early-return on `!x.failed()`. Direct `err.message` without that proof is rejected. (This precise statement supersedes the earlier "without prior .failed() returning true" wording — what matters is the flow-narrowing into the failed branch, which IS the gate for `.message`/`.suggestions`/`.trace`/`.source` access.) M7 P3a enforces; negative fixture `m7_message_without_check.ynz`. Positive fixture `m7_message_inside_failed_branch.ynz`.
- **Checking after use is a compile error** (matches `design/errors.md` example): `let parsed = parseConfig(raw); if (raw.failed()) {...}` is rejected when `raw` was auto-propagated by `parseConfig(raw)`. M7 P3a enforces; negative fixture `m7_check_after_use.ynz`.
- **`errors` function calling another `errors` function inside non-errors context is rejected**: explicit handler required (`.or()`, `.failed()`, OR wrap caller in `errors`). M7 P3a enforces; negative fixture `m7_unhandled_errors_call.ynz`.
- **`for x in fallible_iter` outside an `errors` function is rejected**: must use `.orSkipFailures()` / `.withErrors()` adapter OR mark caller `errors`. M7 P3c enforces; negative fixture `m7_fallible_iter_no_errors.ynz`.
- **String bracket access out of bounds returns `none`** (not panic): `let c = "ab"[100]` returns `maybe<string>::none`. M7 P3c enforces consistent with existing M5 bracket sugar on collections. Positive fixture `m7_string_index_oob.ynz`.
- **String mutation via bracket is rejected**: `s[0] = "B"` produces the diagnostic per `spec/strings.md`. M7 P3c enforces; negative fixture `m7_string_immutable.ynz`.
- **`next()` on an iterator must take `lend self`**: a user-defined `next(share self: Foo)` doesn't satisfy `follows Iterable<T>`. M7 P3c enforces; negative fixture `m7_next_wrong_ownership.ynz`.
- **`Iterable<T>` instantiation requires concrete T at every for-loop site**: `for x in someFn()` where `someFn() -> Iterable<T>` for unresolved T is rejected. M7 P3c enforces; negative fixture `m7_iter_unresolved_type.ynz`.
- **Iter wrapper holds `share` borrow of source — source cannot be mutated mid-iteration**: `for x in arr { arr.add(...) }` is rejected (M4 borrow rules: cannot `lend` while `share` is outstanding). M7 P3c enforces via existing M4 ownership machinery; negative fixture `m7_iter_mutate_source.ynz`.
- **User-defined iterator shape holding owned self-reference creates a cycle leak (documented v0.1 limitation, mirrors M5's `maybe<Node<T>>` cycle)**: a user writes `shape RecursiveIter follows Iterable<int> { hidden inner: maybe<RecursiveIter> = none, ... }` and constructs a cycle at runtime — same v0.1 borrow-checker gap as M5's maybe-self cycle. Documented; not detected. Fixture `m7_iter_cycle_leak.ynz` exercises and documents the leak. Borrow-checker cycle detection is v0.2+. Cross-reference to M5's same limitation noted.
- **Pre-existing M4 ownership rules apply to iter wrapper field access**: `for x in arr.share { ... }` works (share-of-share is fine); `for x in arr.lend { ... }` works but blocks any other use of `arr` until loop exit. Codegen via existing M4 share/lend infrastructure; no new rules added by M7.
- **`range` stored or returned is now allowed** (M3 catch-up): `let r = range(0, 10); for i in r { ... }` works. Positive fixture `m7_range_first_class.ynz`. M3's "M7 deferral" diagnostic at `crates/ynz-typeck/src/check.rs:300` removed.
- **All M4 ownership invariants carry forward**: `const` bindings still emit `readonly`; iter wrappers' `next(lend self)` emits proper noalias; `errors`-capable values participate in consume-tracking (cannot use after auto-propagation). P4a/P4c IR-snapshot tests assert.
- **All M5 generics invariants carry forward**: monomorphization dedup applies to `ArrayIter<int>` etc. IR-snapshot tests assert.
- **All M6 narrowing invariants carry forward**: `errors`-capable values use the same narrowing infrastructure as `maybe<T>`. M6 rules table is reused; M7 adds the `errors`-specific rows in `design/narrowing.md`.
- **NFC canonical equivalence for `ynz_string_eq`**: M3 catch-up — `"é"` (one code point U+00E9) and `"é"` (two code points: e + combining acute) compare equal. Positive fixture `m7_nfc_equivalence.ynz`. M3 catch-up obligation closed.

### Performance

- **String SSO inline path emits no heap allocation**: every string literal ≤ 23 bytes lowers to a stack/struct-local 24-byte value. IR-snapshot asserts `no ynz_alloc` in the lowering of `let s = "hi"`. M7 P4b enforces.
- **String SSO heap path emits one `ynz_alloc`**: every string literal > 23 bytes lowers to one heap allocation of `len + 1` bytes. IR-snapshot asserts.
- **Concatenation result auto-promotion**: when the result of `s1 + s2` fits ≤ 23 bytes (statically derivable from M1/M2 const-fold), codegen emits inline-form result. When dynamic, uses heap. Tested via `m7_concat_inline.ynz` and `m7_concat_heap.ynz` IR snapshots.
- **SIMD UTF-8 validation**: every string crossing the boundary from external bytes (parser literal validation, file reads, `string.fromBytes()` future API) uses `simdutf8::basic::from_utf8` (SSE4.1 / AVX2 / NEON path, scalar fallback). Runtime benchmark fixture `m7_string_validation_bench.ynz` measures ≥ 1 GB/s on a 1MB ASCII input.
- **SIMD search for `.contains`/`.indexOf`** on patterns ≥ 16 bytes uses SIMD path; ≤ 15 bytes uses scalar `memmem`-style scan. IR-snapshot of two-fixture pair asserts the dispatch.
- **`errors` function entry cost ≤ 10ns** for a no-error path: frame push, body executes, frame pop. Microbenchmark fixture `m7_errors_no_op_bench.ynz` measures.
- **Auto-propagation IR emits one branch** at each first-use site: `error_ptr_is_null ? success_value : early_return_error`. IR-snapshot asserts.
- **`.failed()` lowers to a tag check** (one load + one compare). IR-snapshot asserts.
- **`.message` on a non-failed error is a compile error, NOT a runtime branch**: M7 P3a typeck enforces.
- **For-loop over built-in collection inlines `next()`** in optimized builds: LLVM `alwaysinline` attribute on the synthesized wrapper's `next`. IR-snapshot of optimized output (`-O2`) asserts no `call @next_array_iter` remains.
- **For-loop wrapper alloca + no heap**: `alloca [24 x i8]` for the wrapper struct, no `ynz_alloc`. IR-snapshot asserts.
- **MapEntry<K, V> destructured directly into `let k = ...; let v = ...;`** with no intermediate allocation. IR-snapshot asserts.
- **NFC equality fast path**: if both strings have the `is_nfc_known` cache bit set, equality is a byte-by-byte loop with no normalization work. Cache bit logic locked in P0. Microbenchmark `m7_nfc_fast_path.ynz` measures.
- **All M4/M5/M6 codegen invariants carry forward**: `readonly` attributes, monomorphization dedup, Swiss Tables hash dispatch. IR-snapshots from M4/M5/M6 fixtures all re-pass on the M7 ABI.

**Auto-promotion analysis** (mandatory per `.claude/rules/auto-promotion.md`):

- **Short string SSO auto-promotion**: per `design/strings.md` "Auto-promotion" section. Codegen ALWAYS picks inline storage for strings ≤ 23 bytes. Tier 3 lint NOT applicable (no source-level "make explicit" rewrite). Muted IDE hint deferred to v0.2 LSP: shows `// fits inline — no heap` after a known-short binding. M7 emits the data side-table.
- **`array<T>` → `fixed<T>` promotion from M5 carries forward**, now interacts with iter wrappers: a promoted `fixed<T, N>` for-loop uses `FixedIter<T, N>` wrapper, not `ArrayIter<T>`. M7 P3c handles correctly.
- **String interpolation builder pre-size**: when all `${expr}` segments are statically-bounded length (e.g., int → max 20 chars; bool → max 5 chars), the builder pre-sizes to the upper bound. Codegen auto-promotion only — no source-level form. IR-snapshot asserts.
- **Iter wrapper inline-promotion when `next()` is trivial** (built-in array, fixed, range): LLVM `alwaysinline` codegen attribute. No source-level form — codegen auto-promotion only. IR-snapshot of optimized output asserts.
- **NFC fast-path cache bit on strings** (set when string was constructed from a normalized literal or from prior NFC-known sources): codegen auto-promotion. No source-level form.
- **No new auto-promotion candidates introduced by errors/Frame/.trace** — all error-shape construction is a fixed codegen path; no inferences possible.

### Teaching

M7 adds approximately 40-50 new diagnostic classes. Each invariant below is testable:

- **Every M7 diagnostic follows WHAT/WHAT-INSTEAD/WHY three-part format** — enforced by `Diagnostic` constructor's three-non-empty-field assertion (M1+ carry-forward).
- **Banned jargon stays absent**: `crates/ynz-diagnostics/tests/jargon_audit.rs` confirms `fallible`/`infallible`/`monad`/`lift`/`wrap`/`Result`/`Option`/`Either`/`exception`/`try`/`catch`/`throw`/`UTF-16` don't appear in any user-facing M7 diagnostic. P0 adds new banned-jargon entries.
- **`errors` keyword diagnostics use plain English**: "this function might fail — handle the failure or mark the caller `errors`" — never "fallible callee in non-errors context."
- **Auto-propagation point diagnostic on misuse**: when user writes `if (raw.failed())` AFTER first-use, the diagnostic names BOTH spans (the first-use line, the .failed() line) using Ariadne's related-span feature. `m7_check_after_use.ynz` stderr snapshot asserts.
- **`.message` without `.failed()` check** suggests both alternatives (`.failed()` check, `.or()` fallback): three-part diagnostic with code-suggestion blocks per `m7_message_without_check.ynz`.
- **Unhandled errors call** names the function, says "this function might fail", and lists the three handling options from `spec/errors.md`: mark caller `errors`, use `.or(default)`, check `.failed()` explicitly.
- **String mutation via bracket** suggests the rebuild pattern from `spec/strings.md`: "let newName = "B" + name.substring(1)" OR rebind.
- **String OOB indexing returns `none` (not error)**: a `.value` access on the result triggers the M5 maybe-narrowing diagnostic — already taught. Crossreference noted.
- **`for x in fallible_iter` outside errors function** says "this iteration step can fail" and lists adapter options (`.orSkipFailures()`, `.withErrors()`) AND the mark-caller-errors path.
- **`Iterable<T>` constraint violation** when user shape's `next()` has wrong signature: names the contract, names the concrete-type, names the missing-or-mismatched method signatures.
- **`range` first-class diagnostic for M3-style "range arrives in M7"** REMOVED. Positive fixture asserts the previously-deferred form now compiles.
- **NFC canonical equivalence DEMO** in `examples/pirates-roster/`: shows `"café" == "café"` returning true with a comment explaining NFC. Teaching surface for users who hit Unicode equivalence issues.
- **`.trace` access teaches stack walking**: doc example in `spec/errors.md` shows iterating `err.trace` to print the call chain. Teaching surface for "why did this error happen here?"
- **String interpolation diagnostic when `${expr}` evaluates to a type without `.toString()`**: names the type, suggests `.toString()` or struct literal. Negative fixture `m7_interpolate_no_tostring.ynz`.
- **IDE muted-hint surfaces (iter `.items()` insertion, SSO inline, errors auto-prop point)** — M7 does NOT ship; v0.2 LSP does (per `design/ide-hints.md`). Cross-reference recorded.

### Runtime Dependencies

Per `### Kernel-Mode Behavior` below + `design/future/no-runtime-mode.md`, every M7 feature declares its runtime requirements:

- **String SSO inline (≤ 23 bytes)**: no runtime dependency — stack-resident 24-byte struct.
- **String heap path (> 23 bytes)**: depends on `ynz_alloc` / `ynz_free` (M4 runtime; no new symbol).
- **String concatenation (`+`)**: depends on `ynz_alloc` / `ynz_free`. Adds NEW symbol `ynz_string_concat` (returns 24-byte struct).
- **String equality (`==`)**: depends on NEW `ynz_string_eq` (replaces M1's byte-eq with NFC-aware version) — and indirectly on `unicode-normalization` linked into runtime.
- **String code point access (`s.get(n)`, `s[n]`)**: depends on NEW `ynz_string_codepoint_at` (UTF-8 walk).
- **String byte access (`.byteAt(n)`)**: no NEW symbol — direct memory access into the string struct.
- **String grapheme access (`.graphemeAt(n)`)**: depends on NEW `ynz_string_grapheme_at` (uses `unicode-segmentation` crate).
- **String search (`.contains`, `.indexOf`, `.startsWith`, `.endsWith`)**: depends on NEW `ynz_string_contains` / `ynz_string_index_of` (uses `simdutf8` + SIMD search where available).
- **String case (`.toUpperCase`, `.toLowerCase`)**: depends on NEW `ynz_string_to_upper` / `ynz_string_to_lower` (locale-invariant; uses Unicode case-fold tables).
- **String interpolation**: depends on NEW `ynz_string_builder_*` family OR codegen lowers to a series of `ynz_string_concat` calls. Decision in P0.
- **String validation (`simdutf8::basic::from_utf8`)**: depends on `simdutf8` crate linked into runtime. SIMD intrinsics where available; scalar fallback otherwise.
- **`errors` keyword runtime**: depends on NEW `ynz_error_new`, `ynz_error_drop`, `ynz_error_message`, `ynz_error_suggestions`, `ynz_error_trace`, `ynz_error_source`. AND `ynz_frame_push` / `ynz_frame_pop` (thread-local frame stack).
- **`.or(default)`**: no runtime dependency — codegen lowers to a select instruction.
- **`.failed()`**: no runtime dependency — direct tag check.
- **Iterable wrapper shapes**: stack-resident structs; no runtime dependency for instantiation. `next()` on wrappers may depend on the source collection's runtime symbols (`ynz_array_get`, `ynz_map_iter_next`).
- **`Range` shape**: no runtime dependency — pure arithmetic.
- **`StringCodePointIter`**: depends on `ynz_string_codepoint_at` (string walk).
- **`MapIter<K, V>`** (renamed/clarified from M5's `MapEntry` iteration): depends on M5's `ynz_map_iter_*` symbols (no NEW).
- **`FallibleIterable` adapters (`.orSkipFailures`, `.withErrors`, `.logSkippedFailuresTo`)**: implemented as wrapper shapes in stdlib; depend on `errors` runtime indirectly. `.orSkipFailures` and `.withErrors` are PURE (no new runtime symbol). `.logSkippedFailuresTo(sink)` calls `sink.write(message)` on each skip — `sink` is a `LogSink`-following shape (M7 ships `terminal.stderr` and `terminal.stdout` as the two M7 LogSink-followers).
- **`terminal.stderr` / `terminal.stdout` (`LogSink`)**: NEW runtime symbols `ynz_terminal_stderr_write`, `ynz_terminal_stdout_write` (or one shared `ynz_log_write(fd, ...)`).
- **`file.lines(path)` stub**: depends on libc `fopen`/`fread`/`fclose`. NEW symbol `ynz_file_open_for_lines`, `ynz_file_read_line`, `ynz_file_close`. Stub-quality only — v0.5 will replace.

### Kernel-Mode Behavior

For each M7 runtime dependency above, the `--kernel` mode (v0.3+) behavior is locked:

- **Compile-time-only features** (string SSO inline path, `.failed()`, `.or()`, iter wrapper instantiation, `Range`, string byte access, errors typeck): **always work in `--kernel` mode**. No compile error; no plug-in required.
- **String heap path, concat, all string runtime symbols requiring `ynz_alloc`**: **COMPILE ERROR in `--kernel` mode** unless a custom allocator is provided via the v0.3 plug-in syntax. Error message points to `design/future/no-runtime-mode.md`. Same as M4/M5 heap-allocating types.
- **`ynz_string_eq` with NFC normalization**: depends on `unicode-normalization` which has its own internal allocator use. **COMPILE ERROR in `--kernel` mode** unless an alternative byte-eq comparator is plugged in (per the v0.3 plug-in design). Error message documents the byte-eq escape hatch.
- **`simdutf8` UTF-8 validation**: pure compute (no allocation). **Works in `--kernel` mode** as long as the kernel supports the SIMD instructions; otherwise fallback to scalar (`simdutf8` does this automatically via CPU feature detection — works in kernel mode where CPUID can be queried).
- **String search SIMD**: same as UTF-8 validation — pure compute, works in kernel mode where SIMD is available.
- **`errors` runtime**: `ynz_error_new` allocates the error struct. **COMPILE ERROR in `--kernel` mode** without a plug-in allocator. Trace frame stack is a thread-local — **WORKS in `--kernel`** if the kernel provides a thread-local storage mechanism. P0 documents what the kernel must provide.
- **`ynz_frame_push` / `ynz_frame_pop`**: pure thread-local writes. **WORKS in `--kernel` mode**.
- **`file.lines(path)` stub**: depends on libc file I/O. **COMPILE ERROR in `--kernel` mode** (file I/O not generally available in kernel modules; kernel-specific file APIs would be a v0.5+ FFI binding).
- **`Iterable<T>` over heap-allocated collections**: same `--kernel` rules as the underlying collection (array/map = error unless plug-in alloc; fixed/range = always work).
- **`unicode-segmentation` for grapheme access**: pure compute (no allocation in the call path we use). **Works in `--kernel` mode**.

**Forward declaration to v0.3 plug-in allocator API.** Same as M4/M5/M6: M7 does NOT implement the plug-in mechanism; the kernel-mode compile error for string heap path, errors, file I/O says "kernel-mode disabled" and points to `design/future/no-runtime-mode.md`.

### Demo & Error Gallery

- **`examples/pirates-roster/entrypoint.ynz` MUST be extended in P5** to demonstrate every M7 feature in context:
  - String interpolation in a real message
  - Multi-line strings with interpolation
  - `.contains` / `.indexOf` / `.substring` in a parsing example
  - `for c in "café"` over code points
  - A function declared `-> T errors`, called from an `errors` function with auto-propagation
  - A non-errors function handling an `errors` call with `.or(default)`
  - A function handling explicitly via `.failed()` and `.message`
  - `for i in range(0, 5)` (range first-class — assigned to a variable then iterated)
  - `for entry in scores` (map iteration with default destructuring `entry.key`, `entry.value`)
  - `for (k, v) in scores` (map iteration with tuple destructuring)
  - A user-defined `shape CountDown follows Iterable<int>` with standalone `next()`
  - `for line in file.lines("data.txt")` in an `errors` function (stub-quality file I/O)
  - `.trace` introspection — print the call chain after catching an error in a non-errors caller
- **`examples/primantis-orders/m7_errors.ynz` MUST be created in P5** with intentional triggers for every M7 compile-error class (~40-50 triggers). Each trigger has a `// WHY:` comment naming the diagnostic class.
- **Both files get insta stdout/stderr snapshots** committed. Updating these snapshots requires a `// test-ratchet: <reason>` marker.
- **Patrick must read and sign off on both files before P5 merges** — hands-on UX validation per `.claude/rules/plan-invariants.md`.

---

## Out-of-Scope For This Plan (M7 guardrails)

Restated as the bottom-line guardrail:

- Modules / imports — M8
- Doc comments — M8
- `sensitive` modifier — M8
- Concurrency keyword parsing — M8
- Bignum `number<N>` for N > 34 — M8
- IDE muted-hint surfaces — v0.2 LSP
- Tier 3 lint suggestions — v0.4 lint tier
- Full file system module (`file.read`, `file.write`, `file.exists`, etc.) — v0.5 (M7 ships only `file.lines` stub for FallibleIterable testing)
- HTTP / network / database stdlib — v0.14+
- Regex (`s.find(pattern)` style) — v0.13 (already locked to RE2-style per `.claude/rules/stdlib-design.md` Rule 7)
- Locale-aware `.toLowerCaseLocale(locale)` — v0.5+ stdlib (M7 ships locale-invariant only)
- `string.fromBytes(bytes)` constructor — v0.5+ (M7 strings only constructed from literal source bytes or runtime concat)
- Stack-trace .toString() formatting customization — v0.2+ (M7 ships a default formatter)
- `errors` propagation through closures — v0.3+ (closures don't exist in v0.1)
- `errors` types other than the base shape (typed stdlib errors like `DatabaseError`) — v0.15+ (`design/stdlib/database.md` written against this; not in v0.1 scope)
- Async iteration / `wait` in iterators — v0.3+
- libunwind / backtrace-crate integration — never (we use compile-time emitted frames)
- Higher-kinded `Iterable<F<_>>` style abstractions — NEVER in v0.1

If you find yourself adding code that touches any item above, STOP and either re-plan this milestone or escalate the work to its proper milestone.

---

## M7 Catch-Up Obligations Reference (from M5 and M3 — addressed by this plan)

**M5's REPLACE-AT M7 sites** — all unwound in P3b/P3c/P4c:

1. `crates/ynz-ast/src/nodes.rs:526` — `Type::Range` AST variant → removed; replaced by `Range` user-visible shape
2. `crates/ynz-typeck/src/types.rs:35` — `Type::Range` typeck variant → removed
3. `crates/ynz-typeck/src/check.rs:631` — for-loop typeck special-case → replaced by `Iterable<T>` protocol dispatch
4. `crates/ynz-runtime/src/lib.rs:195` — `ynz_string_eq` byte-equality → replaced with NFC canonical equivalence

**Codegen for-loop special-cases** — all unwound in P4c (~3 sites in `crates/ynz-codegen/src/emit.rs`):
- Line ~1053: BuiltinArray for-loop → emit synthesized `ArrayIter<T>` wrapper + `next()` call
- Line ~1109: BuiltinMap for-loop → emit synthesized `MapIter<K, V>` wrapper + `next()` call
- Line ~1177: Range iteration → use `Range`'s standalone `next()` (Range becomes regular shape)
- BuiltinFixed is folded into the array path with a slight tweak — also unwound

**MapEntry<K, V> synthesis revisit** (M5 carry-over): M7 keeps `MapEntry<K, V>` as a built-in shape, but it's now accessed via `MapIter<K, V>.next()` returning `maybe MapEntry<K, V>`. The user-visible surface (`entry.key`, `entry.value`, or `let (k, v) = entry` destructuring) stays. P3c locks this.

**M3's `range()` first-class deferral** — addressed in P3b: the typeck rejection at `crates/ynz-typeck/src/check.rs:300` for stored Range values is removed. `let r = range(0, 10)` works. `function foo() -> Range { return range(0, 100) }` works.

**M3 NFC equivalence catch-up** — addressed in P4b: `ynz_string_eq` now uses NFC canonical equivalence via `unicode-normalization`.

**M3 share-param deferral fixture** (from `.claude/state.md` line 95 "M4 catch-up obligations from M3"): the `m3_share_param_deferral.ynz` stderr snapshot must be updated when `share` parameter ownership works — this was already done in M4 P3c. Verify still up-to-date in P6.

**M2/M3 fallible-string-conversion catch-up** — already closed in M6 (`.toInt()`, `.toFloat()`, `.toNumber()` on strings ship as `maybe<T>` per M6 plan). M7 verifies no regression.

---

## Roadmap (this milestone's phase structure)

11 phases. Each phase is one PR landing on `main` via `/pr`.

**Ships via** (per Step 4a):
- **Per-phase**: `/pr` (project-local pr skill detected; uses project `pr` skill in `.claude/skills/pr/`)
- **Per-milestone**: `/release` (project-local release skill detected)

| Phase | Theme | Est. lines | Branch |
|---|---|---|---|
| P0 | Doc lockdown + design questions answered | ~600 (docs only) | `chore/m7-doc-lockdown` |
| P1 | Lexer + AST scaffolding (`errors`, backticks, interpolation, iter contracts) | ~400 | `feat/m7-lexer-ast` |
| P2 | Parser (`errors` return, backtick interpolation, multi-line, range first-class) | ~500 | `feat/m7-parser` |
| P3a | Typeck — `errors` flow-sensitive auto-propagation | ~700 | `feat/m7-typeck-errors` |
| P3b | Typeck — strings full (interpolation, new methods, code-point iter) | ~500 | `feat/m7-typeck-strings` |
| P3c | Typeck — Iterable<T>/FallibleIterable<T>, iter wrappers, range first-class, unwind M5 markers | ~600 | `feat/m7-typeck-iterables` |
| P4a | Codegen — errors runtime, cascade emission, frame stack | ~600 | `feat/m7-codegen-errors` |
| P4b | Codegen — string runtime (SSO, SIMD, all methods, NFC eq) | ~900 | `feat/m7-codegen-strings` |
| P4c | Codegen — Iterable protocol dispatch, unwind M5/M3 codegen markers | ~500 | `feat/m7-codegen-iterables` |
| P5 | Fixtures + demo + error gallery + audit | ~400 (mostly fixtures) | `feat/m7-fixtures-demo` |
| P6 | Verify + tag v0.1.0-m7 | ~50 (version bump + changelog) | `chore/m7-release` |

**Total estimate: ~5750 lines across 11 PRs.** Comparable to M5 (~5000 lines, 8 PRs) and M6 (~4200 lines, 8 PRs).

**New-test budget by phase** (each phase's acceptance gate must hit ≥ its budget):

| Phase | New tests minimum | Cumulative |
|---|---|---|
| P0 (docs only) | 0 | 631 (M6 baseline) |
| P1 | 8 (lexer + AST variants) | 639 |
| P2 | 15 (parser positive + negative) | 654 |
| P3a | 25 (errors typeck — 14 Safety invariants + 11 positive cases) | 679 |
| P3b | 25 (strings typeck — methods × positive + interpolation + bracket sugar + iter-over-string) | 704 |
| P3c | 25 (Iterable + FallibleIterable typeck + REPLACE-AT M7 sites + range first-class + user iter) | 729 |
| P4a | 15 (errors runtime + frame stack + cascade) | 744 |
| P4b | 25 (string runtime — SSO + SIMD + NFC + every method) | 769 |
| P4c | 15 (iter codegen + adapters + range first-class binary) | 784 |
| P5 | 20 (cross-feature + 10 adversarial fixtures) | 804 |
| P6 | 0 (verification only) | 804 |

**Target total: ≥ 800 tests** (revised up from initial ≥ 750 to ensure adversarial coverage). M6 baseline was 631; M7 adds ~170 new tests.

---

## Phases

### Phase 0: Doc lockdown + design questions answered
**PR scope**: Lock all M7 design questions; update spec + design docs to coherent state for the implementation phases. No code changes.
**Branch**: `chore/m7-doc-lockdown`
**Flag**: N/A
**Est. lines**: ~600 (docs only)
**Ships via**: `/pr`
**Objective**: Eliminate all open design questions BEFORE any code lands. M7 implementation phases must not stall on unresolved design.
**Why this phase exists**: SSO layout, NFC strategy, trace mechanism, error shape — these affect every code phase. Locking them in doc form, with concrete decision tables and ABI specs, prevents drift mid-implementation.

**Current-state anchors**:
- `design/strings.md` — has SSO and SIMD sections; needs the locked exact bit layout written out + NFC fast-path cache bit semantics
- `design/errors.md` — has flow-sensitive rules; needs frame-stack mechanism documented + Frame shape spec
- `design/iterables.md` — has contract shapes; needs the synthesized wrapper mechanism documented + adapter logging semantics
- `spec/strings.md`, `spec/errors.md`, `spec/iterables.md` — user-facing surface needs the M7-locked decisions reflected
- `design/narrowing.md` — needs new rows for `errors`-capable values

**Files (expected scope)**:
- `design/strings.md` — SSO exact bit layout, NFC cache bit, SIMD crate choice (simdutf8 vs portable_simd)
- `design/errors.md` — Frame shape spec, frame stack mechanism, trace truncation rule
- `design/iterables.md` — synthesized wrapper shapes, adapter logging semantics, MapEntry destructuring forms
- `design/narrowing.md` — rows for `errors` auto-propagation (matches `maybe<T>` patterns)
- `design/stdlib/strings.md` — NEW FILE if not present (per-method documentation for the M7 string API)
- `spec/strings.md`, `spec/errors.md`, `spec/iterables.md` — updates per locked decisions
- `.claude/plans/active/v0-1-compiler.md` — M7 paragraph refreshed with locked decisions
- `.claude/state.md` — Active Decisions append (per global rule)
- `.claude/rules/vocabulary.md` — banned-jargon additions (`monad`, `lift`, `wrap`, etc.)
- `crates/ynz-diagnostics/src/banned_jargon.rs` — extend the const slice (this file is doc-aligned banned terms; counts as P0 doc work)

**Deviation rule**: Standard. P0 may touch additional design files if a discovered ambiguity blocks subsequent phases. Each deviation noted in the PR description.

**Steps**:
1. **Lock SSO 24-byte layout** in `design/strings.md`. The compact_str-style scheme: ALL 24 bytes are either three i64s (heap form) or 23 data bytes + tag byte (inline form). The TAG BYTE LIVES AT THE LAST OFFSET (byte 23) and serves as the discriminator AND the inline length AND flag carrier. This works because UTF-8 source bytes cannot produce a final byte in the 0xC0..0xFF range when valid (and UTF-8 SHOULD never have a final continuation byte in 0x80..0xBF as the LAST byte of a complete string — but to be safe we pick a tag-byte range that is unambiguous).

   **Locked layout**:

   | Byte offset | Inline form (tag byte at offset 23 has bit 7 = 1) | Heap form (high byte of `cap` at offset 23 has bit 7 = 0) |
   |---|---|---|
   | 0..7 (8 bytes) | data[0..7] | `ptr: *u8` (8-byte aligned) |
   | 8..15 (8 bytes) | data[8..15] | `len: i64` |
   | 16..22 (7 bytes) | data[16..22] | low 7 bytes of `cap: i64` |
   | 23 (1 byte) | tag: `0x80 \| (length & 0x1F) \| (is_nfc_known << 6)` | high byte of `cap: i64`, top bit = 0 |

   **Tag-byte breakdown (inline form, offset 23)**:
   - Bit 7 (0x80): inline-discriminator flag — always 1 for inline.
   - Bit 6 (0x40): `is_nfc_known` — 1 if string is known NFC-normalized.
   - Bits 5..0 (0x1F mask): inline length, range 0..23.

   **Cap budget (heap form)**: cap is a 64-bit value but its top bit MUST be 0 to keep byte 23 in the 0x00..0x7F range — distinguishable from inline tag (0x80..). This caps maximum heap string capacity at 2^63-1 bytes ≈ 9.2 exabytes. Acceptable.

   **`is_nfc_known` for heap form**: stored in bit 1 of the `len: i64` field. Lengths < 2^62 leave the top 2 bits free. Bit 0 of `len` reserved for future use (zero). This avoids needing a separate header byte and keeps len arithmetic clean (just mask before use).

   **Worked examples**:
   - `"hi"`: inline, length 2, NFC-known. Bytes 0..1 = 'h','i'; bytes 2..22 = zero-fill; byte 23 = `0x80 | 0x40 | 0x02 = 0xC2`.
   - 30-char ASCII literal: heap. Bytes 0..7 = ptr; bytes 8..15 = `len = 30 | (1 << 1)` for NFC-known = `30 | 2 = 32`; bytes 16..23 = `cap = 30`.

   Compiler emits accessor inlines that read byte 23, branch on bit 7, and dispatch to inline or heap path. ABI-locked per `stdlib-design.md` Rule 7.

   **Compile-time verification**: `crates/ynz-runtime/tests/string_layout.rs` asserts `mem::size_of::<YnzString>() == 24` AND `mem::align_of::<YnzString>() == 8`. Bit-pattern test: construct a known inline string + a known heap string and assert byte-by-byte the expected pattern.

2. **Lock NFC-known propagation table**. For every string-producing operation, define whether the result has `is_nfc_known = true`:

   | Operation | Result `is_nfc_known`? | Why |
   |---|---|---|
   | String literal (parser) | TRUE — compiler pre-normalizes at compile time | Lock the parser-side NFC pass |
   | Backtick-interpolation result | TRUE only if every segment was NFC-known | One non-NFC segment poisons the result |
   | `s1 + s2` (`ynz_string_concat`) | TRUE only if BOTH s1 and s2 were NFC-known AND s1's last code point is NOT a base-character that could recompose with s2's first code point if it were a combining mark | Subtle edge: `"e" + "́"` produces NFD output even though both operands are individually NFC-normal (combining marks recompose with their bases). Conservative implementation: when concat detects s2 starts with a combining-class code point, force `is_nfc_known = false`. Optimization opportunity for v0.2+: only force false when the recomposition would actually fire (~99% of concats won't hit a combining mark first byte) |
   | `.substring(start, end)` | TRUE if source was NFC-known AND boundaries are code-point boundaries | Substring of NFC is NFC |
   | `.trim()` | TRUE if source was NFC-known | Trim is byte-level on whitespace; preserves NFC |
   | `.split(sep)` | TRUE for each piece if source was NFC-known | Split is byte-level; preserves NFC per piece |
   | `.replace(old, new)` | TRUE only if source AND `new` were both NFC-known | Inserts `new` text — must be NFC too |
   | `.toUpperCase()` / `.toLowerCase()` | **FALSE** — case folding produces NFD code points in many cases (Turkish-I, Greek, etc.) | Cannot assume NFC; force slow path |
   | `.toString()` on primitives (int → "42", bool → "true", etc.) | TRUE | ASCII only |
   | `.toString()` on user shape | UNKNOWN — set to FALSE conservatively unless shape declares a `nfc-known` annotation | Safest default; user can opt in |
   | `string.fromBytes(bytes)` (v0.5+ — deferred) | FALSE always | Runtime byte input may not be normalized |
   | `ynz_string_codepoint_at(s, n).value` (single code point) | FALSE | A single code point in isolation cannot be verified NFC without context; cheaper to assume false |

   `ynz_string_eq` fast path: both sides have `is_nfc_known = true` → byte-compare. Slow path: normalize-both-via-`unicode-normalization::nfc()`-then-byte-compare. Document the table in `design/strings.md`.

3. **Pin exact crate versions** in `Cargo.toml` workspace deps in P0 text (actual file edit deferred to P4b but the PINNED versions are locked here): `simdutf8 = "=0.1.4"`, `unicode-normalization = "=0.1.24"`, `memchr = "=2.7.4"` (for SIMD-accelerated string search). Verify each version is available on crates.io at P0 time; if not, lock to the latest stable equivalent. Crates pinned with `=` to prevent surprise upgrades during late-phase debugging.

4. **Lock SIMD crate choice**: `simdutf8` (Rust port of simdjson's UTF-8 validator). MIT/Apache-2.0 licensed. Runtime CPU feature detection with scalar fallback. Used for: literal validation at parse time, runtime byte-to-string construction, and as a building block for `.contains` long-pattern path. Document the decision with rationale: vetted, maintained, scalar fallback baked in, no transitive deps.

5. **Lock Frame shape** in `design/errors.md` and add row to `spec/errors.md`. Fields: `file: string, line: maybe int, function: string`. **`line` is `maybe int`** (not `int`) so the truncation-sentinel frame can use `none` instead of a magic -1 value (closes reviewer Required Fix #9 + `comments.md` ambiguous-sentinel rule). Real frames have `line: <one-based positive int>`. Truncation sentinel: `Frame { file: "<trace truncated at depth 1024>", line: none, function: "<...>" }`. **Frame.line CONTRACT**: one-based; matches the compiler's diagnostic line numbering exactly. Tools integrating with `.trace` MUST treat this as one-based; do not subtract 1 to map to LSP zero-based positions — convert at the tool boundary instead. Document in `spec/errors.md`.

6. **Lock frame-stack mechanism**: thread-local `array<Frame>` capped at 1024. Document overflow behavior (silent truncation; the 1024th push records the LAST useful frame; the 1025th push and onward are dropped; when the error surfaces, an additional sentinel Frame with `line: none` is appended to the trace marking the truncation point).

7. **Lock trace capture timing**: `ynz_frame_push` at the START of every `errors` function body; `ynz_frame_pop` before every return path; `ynz_error_new` snapshots the current frame stack. Auto-propagation re-uses the snapshot (no re-capture). Document.

8. **Lock SourceLoc vs Frame distinction**: `SourceLoc { file: string, line: maybe int }` is the "where did this error originate" record carried by `ErrorBaseShape.source`. `Frame { file: string, line: maybe int, function: string }` is one entry in the call-chain `.trace`. They're distinct because `.source` answers "where did THIS specific failure originate" (often the leaf-call's site), while `.trace[0..N]` is the full call chain. `Frame` has `function` because the call chain needs function names to be useful; `SourceLoc` doesn't because the leaf failure site is one specific line/file pair without a function-name context that isn't already in the trace's top frame. Document both shapes in `design/errors.md` AND `spec/errors.md`.

9. **Lock synthesized iterator wrapper mechanism** in `design/iterables.md`. Document the four built-in wrappers: `ArrayIter<T>`, `FixedIter<T, N>`, `MapIter<K, V>`, `StringCodePointIter`. Document that `Range` is a user-visible shape (not synthesized; ships in `design/iterables.md` as a worked example). Document that user shapes following `Iterable<T>` are not wrapped — they ARE the iterator. Document the codegen contract: wrappers are stack-allocated; `next()` is always-inlinable for built-ins.

10. **Lock contract instantiation for built-in wrappers**:
    - `ArrayIter<T> follows Iterable<T>` — next returns `maybe T`.
    - `FixedIter<T, N> follows Iterable<T>` — next returns `maybe T`.
    - `MapIter<K, V> follows Iterable<MapEntry<K, V>>` — next returns `maybe MapEntry<K, V>`. The T-of-the-contract is explicitly `MapEntry<K, V>`, not `K` or `V`.
    - `StringCodePointIter follows Iterable<string>` — next returns `maybe string` (one code point as a 1-character string).
    - `Range follows Iterable<int>` — next returns `maybe int`.
    - User shapes write `function next(lend self: TheirShape) -> maybe T` (or `maybe T errors` for FallibleIterable).
    Document the contract-T resolution table in `design/iterables.md`.

11. **Lock adapter logging semantics — `.orSkipFailures()` is PURE** (no I/O side effects). The previous draft had `.orSkipFailures()` log each skip to stderr; that violates `stdlib-design.md` Rule 1 (pure-named methods must be pure). REVISED: `.orSkipFailures()` silently drops failed iterations and continues. For users who want logging, ship a separate composable builder `.logSkippedFailuresTo(sink)` which takes a `LogSink` (initially just `terminal.stderr` and `terminal.stdout` in M7; expandable in v0.5+ stdlib). The user composes them: `iter.logSkippedFailuresTo(terminal.stderr).orSkipFailures()`. Two methods, two explicit names, no hidden side effects. Lock the `LogSink` shape spec in P0:
    ```
    shape LogSink {
        write(lend self, message: string) -> nothing
    }
    ```
    `terminal.stderr` and `terminal.stdout` follow `LogSink`. Document in `spec/iterables.md`.

12. **Lock `.withErrors()` return type — Iterable<maybe T errors>, NOT a new Result<T> shape**. The previous draft proposed `Iterable<Result<T>>` with `Result<T>` defined as a tagged shape. `Result` is on the banned-jargon list per `vocabulary.md` (and M7's P0 adds it explicitly). REVISED: `.withErrors()` returns `Iterable<maybe T errors>` — each iteration step yields an errors-capable maybe-value that the user inspects with the standard `.failed()` / `.message` / `.or()` machinery from `errors`-context. No new shape; reuses M7's own errors-capable mechanism uniformly. Example:
    ```ynz
    for (result in file.lines(path).withErrors()) {
        if (result.failed()) {
            log.warn(`bad line: ${result.message}`)
        } else {
            // result auto-narrows to maybe string via M7 errors narrowing rules
            if (result.value.exists()) {
                process(result.value.value)
            }
        }
    }
    ```
    Document in `design/iterables.md` AND `spec/iterables.md`. Update `design/errors.md` if it referenced the old `Result<T>` path.

13. **Lock MapEntry destructuring forms**: `for (entry in m) { entry.key; entry.value }` (single-binding) AND `for ((k, v) in m) { ... }` (tuple-destructure). Both desugar identically at codegen. Document.

14. **Lock string iteration default**: `for c in "café"` steps by CODE POINT (matches `s.get(n)` default). Document that grapheme iteration is opt-in via `.graphemes()` returning `Iterable<string>` (deferred to v0.5+ — for now, `.graphemeAt(n)` is the only grapheme access).

15. **Lock multi-line string escape rules**: backtick-quoted strings interpret `\n`, `\t`, `\\`, `\``, `${`, but NOT `\u{...}` (deferred to v0.5+). Document.

16. **Lock interpolation expression evaluation semantics**: `` `${x}-${x}` `` evaluates `x` ONCE per occurrence in source (TWO evaluations for two `${x}`s). Repeated identical expressions are NOT memoized. This matches user-written `x.toString() + "-" + x.toString()` — two evaluations. If `x` has side effects, both fire. Document in `spec/strings.md` with a worked example.

17. **Add banned-jargon entries** to `.claude/rules/vocabulary.md` and `crates/ynz-diagnostics/src/banned_jargon.rs`: `monad`, `lift`, `wrap` (`unwrap` was already added by M5/M6); `Result`, `Option`, `Either`, `exception`, `try`, `catch`, `throw`. Verify M6's earlier additions (`fallible`, `infallible`) are still present.

18. **Update `.claude/plans/active/v0-1-compiler.md`** M7 paragraph to reflect locked decisions.

19. **Update `.claude/state.md`** Active Decisions with the four pre-draft locks AND the locked items above.

20. **Update `design/narrowing.md`** with new rows for `errors`-capable values (mirror the `maybe<T>` rules; the auto-propagation is the M7 analog of `.exists()` check).

21. **Create OR extend `design/stdlib/strings.md`** — unconditional. If the file already exists, extend with M7 sections; if not, create it. Sections required: per-method documentation for `.contains`, `.indexOf`, `.startsWith`, `.endsWith`, `.toUpperCase`, `.toLowerCase`, `.substring`, `.trim`, `.split`, `.replace`, `.byteAt`, `.get`, `.graphemeAt`, `.count`, `.byteCount`, `.graphemeCount`. Lock each method's signature (return type, fallibility, ownership).

22. **Lock case-folding crate decision — `unicode-normalization` is NOT sufficient for case folding; add `unicase = "=2.7.0"` OR vendor case-fold tables**. `unicode-normalization` provides NFC/NFD/NFKC/NFKD only — NOT case-folding. For `.toUpperCase()` / `.toLowerCase()` (locale-invariant): pick ONE: (a) `unicase` crate (small, well-maintained, Unicode case-folding tables, no transitive deps beyond `version_check`); OR (b) vendor the Unicode CaseFolding.txt tables ourselves (one-time generator script + 30KB of static tables). **LOCKED: option (a) — pin `unicase = "=2.7.0"`** unless P0 verification finds licensing issues (MIT/Apache-2.0 expected). Document in P0; add to Cargo.toml pins. NOT "executor decides at code time" — locked here.

23. **Lock SIMD fallback CI requirement — pin to P4b**: CI runs on x86_64 (current). P4b adds a second CI job that disables SIMD intrinsics via `RUSTFLAGS=-C target-feature=-sse4.1,-avx2` to exercise the scalar fallback path. Both must pass. Cross-referenced in the Risk table mitigation row + P4b acceptance criteria.

24. **Lock runtime representation of `maybe int` (the type of `Frame.line`)**: use M5's existing maybe-int encoding (locked in M5 P3b/P4a). M5 lowers `maybe<int>` to `{ tag: i8 (0 = none, 1 = some), value: i64 }` for general use — `Frame.line` uses the same. P4a step 2 `Frame` struct layout is therefore `{ file: *u8, line: { tag: i8, padding: 7 bytes, value: i64 }, function: *u8 }` = `8 + 16 + 8 = 32 bytes` (16-byte aligned). NOT a sentinel-int — the tag byte makes "none" unambiguous. This is the same machinery M5 ships, no new representation. Document the cross-reference in P4a step 2.

**Acceptance criteria** (observable conditions that define DONE):
- [x] `design/strings.md` has a "M7 SSO Layout" subsection with the bit layout written out
- [x] `design/errors.md` has a "M7 Frame Stack" subsection with the mechanism written out
- [x] `design/iterables.md` has a "M7 Synthesized Wrappers" subsection
- [x] `design/narrowing.md` has new rows for `errors`-capable narrowing
- [x] `design/stdlib/strings.md` exists with per-method signatures
- [x] `.claude/rules/vocabulary.md` banned-jargon table updated with M7 entries
- [x] `crates/ynz-diagnostics/src/banned_jargon.rs` const slice extended (matches vocabulary)
- [x] `.claude/plans/active/v0-1-compiler.md` M7 paragraph reflects locked decisions
- [x] `.claude/state.md` Active Decisions has the locks
- [x] No design file says "M7 will decide" or "TBD" about any locked-in-P0 question — all questions answered

**Quality gate** (check BEFORE moving to next phase):
- [x] Every locked decision in P0 has a one-paragraph rationale matching the format of M5/M6 design locks
- [x] No locked decision contradicts existing design docs
- [ ] Patrick reads + signs off on the P0 merge

**Verification**: `cargo test --workspace` (no test changes; should pass unchanged); `git diff` review of doc changes; manual read-through of `design/strings.md`, `design/errors.md`, `design/iterables.md`, `design/narrowing.md` for consistency.

---

### Phase 1: Lexer + AST scaffolding
**PR scope**: Add tokens, AST nodes, and Type variants for `errors` keyword, backtick-quoted strings, string interpolation, range first-class, iterator contract shapes. No typeck, no codegen logic.
**Branch**: `feat/m7-lexer-ast`
**Flag**: N/A
**Est. lines**: ~400
**Ships via**: `/pr`
**Objective**: Get the AST shape right so P2 parser + P3 typeck have a stable foundation. AST variant counts locked.
**Why this phase exists**: Token + AST changes ripple across every later phase. Get them right in one PR; downstream phases can move fast on top.

**Current-state anchors**:
- `crates/ynz-parser/src/lexer.rs` — current token set (M6 ended at 65+ tokens; verify count in P1)
- `crates/ynz-ast/src/nodes.rs` — current AST shape (`Expr` 16 variants, `Stmt` 9 variants, `Type` 16 variants; verify counts in P1)
- `crates/ynz-ast/src/tokens.rs` — current Tok enum
- `crates/ynz-diagnostics/src/banned_jargon.rs` — already extended in P0

**Files (expected scope)**:
- `crates/ynz-parser/src/lexer.rs` — handle backtick-quoted strings; recognize `errors` keyword (already reserved); track `${` / `}` interpolation boundaries (state-machine extension for string-interp mode)
- `crates/ynz-ast/src/tokens.rs` — new `Tok` variants: `BacktickString`, `InterpolationStart` (`${`), `InterpolationEnd` (`}`), `Errors`
- `crates/ynz-ast/src/nodes.rs` — new `Expr` variants: `InterpolatedString(Vec<StringPart>, SourceSpan)` where `StringPart = Lit(Vec<u8>) | Expr(Box<Expr>)`; new `Type` variant: `ErrorCapable { inner: Box<Type> }` (or extend `Type::Maybe` to share machinery)
- `crates/ynz-ast/src/nodes.rs` — `FunctionDecl` gains `errors_capable: bool` field on return type
- `crates/ynz-ast/src/nodes.rs` — remove `Type::Range` (REPLACE-AT M7 marker site 1) — but the AST variant stays as a no-op until P3b removes it cleanly (alternative: rename to `Type::RangeDeprecated` and remove in P3c)
- Tests: `crates/ynz-parser/tests/lexer.rs` — backtick + interpolation tokenization; `crates/ynz-ast/tests/variants.rs` — variant-count assertions updated with `// test-ratchet: M7 P1 adds N variants`

**Deviation rule**: Standard. P1 may need to revisit `Tok` enum if existing parser code conflicts with new variants.

**Steps**:
1. Verify current token / variant counts. Document pre-P1 numbers in PR description.
2. Add new `Tok` variants. Update `Tok::variant_count` test if present.
3. Implement backtick-string tokenization in lexer. Inside backticks: track `${` → `InterpolationStart`, then re-enter regular tokenization, until `}` → `InterpolationEnd`, then return to backtick-string mode. Newlines and escapes per P0 locked rules.
4. Verify `errors` keyword is already reserved (M2-era ban). Add explicit acceptance as a return-type modifier keyword (NOT an identifier). Add lexer test.
5. Add new `Expr` variants: `InterpolatedString(Vec<StringPart>, SourceSpan)`. Define `StringPart` enum: `Lit(Vec<u8>, SourceSpan) | Expr(Box<Expr>, SourceSpan)`.
6. Add `Type::ErrorCapable { inner: Box<Type> }`. This wraps any `Type` to mark it as auto-propagating. (Internally, `errors`-capable maps to `{success_value, error_ptr}` representation — but typeck and codegen treat it as a marker type.)
7. Extend `FunctionDecl` return type representation: a function declaration's return type is `Type` AND an `errors_capable: bool` flag. (Alternatively: wrap in `Type::ErrorCapable` at parse time. Decision in step 1.)
8. Add lexer + AST tests for new variants. Snapshot tests for tokenization of representative examples.
9. Update `crates/ynz-ast/tests/variants.rs` (if present) variant-count assertions with `// test-ratchet: M7 P1 adds N variants — Expr, Type, Tok` and a one-line WHY comment.
10. **Range AST cleanup is DEFERRED to P3c** — leaving the variant in place during P1 to avoid breaking M3-M6 fixtures.

**Acceptance criteria**:
- [ ] All M6 tests still pass (no parser/AST regression)
- [ ] New tests for backtick tokenization green
- [ ] New tests for interpolation tokenization green
- [ ] New tests for `errors` keyword recognized as return-type modifier (not as identifier) green
- [ ] Variant-count test ratchet markers in place with one-line WHY

**Quality gate**:
- [ ] No `// REPLACE-AT` markers introduced (only resolved later)
- [ ] No banned jargon in user-facing diagnostic strings
- [ ] Existing parser regression tests all pass

**Verification**: `cargo test --workspace --no-fail-fast`; `cargo clippy --workspace -- -D warnings`; manual review of new variant lists.

---

### Phase 2: Parser
**PR scope**: Parser produces AST for `errors`-return functions, backtick-interpolated strings, multi-line strings, and range-as-first-class. No typeck or codegen logic.
**Branch**: `feat/m7-parser`
**Flag**: N/A
**Est. lines**: ~500
**Ships via**: `/pr`
**Objective**: Every M7 source form parses to a typeable AST. Negative inputs produce three-part diagnostics.
**Why this phase exists**: Decoupling parser from typeck keeps the diff reviewable. Parser changes touch grammar; typeck changes touch semantics — different concerns.

**Current-state anchors**:
- `crates/ynz-parser/src/parser.rs` — current recursive-descent parser
- `crates/ynz-parser/src/parser/expr.rs` (if split) — current expr parser
- `crates/ynz-parser/src/parser/type_.rs` (if split) — current type parser

**Files (expected scope)**:
- `crates/ynz-parser/src/parser.rs` — `parse_return_type()` extended for `T errors` and `maybe T errors`; `parse_string_literal()` extended for backtick + interpolation
- `crates/ynz-parser/src/parser.rs` — `parse_for_loop()` accepts pattern destructure `for ((k, v) in m)` (or `for (entry in m)`)
- `crates/ynz-parser/tests/expr.rs` — interpolation parser tests
- `crates/ynz-parser/tests/function.rs` — `errors` return type parser tests
- `crates/ynz-parser/tests/parser_for_loop.rs` — pattern-destructure for-loop tests

**Deviation rule**: Standard. P2 may need to extend pattern-matching syntax beyond MapEntry destructuring if a discovered ambiguity makes it cleaner.

**Steps**:
1. Parser: `function foo() -> T errors { ... }`. The `errors` token after the return type sets `errors_capable: bool = true`. Test with all M6 return-type forms: `T`, `maybe T`, `T | U`, etc.
2. Parser: backtick-string body. Sequence of `StringPart`s. Lexer hands tokens in order; parser assembles into `InterpolatedString`.
3. Parser: multi-line backtick strings — newline allowed inside; escapes per P0.
4. Parser: pattern-destructure for-loop `for ((k, v) in m) { ... }` — recognize the tuple pattern after `for (`. Desugar to `for (entry in m) { let k = entry.key; let v = entry.value; ... }`.
5. Parser: `range(...)` no longer needs a special-case parse — it's a regular call expression. Existing `parse_call()` handles it. The M3 typeck-side "Range cannot be stored" check goes away in P3c.
6. Parser: `is` narrowing on errors-capable values doesn't exist — `errors`-capable is checked via `.failed()` method, not `is` type check. Verify parser doesn't accept `if (x is errors)` (this should already fail at typeck per M6 rules).
7. Add parser tests for every new form. Negative tests with three-part diagnostics for: malformed interpolation (`${` without matching `}`), `errors` after a non-type position, unclosed backtick.
8. Snapshot AST output for representative fixtures.

**Acceptance criteria**:
- [ ] `function foo() -> string errors { return "ok" }` parses; AST has `errors_capable: true`
- [ ] `` `Hello ${name}` `` parses as `InterpolatedString([Lit("Hello "), Expr(name)])`
- [ ] Multi-line backtick strings parse with newlines preserved
- [ ] `for ((k, v) in m)` parses with the tuple-destructure form
- [ ] `let r = range(0, 10)` parses (no parser error; typeck currently rejects, will accept in P3c)
- [ ] All M6 parser tests still pass

**Quality gate**:
- [ ] Negative tests for every new error class (3 minimum per new form)
- [ ] No new tokens emitted by lexer that the parser doesn't consume

**Verification**: `cargo test -p ynz-parser`; `cargo clippy --workspace -- -D warnings`; insta snapshot review.

---

### Phase 3a: Typeck — `errors` flow-sensitive auto-propagation
**PR scope**: Implement `errors` keyword in typeck. Auto-propagation analysis at first-use site. `.failed()`, `.message`, `.suggestions`, `.trace`, `.source`, `.or(default)` method dispatch.
**Branch**: `feat/m7-typeck-errors`
**Flag**: N/A
**Est. lines**: ~700
**Ships via**: `/pr`
**Objective**: Every `errors`-capable function compiles with correct narrowing. Every misuse produces a three-part diagnostic. Same narrowing infrastructure as M6's `maybe<T>`.

**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs` — current typeck (M6 narrowing for maybe + unions lives here)
- `crates/ynz-typeck/src/narrowing.rs` (if present; else inline in check.rs) — M6 narrowing infrastructure
- `crates/ynz-typeck/src/types.rs` — current Type enum
- `design/errors.md` — flow-sensitive auto-propagation rules

**Files (expected scope)**:
- `crates/ynz-typeck/src/check.rs` — add ErrorCapable narrowing analysis; method dispatch for `.failed()`/`.message`/etc.
- `crates/ynz-typeck/src/types.rs` — add `ErrorBaseShape` shape (the public-facing error type with `.message`, `.suggestions`, `.trace`, `.source`); add `Frame` shape
- `crates/ynz-typeck/src/types.rs` — add `errors_capable: bool` on FunctionSig (or wrap return in `Type::ErrorCapable`)
- `crates/ynz-typeck/src/narrowing.rs` (or inline) — extend narrowing facts to include `ErrorChecked(binding_name)` analogous to `MaybeExists(binding_name)`
- `crates/ynz-typeck/tests/errors.rs` — NEW comprehensive errors typeck test file (~30 cases)

**Deviation rule**: Standard.

**Steps**:
1. Define `ErrorBaseShape` as a built-in shape (added to `ShapeTable` at typeck init). Fields per `design/errors.md`: `message: string`, `suggestions: array<string>`, `trace: array<Frame>`, `source: SourceLoc`.
2. Define `Frame` as a built-in shape: `file: string, line: int, function: string`. Define `SourceLoc` as a built-in shape: `file: string, line: int`.
3. Implement narrowing fact `ErrorChecked(binding_name)`. Same shape as M6's `MaybeExists` fact. Cleared on reassignment, on `lend` call (conservative).
4. At a call site to an `errors` function:
   - Inside an `errors` caller: produce an `ErrorCapable<T>` value. Track per-binding "auto-prop pending" status (active until first non-`.failed()`/`.message`/`.suggestions`/`.trace`/`.source` use).
   - Inside a non-errors caller: produce an `ErrorCapable<T>` value. REQUIRE explicit handling (`.or()`, `if (x.failed()) {...}`, etc.) before the variable is used as `T`. If not handled by end-of-scope: compile error with three-part diagnostic naming the call site + handler options.
5. At first non-error-inspection use of an auto-prop-pending binding inside an `errors` caller: insert auto-propagation marker (typeck-side annotation that P4a will lower to early-return IR). Clear the "auto-prop pending" flag; narrow the binding to plain `T` from this point.
6. After auto-prop, accessing `.failed()` / `.message` / `.suggestions` / `.trace` / `.source` on the binding produces the "check after use" compile error.
7. `.or(default: T)` returns `T` (unwrapped); clears auto-prop pending; narrows binding to `T`.
8. `.failed() -> bool` is the gate for explicit handling. After `if (x.failed()) { return defaultPath }`, the rest of the block narrows `x` to `T` (just like `if (!m.exists()) { return } m.value` from M6).
9. Implement all narrowing rules from `design/errors.md` AND extend rows in `design/narrowing.md`.
10. Add comprehensive typeck tests:
    - Happy path: errors function calls errors function, auto-propagates
    - Explicit handle: `.or(default)`
    - Explicit handle: `if (x.failed()) { return defaultValue }`
    - Negative: unhandled errors call in non-errors function
    - Negative: `.message` without `.failed()` check
    - Negative: check after use (the canonical example from `spec/errors.md`)
    - Negative: `.failed()` after auto-prop
    - Negative: re-binding clears the narrowing
    - Negative: nested errors call inside an `errors` function — verify auto-prop chains correctly
    - Field access on `errors`-capable: `errCapable.message` requires prior `.failed()` ⇒ true
    - `Frame` field access: `err.trace[0].file` works
11. Update diagnostic strings — verify no banned jargon. Run `cargo test -p ynz-diagnostics --test jargon_audit`.

**Acceptance criteria**:
- [ ] Happy path fixture: `errors` function calling `errors` function compiles + has correct auto-prop IR markers (typeck-side; codegen ships P4a)
- [ ] Explicit handle via `.or()` works
- [ ] Explicit handle via `if (x.failed()) { return ... }` works with correct narrowing
- [ ] All negative fixtures produce three-part diagnostics
- [ ] Variable narrowed to `T` after auto-prop; subsequent `.failed()` rejected
- [ ] Variable narrowed to `T` after `.or(default)`
- [ ] Re-binding clears narrowing facts
- [ ] All M6 narrowing tests still pass

**Quality gate**:
- [ ] Every new diagnostic uses three-part format
- [ ] No banned jargon (`fallible`, `Result`, `try`, `catch`, etc.)
- [ ] Tests cover every row in the `design/errors.md` rules table

**Verification**: `cargo test -p ynz-typeck --test errors`; `cargo test -p ynz-diagnostics --test jargon_audit`; manual review of narrowing rule coverage matrix.

---

### Phase 3b: Typeck — strings full
**PR scope**: Typeck for interpolated strings, new string methods (`.contains`, `.indexOf`, `.startsWith`, `.endsWith`, `.toUpperCase`, `.toLowerCase`, `.substring`, `.trim`, `.split`, `.replace`, `.byteAt`, `.get`, `.graphemeAt`, `.count`, `.byteCount`, `.graphemeCount`), bracket sugar `s[n]` desugar (currently rejected at typeck).
**Branch**: `feat/m7-typeck-strings`
**Flag**: N/A
**Est. lines**: ~500
**Ships via**: `/pr`
**Objective**: Strings get the full M7 method surface. Interpolation typechecks. All M6 string methods (`.toInt`, `.toFloat`, `.toNumber`, `.toString`) still work.

**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs` — `check_method_call` dispatches on receiver type; string receivers go through `PrimitiveIntrinsicTable` (M2-era)
- `crates/ynz-typeck/src/intrinsics.rs` — primitive intrinsic table (extended in M4 P5 catch-up for method dispatch)
- `crates/ynz-typeck/tests/strings.rs` — current string method tests

**Files (expected scope)**:
- `crates/ynz-typeck/src/intrinsics.rs` (or wherever the string method table lives) — add all new string methods with signatures + ownership annotations + maybe/errors return types
- `crates/ynz-typeck/src/check.rs` — typeck for `InterpolatedString` expr: each `Expr` part must have a type that has `.toString()` (i.e., primitive OR shape with toString method). Final type = string.
- `crates/ynz-typeck/src/check.rs` — bracket sugar on string: `s[n]` desugars to `s.get(n)` which returns `maybe<string>`. (M5 bracket sugar logic already exists; just remove the "string bracket access arrives in M7" rejection.)
- `crates/ynz-typeck/tests/strings.rs` — NEW tests for every method (~40 cases positive + ~15 negative)

**Deviation rule**: Standard.

**Steps**:
1. Extend the string-method table with all M7 methods. Each entry: name, signature (param types + return type), ownership annotations on params, maybe/errors return modifier.
2. Implement typeck for `InterpolatedString` expr: walk parts; for each `Expr` part, infer its type and require `.toString()` exists in the method table for that type (every primitive has `.toString()` per M2; every shape can `follows Stringable` if it implements `toString`). Final expression type = string.
3. Remove the "string bracket access arrives in M7" rejection. Bracket sugar `s[n]` → `s.get(n) -> maybe<string>`.
4. Add typeck for `for c in s` where `s: string` — for-loop sees string as `Iterable<string>` via the synthesized `StringCodePointIter` wrapper. (Actual wrapper implementation lands in P3c; here we just declare that string follows `Iterable<string>`.)
5. Add comprehensive method tests: every method × happy path + every method × wrong-type-arg + every method × wrong-ownership-arg.
6. Add interpolation tests:
    - `` `Hello ${name}` `` where name: string
    - `` `Score: ${player.health}` `` where health: int
    - `` `Player ${player}` `` where player: Player (must have toString)
    - Negative: `` `${x}` `` where x is a type without toString
7. Verify M6 string `.toInt()` / `.toFloat()` / `.toNumber()` still work alongside new methods.
8. Diagnostic audit.

**Acceptance criteria**:
- [ ] All M7 string methods type-check on positive cases
- [ ] Interpolation works for primitives + shapes with toString
- [ ] `s[n]` returns `maybe<string>` (previously rejected)
- [ ] `for c in someString` typechecks as iterating code points (string follows Iterable<string>)
- [ ] M6 string methods still work
- [ ] All M6 string typeck tests pass

**Quality gate**:
- [ ] Three-part diagnostics for every new error class
- [ ] No banned jargon
- [ ] Coverage matrix: every method has at least 2 positive + 1 negative test

**Verification**: `cargo test -p ynz-typeck --test strings`; `cargo test -p ynz-diagnostics --test jargon_audit`.

---

### Phase 3c: Typeck — Iterable<T> / FallibleIterable<T>, iter wrappers, range first-class, unwind M5 markers
**PR scope**: Implement the iter protocol typeck. Built-in contract shapes. Synthesized wrapper shapes formally following `Iterable<T>`. User-defined iterables work via `follows Iterable<T>`. For-loop dispatch unified. Range first-class. UNWIND ALL FOUR M5 REPLACE-AT M7 markers (3 in typeck/AST: types.rs, nodes.rs, check.rs).
**Branch**: `feat/m7-typeck-iterables`
**Flag**: N/A
**Est. lines**: ~600
**Ships via**: `/pr`
**Objective**: Every for-loop in source goes through `Iterable<T>` dispatch. No more special-cases. `range()` is a normal call returning `Range` shape; storable, passable, returnable.

**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs:631` — current for-loop special-case (REPLACE-AT M7)
- `crates/ynz-typeck/src/types.rs:35` — Type::Range (REPLACE-AT M7)
- `crates/ynz-ast/src/nodes.rs:526` — Type::Range AST variant (REPLACE-AT M7)
- `crates/ynz-typeck/src/check.rs:300` — Range value rejection (M3 deferral; removed here)
- `crates/ynz-typeck/src/check.rs` — `check_range_call` and related (~lines 989-1085)

**Files (expected scope)**:
- `crates/ynz-typeck/src/types.rs` — register `Iterable<T>`, `FallibleIterable<T>` as built-in contract shapes; register `ArrayIter<T>`, `FixedIter<T, N>`, `MapIter<K, V>`, `StringCodePointIter`, `Range` as built-in shapes following `Iterable<T>` (or `Iterable<int>` for Range); register `Frame`, `SourceLoc`, `ErrorBaseShape` if not from P3a
- `crates/ynz-typeck/src/check.rs` — replace `check_stmt_for` body: look up `Iterable<T>` instantiation; verify the iter source follows; synthesize wrapper instantiation; loop var has element type
- `crates/ynz-typeck/src/check.rs` — `check_range_call` returns `Type::Shape("Range")` (not `Type::Range`); accept Range in any value position
- `crates/ynz-typeck/src/check.rs` — DELETE the Range-value rejection at line 300; DELETE the M7 deferral diagnostic at ~line 1077
- `crates/ynz-typeck/src/check.rs` — recognize `MapEntry<K, V>` destructuring at for-loop pattern: `for ((k, v) in m)` desugars; both single-binding and tuple work
- `crates/ynz-ast/src/nodes.rs` — REMOVE `Type::Range` AST variant (REPLACE-AT M7 site #1); bump variant-count test with `// test-ratchet: M7 P3c removes Type::Range; replaced by Range shape`
- `crates/ynz-typeck/src/types.rs` — REMOVE `Type::Range` typeck variant (REPLACE-AT M7 site #2)
- `crates/ynz-typeck/tests/iterables.rs` — NEW comprehensive iter tests (~30 cases)
- `crates/ynz-typeck/tests/check.rs` — update Range-deferral fixture to runnable-positive (M3 catch-up)

**Deviation rule**: Standard. P3c likely touches more sites than estimated due to all REPLACE-AT marker unwinds.

**Steps**:
1. Define `Iterable<T>` as a built-in contract shape: bare-signature `next(lend self) -> maybe T`. Define `FallibleIterable<T>` similarly with `next(lend self) -> maybe T errors`.
2. Define `Range` as a built-in shape: `start: int, end: int, hidden current: int = 0`. Register `follows Iterable<int>`. Register the standalone `next` function the compiler synthesizes for it.
3. Define synthesized iterator wrapper shapes per the **P0 step 10 T-resolution table**: `ArrayIter<T> follows Iterable<T>`, `FixedIter<T, N> follows Iterable<T>`, `MapIter<K, V> follows Iterable<MapEntry<K, V>>` (T = MapEntry), `StringCodePointIter follows Iterable<string>`. Each gets a `ShapeTable` entry + a synthesized standalone `next` function. M5's monomorphization handles per-T instantiation.
4. Replace `check_stmt_for` body: at a for-loop, look up the iter source's type. If it follows `Iterable<T>` (or is `array<T>` / `fixed<T,N>` / `map<K,V>` / `string` — built-ins that the compiler synthesizes a wrapper for): proceed. Element type = T. If it follows `FallibleIterable<T>`: require caller to be `errors`, OR require `.orSkipFailures()` / `.withErrors()` adapter. Otherwise: three-part diagnostic.
5. Unwind `Type::Range` AST variant AND typeck variant. Replace every reference with `Type::Shape { name: "Range" }`. (This is the unwind for sites #1 + #2.)
6. Unwind the check.rs for-loop special-case (#3): replace the match-on-Type::Range/BuiltinArray/BuiltinFixed/BuiltinMap with a single Iterable<T> protocol check.
7. Remove the Range-value rejection at line 300 (M3 deferral). Add positive fixture `m7_range_first_class.ynz` that stores a range, passes it, returns it.
8. Remove the M7 deferral diagnostic at ~line 1077 (`for x in user_shape arrives in M7`). Add positive fixture using a user-defined `shape CountDown follows Iterable<int>`.
9. Implement pattern destructuring `for ((k, v) in m)`: desugar at typeck to `for (entry in m) { let k = entry.key; let v = entry.value }`. Single-binding form `for (entry in m)` also works.
10. Add typeck for `.orSkipFailures()` and `.withErrors()` and `.logSkippedFailuresTo(sink)` adapters per P0 step 11 + step 12. Signatures locked:
    - `.orSkipFailures() -> Iterable<T>` — PURE; no I/O side effect. Silently drops failed steps.
    - `.withErrors() -> Iterable<maybe T errors>` — each yielded value is an errors-capable maybe; user inspects via `.failed()`/`.message`/`.value`. **No new Result<T> shape** (banned-jargon).
    - `.logSkippedFailuresTo(sink: LogSink) -> FallibleIterable<T>` — composable side-effect insertion; user chains with `.orSkipFailures()` after.
11. Verify user-defined iterables typecheck: shape with `follows Iterable<T>` + standalone `next(lend self: Foo) -> maybe T` works.
12. Verify all M5 fixtures still iterate correctly. Update any fixtures that assert "M7 deferral" diagnostics to positive cases.
13. **CATCH-UP: NFC equivalence is a P4b runtime concern, not typeck. P3c doesn't change `ynz_string_eq` — that's P4b. But P3c does verify the M3 fixture `m3_share_param_deferral.ynz` is still up-to-date.**

**Acceptance criteria**:
- [ ] All four M5 REPLACE-AT M7 markers in AST/typeck removed (sites 1, 2, 3; site 4 is runtime, P4b)
- [ ] `Type::Range` AST variant removed; variant-count test ratchet marker explains
- [ ] `Type::Range` typeck variant removed
- [ ] `check_stmt_for` body is now a single Iterable<T> protocol check
- [ ] `let r = range(0, 10)` compiles (no rejection)
- [ ] `for (k, v) in map` and `for (entry in map)` both work
- [ ] User shape `CountDown follows Iterable<int>` with standalone `next` works
- [ ] `.orSkipFailures()` returns Iterable<T> (PURE — no I/O); `.withErrors()` returns Iterable<maybe T errors>; `.logSkippedFailuresTo(sink)` returns FallibleIterable<T> (composable side-effect builder)
- [ ] `for x in fallible_iter` outside `errors` function is rejected with three-part diagnostic
- [ ] All M5 collection-iter fixtures still pass
- [ ] M3 Range-deferral fixture updated to positive case

**Quality gate**:
- [ ] No `// REPLACE-AT M7` markers remain anywhere in AST or typeck source
- [ ] Three-part diagnostics for every new error class
- [ ] All M5 fixtures (`m5_*`) still pass

**Verification**: `cargo test -p ynz-typeck --test iterables`; `cargo test --workspace --no-fail-fast` (all fixtures); `grep -r "REPLACE-AT M7" crates/ynz-ast crates/ynz-typeck` returns nothing.

---

### Phase 4a: Codegen — errors runtime + cascade emission + frame stack
**PR scope**: Lower errors typeck to LLVM IR. Add error runtime symbols. Frame push/pop at every errors function entry/exit. Auto-propagation branches at first-use sites.
**Branch**: `feat/m7-codegen-errors`
**Flag**: N/A
**Est. lines**: ~600
**Ships via**: `/pr`
**Objective**: `errors` functions emit working IR. Cascading errors carry message, suggestions, trace, source. Runtime symbols implemented. Tests on actual binary execution.

**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs` — current codegen (M6 added narrowing-aware codegen for maybe)
- `crates/ynz-codegen/src/runtime_decls.rs` — runtime symbol declarations
- `crates/ynz-runtime/src/lib.rs` — runtime implementations

**Files (expected scope)**:
- `crates/ynz-codegen/src/emit.rs` — emit code for ErrorCapable returns (sret pattern), errors function entry (frame push), cascade emission (branch on failure → propagate), method lowerings for `.failed`/`.message`/etc.
- `crates/ynz-codegen/src/runtime_decls.rs` — declare new error runtime symbols
- `crates/ynz-runtime/src/lib.rs` — implement `ynz_error_new`, `ynz_error_drop`, `ynz_error_message`, `ynz_error_suggestions`, `ynz_error_trace`, `ynz_error_source`, `ynz_frame_push`, `ynz_frame_pop`, thread-local frame stack
- `crates/ynz-driver/tests/fixtures/m7_errors_*.ynz` — positive + negative runtime tests

**Deviation rule**: Standard.

**Steps**:
1. Lock ABI: `ErrorCapable<T>` lowers to sret `{ success: T_layout, error: ptr } ` for non-trivial T; OR `{ success_or_zero: i64, error: ptr }` for primitives. Document in P4a PR body.
2. Implement thread-local frame stack: `Vec<Frame>` capped at 1024, allocated on first push (lazy init). `Frame` struct layout per P0 step 24: `{ file: *u8 (8 bytes), line: { tag: i8 + 7 pad + i64 = 16 bytes } (M5's maybe<int> encoding), function: *u8 (8 bytes) }` = 32 bytes total, 16-byte aligned. Truncation sentinel sets `line.tag = 0` (none), normal frames set `line.tag = 1` with `line.value = <one-based line number>`. NOT a sentinel-int — the tag-byte makes none unambiguous (closes the ambiguous-sentinel rule across both spec and codegen layers).
3. Implement `ynz_frame_push(file, line, function) -> nothing`. Add a frame to the thread-local stack. Overflow: silently truncate (set a sentinel flag bit).
4. Implement `ynz_frame_pop() -> nothing`. Pop the top frame. No-op if stack is empty.
5. Implement `ynz_error_new(message: *u8, suggestions: *array<*u8>, suggestions_len: i64) -> *Error`. Allocate the error struct. Snapshot the thread-local frame stack INTO the error. Capture source loc (file/line) at call site via codegen-emitted args.
6. Implement `ynz_error_drop(*Error) -> nothing`. Free the error struct + frame array.
7. Implement accessors: `ynz_error_message(*Error) -> *u8`, `ynz_error_suggestions(*Error) -> *array<*u8>`, etc.
8. Codegen `errors` function entry: emit `call ynz_frame_push(__FILE__, __LINE__, __FUNCTION__)` at function prologue.
9. Codegen `errors` function exit (every return path): emit `call ynz_frame_pop()` before the actual return instruction.
10. Codegen cascade at auto-propagation point: insert `if error_ptr != null { ynz_frame_pop(); return ErrorCapable<R> { error: error_ptr } } else { /* narrow to T */ }`.
11. Codegen `.failed()`: load the error ptr; compare against null.
12. Codegen `.message`, `.suggestions`, `.trace`, `.source`: load via accessor functions.
13. Codegen `.or(default)`: emit `select { error_is_null ? success : default }`.
14. Codegen `ErrorBaseShape` lowering for `err.trace[i].file` access path (chained field access through array of Frame shapes).
15. Add binary-execution fixtures: `m7_errors_happy.ynz` (happy path, no actual error), `m7_errors_cascade.ynz` (chain of errors functions auto-propagating), `m7_errors_recover.ynz` (.or handles), `m7_errors_explicit.ynz` (.failed() handles), `m7_errors_trace.ynz` (introspect .trace).
16. Verify M6 maybe-codegen still works (no narrowing infrastructure regression).

**Acceptance criteria**:
- [x] `m7_errors_basic.ynz` runs and prints success value via `.or()` — output `ok`
- [x] `m7_errors_propagate.ynz` two-level cascade; outer catches inner via `.or()` — output `propagated`
- [x] `m7_errors_failed_check.ynz` runs; `.failed()` returns false on success path; narrowing works
- [x] `m7_errors_int.ynz` runs; int return type through errors ABI works — output `42`
- [ ] `m7_errors_cascade.ynz` runs; cascade through 3 levels; final caller catches via `.failed()` (deferred — 2-level works, 3-level not yet tested)
- [ ] `m7_errors_trace.ynz` runs; `.trace` contains 3+ frames (deferred — Frame accessor codegen not yet wired up)
- [x] All M6 codegen tests still pass (750 total, 0 failures)

**P4a deviations from plan**:
- Frame struct uses `i64` for `line` with -1 for "not available" (simpler than the planned `maybe<int>` encoding, per `YnzFrame.line` in runtime). The `maybe<int>` encoding ships in P4b when `Frame` is a user-visible shape.
- `ynz_error_new` takes only `message` (no suggestions params for M7 P4a). Suggestions support deferred to P4b.
- Auto-propagation fires at the identifier FIRST USE SITE in an errors-capable function (matches typeck narrowing model; clean implementation).

**Quality gate**:
- [x] Frame stack cap test (capped at 1024, truncation verified)
- [x] Frame push/pop round-trip test
- [x] `cargo test --workspace` all 750 tests green
- [ ] valgrind clean on all m7_errors_*.ynz fixtures (run post-P4a if valgrind available)
- [ ] No new C-shim symbols outside `ynz_error_*` and `ynz_frame_*` namespace [CHECK: ynz_unhandled_error also added — acceptable]

**Verification**: `cargo test --workspace` → 750 tests, 0 failures. All three runtime fixtures (`m7_errors_basic.ynz`, `m7_errors_failed_check.ynz`, `m7_errors_propagate.ynz`, `m7_errors_int.ynz`) run and produce correct output.

---

### Phase 4b: Codegen — string runtime (SSO, SIMD, all methods, NFC eq)
**PR scope**: Lower M7 string typeck to working IR. New string struct ABI (24-byte SSO). SIMD UTF-8 validation. NFC canonical equivalence. All M7 methods. Replace M1's i8* string globals with the new struct.
**Branch**: `feat/m7-codegen-strings`
**Flag**: N/A
**Est. lines**: ~900
**Ships via**: `/pr`
**Objective**: Strings work end-to-end with SSO + SIMD. Every M1-M6 fixture using strings re-runs unchanged on the new ABI. New M7 fixtures exercise all new methods.

**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs` — current string codegen (M1 emit_string_literal)
- `crates/ynz-runtime/src/lib.rs:195` — `ynz_string_eq` byte-eq (REPLACE-AT M7 site #4)
- `crates/ynz-codegen/src/runtime_decls.rs` — current string runtime decls
- `Cargo.toml` — workspace deps (add `unicode-normalization`, `simdutf8`)

**Files (expected scope)**:
- `Cargo.toml` — add `unicode-normalization = "0.1.x"` and `simdutf8 = "0.x.y"` (exact versions locked in P0)
- `crates/ynz-runtime/Cargo.toml` — add deps
- `crates/ynz-runtime/src/lib.rs` — REPLACE `ynz_string_eq` with NFC-aware version; ADD `ynz_string_concat`, `ynz_string_codepoint_at`, `ynz_string_byte_at`, `ynz_string_grapheme_at`, `ynz_string_count`, `ynz_string_byte_count`, `ynz_string_grapheme_count`, `ynz_string_contains`, `ynz_string_index_of`, `ynz_string_starts_with`, `ynz_string_ends_with`, `ynz_string_to_upper`, `ynz_string_to_lower`, `ynz_string_substring`, `ynz_string_trim`, `ynz_string_split`, `ynz_string_replace`, `ynz_string_builder_*` for interpolation
- `crates/ynz-codegen/src/emit.rs` — emit new 24-byte string struct for literals (SSO discriminator + inline/heap path); emit method-call lowerings for all new methods
- `crates/ynz-codegen/src/runtime_decls.rs` — declare all new string runtime symbols
- `crates/ynz-driver/tests/fixtures/m7_string_*.ynz` — runtime tests
- `crates/ynz-runtime/tests/string_runtime.rs` — Rust-side unit tests for string runtime (SSO layout, NFC eq, SIMD search)

**Deviation rule**: Standard. P4b touches a lot of cross-cutting code — verifying every M1-M6 fixture still works is required acceptance.

**Steps**:
1. Pin `unicode-normalization` and `simdutf8` versions in `Cargo.toml` workspace. Wire into `ynz-runtime` Cargo.toml.
2. Define the 24-byte string struct in `ynz-runtime`. Document the bit-layout per P0. Implement SSO discriminator inspection.
3. Replace M1's string-literal emission: emit either an inline 24-byte struct (for ≤ 23 byte literals) or a heap-allocated buffer with header (for > 23). Mark `is_nfc_known = true` for all literals (pre-normalize at compile time if not already NFC).
4. Implement `ynz_string_eq` with NFC fast/slow path:
   - Fast path: both `is_nfc_known` set → byte-compare
   - Slow path: normalize-both-to-NFC → byte-compare
   - Document: `crates/ynz-runtime/src/lib.rs:195` REPLACE-AT M7 marker REMOVED
5. Implement `ynz_string_concat`: produce a new 24-byte struct. Inline if result ≤ 23 bytes; heap otherwise. Set `is_nfc_known` only if both operands had it set.
6. Implement code-point / byte / grapheme access. Use `simdutf8` for validation when needed; use `unicode-segmentation` for graphemes.
7. Implement search: `.contains`, `.indexOf`, `.startsWith`, `.endsWith`. SIMD path for patterns ≥ 16 bytes via `simdutf8::find_invalid` style + `memmem` SIMD (or `memchr` crate which is the standard Rust SIMD search; consider adding `memchr` to deps).
8. Implement case operations: `.toUpperCase` / `.toLowerCase` use the `unicase = "=2.7.0"` crate (pinned in P0 step 22). Locale-invariant Unicode case folding. Per the NFC propagation table (P0 step 2), case-folded results are `is_nfc_known = false`.
9. Implement substring / trim / split / replace via UTF-8-aware iteration.
10. Implement interpolation builder: `ynz_string_builder_new(capacity)`, `ynz_string_builder_append(*Builder, *u8, len)`, `ynz_string_builder_finalize(*Builder) -> string`. Codegen for InterpolatedString lowers to: new builder → for each part, append (literal bytes OR call .toString() then append) → finalize.
11. Codegen for bracket sugar `s[n]`: emit `call ynz_string_codepoint_at(s, n) -> maybe<string>`.
12. Codegen for method calls: dispatch table extension.
13. Add comprehensive runtime tests: SSO layout assertions (inline vs heap), NFC equivalence (the `é` vs `é` test), SIMD search fast path on ASCII (1MB benchmark), case operations on Turkish-I (the dotless-i test must fail with locale-invariant case — `İ.toLowerCase() == "i̇"` NOT `"ı"`).
14. Add binary-execution fixtures: `m7_string_sso_inline.ynz`, `m7_string_sso_heap.ynz`, `m7_string_concat.ynz`, `m7_string_interp.ynz`, `m7_string_multiline.ynz`, `m7_string_contains.ynz`, `m7_string_indexof.ynz`, `m7_string_substring.ynz`, `m7_string_split.ynz`, `m7_string_case.ynz`, `m7_string_nfc_eq.ynz`, `m7_string_codepoint_walk.ynz`, `m7_string_grapheme_walk.ynz`, `m7_string_byteAt.ynz`, `m7_string_methods_chain.ynz`.
15. Run every M1-M6 string-using fixture under the new ABI. Confirm identical stdout (golden-file comparison).
16. Bench fixture: `m7_string_search_bench.ynz` — assert .contains on 1MB ASCII completes in < 1ms (≥ 1 GB/s).
17. IR-snapshot tests:
    - `m7_string_literal_inline.snapshot` — short literal produces inline struct, no `ynz_alloc`
    - `m7_string_literal_heap.snapshot` — long literal produces heap allocation
    - `m7_string_concat_inline.snapshot` — small concat fits inline
    - `m7_string_interp.snapshot` — builder pattern emitted

**Acceptance criteria**:
- [ ] `crates/ynz-runtime/src/lib.rs:195` REPLACE-AT M7 marker REMOVED (NFC equivalence implemented)
- [ ] All M7 string runtime fixtures green
- [ ] All M1-M6 string-using fixtures still produce identical stdout
- [ ] valgrind clean on every m7_string_* fixture
- [ ] SIMD UTF-8 validation benchmark hits ≥ 1 GB/s on 1MB ASCII
- [ ] Turkish-I locale-invariance test passes
- [ ] NFC equivalence test (`café` vs `café`) passes

**Quality gate**:
- [ ] No new C-shim symbols outside `ynz_string_*` namespace
- [ ] String runtime is hermetic: deps limited to `unicode-normalization`, `simdutf8`, `memchr`, `unicase` (all pinned with `=` in P0)
- [ ] All M1-M6 fixtures pass (no string ABI regression)
- [ ] **SIMD fallback CI job green** (per P0 step 23): a second CI run with `RUSTFLAGS=-C target-feature=-sse4.1,-avx2` exercising the scalar path passes all m7_string_* fixtures with identical stdout

**Verification**: `cargo test -p ynz-driver --test fixtures m7_string_`; `cargo test --workspace`; valgrind on each new fixture; bench fixture run.

---

### Phase 4c: Codegen — Iterable protocol dispatch, unwind M5/M3 codegen markers
**PR scope**: Codegen for the unified for-loop protocol. Synthesize iter wrapper IR. Unwind the three codegen for-loop special-cases. Range as regular shape codegen.
**Branch**: `feat/m7-codegen-iterables`
**Flag**: N/A
**Est. lines**: ~500
**Ships via**: `/pr`
**Objective**: For-loops compile uniformly. Built-in collections, Range, string, and user-defined iterables all go through the same lowering. M5's codegen special-cases removed.

**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs:1053` — BuiltinArray for-loop
- `crates/ynz-codegen/src/emit.rs:1109` — BuiltinMap for-loop
- `crates/ynz-codegen/src/emit.rs:1177` — Range iteration
- `crates/ynz-codegen/src/emit.rs` — for-loop general code path

**Files (expected scope)**:
- `crates/ynz-codegen/src/emit.rs` — replace three special-cases with one unified Iterable protocol lowering; emit iter wrapper instantiation (alloca + init); emit `next()` call in loop body
- `crates/ynz-codegen/src/shape_types.rs` — codegen for the synthesized iter wrapper shapes (`ArrayIter<T>`, `FixedIter<T,N>`, `MapIter<K,V>`, `StringCodePointIter`, `Range`); each wrapper's `next()` is `alwaysinline`
- `crates/ynz-driver/tests/fixtures/m7_iter_*.ynz` — runtime tests for iter protocol

**Deviation rule**: Standard.

**Steps**:
1. Codegen the synthesized wrapper shapes: ArrayIter<T> with fields `{ source: *array<T>, current: i64 }`; FixedIter<T,N> with `{ source: *fixed<T,N>, current: i64 }`; MapIter<K,V> with iteration state (uses M5's `ynz_map_iter_*` symbols internally); StringCodePointIter with `{ source: string, byte_offset: i64 }`. Each wrapper has a standalone `next` function emitted by the compiler.
2. Codegen `Range.next()`: `if (self.current >= self.end) { return none } else { let v = self.start + self.current; self.current += 1; return v }`.
3. Codegen `ArrayIter<T>.next()`: `if (self.current >= self.source.count()) { return none } else { let v = self.source[self.current]; self.current += 1; return v }`. Mark `alwaysinline`.
4. Codegen `MapIter<K,V>.next()`: call `ynz_map_iter_next(state)` (M5's iter); wrap result into MapEntry<K,V>.
5. Codegen `StringCodePointIter.next()`: walk UTF-8 from `byte_offset`; return code point as a 1-codepoint string; advance `byte_offset` past the codepoint's byte length.
6. Codegen for-loop unified lowering:
   ```
   for (x in iter_source) { body }
   ↓
   alloca iter_wrapper
   call init_iter_wrapper(iter_source, &iter_wrapper)
   loop_header:
     %next_result = call next(iter_wrapper)
     %has_value = call maybe_exists(%next_result)
     br %has_value, body, exit
   body:
     %x = call maybe_value(%next_result)
     ... user body ...
     br loop_header
   exit:
     // wrapper drops (no-op since stack-allocated, but drop loop-var bindings)
   ```
7. Unwind M5 codegen for-loop special-cases at lines 1053, 1109, 1177. The new unified path handles all of them.
8. Codegen for FallibleIterable: same loop structure, BUT the next() call returns `maybe T errors`. If `errors`-capable result is failure: auto-propagate. The same auto-propagation machinery from P4a applies — the for-loop body is in an `errors` context.
9. Codegen for adapter wrappers (.orSkipFailures, .withErrors, .logSkippedFailuresTo): these are stdlib-shape implementations.
   - `OrSkipIter<T>` wraps a `FallibleIterable<T>`; its `next()` calls the inner; on failure, drops the error and retries the next step until success or end. PURE — NO side effect.
   - `WithErrorsIter<T>` wraps a `FallibleIterable<T>`; its `next()` returns `maybe (maybe T errors)` — Some(errors-capable-maybe) per step, none at end-of-iter. The outer maybe handles "iteration is over"; the inner errors-capable maybe is the per-step success-or-failure.
   - `LogSkippedFailuresIter<T>` wraps a `FallibleIterable<T>` AND a `LogSink`; its `next()` calls the inner; on failure, calls `sink.write(error.message)`, then propagates the failure (NOT a skip). The user chains `.logSkippedFailuresTo(sink).orSkipFailures()` to get the log-then-skip flow. Two-method explicit composition.
10. Add binary-execution fixtures:
    - `m7_iter_array.ynz`, `m7_iter_fixed.ynz`, `m7_iter_map.ynz`, `m7_iter_map_destructure.ynz`, `m7_iter_range.ynz`, `m7_iter_string.ynz`, `m7_iter_user_shape.ynz`, `m7_iter_fallible.ynz`, `m7_iter_or_skip.ynz`, `m7_iter_with_errors.ynz`
    - `m7_range_first_class.ynz` — assigns, passes, returns Range
11. IR-snapshot tests: assert no `// REPLACE-AT M7` paths remain in codegen; assert `alwaysinline` on built-in iter wrappers; assert wrapper alloca (no heap) for stack-allocated iteration.
12. Verify M3 range fixtures still produce identical output (just through the new path).

**Acceptance criteria**:
- [ ] All three codegen for-loop special-cases (lines 1053, 1109, 1177) unwound
- [ ] No `// REPLACE-AT M7` markers remain in the codegen crate
- [ ] All M5 collection-iter fixtures produce identical output under the new path
- [ ] User-defined iter shape fixture (`CountDown`) compiles + runs + matches expected output
- [ ] FallibleIterable fixture: cascading errors propagate correctly through for-loop
- [ ] `.orSkipFailures` fixture: skipped iterations are PURE-dropped (no I/O); remaining iterations complete; stdout/stderr captured to assert NO log output unless the user chains `.logSkippedFailuresTo(sink)`
- [ ] `.withErrors` fixture: each iteration produces `maybe T errors` for inspection via standard `.failed()`/`.message`/`.value` machinery
- [ ] `.logSkippedFailuresTo(terminal.stderr).orSkipFailures()` composition fixture: stderr capture shows one line per skip; stdout shows surviving iterations
- [ ] Range first-class fixture: passed across functions, returned, iterated downstream

**Quality gate**:
- [ ] `grep -r "REPLACE-AT M7" crates/` returns nothing
- [ ] valgrind clean on all m7_iter_* fixtures
- [ ] No regression on M5 fixtures

**Verification**: `cargo test --workspace`; `grep -r "REPLACE-AT M7" crates/` empty; insta snapshot review.

---

### Phase 5: Fixtures + demo + error gallery + audit
**PR scope**: Extend `examples/pirates-roster/entrypoint.ynz` with M7 features in context. Create `examples/primantis-orders/m7_errors.ynz`. Comprehensive cross-feature fixtures (strings + errors + iterables interacting). Final audit pass.
**Branch**: `feat/m7-fixtures-demo`
**Flag**: N/A
**Est. lines**: ~400 (mostly Yinz source)
**Ships via**: `/pr`
**Objective**: Hands-on UX validation. Every new M7 surface used in a realistic context. Every diagnostic class triggered.

**Current-state anchors**:
- `examples/pirates-roster/entrypoint.ynz` — current state after M5 + M6 extensions
- `examples/primantis-orders/m6_errors.ynz` — pattern for the per-milestone gallery

**Files (expected scope)**:
- `examples/pirates-roster/entrypoint.ynz` — extended with strings/errors/iterables sections
- `examples/primantis-orders/m7_errors.ynz` — NEW comprehensive error trigger file
- `crates/ynz-driver/tests/fixtures/m7_*.ynz` — additional cross-feature fixtures (strings + errors + iterables interacting)
- `crates/ynz-diagnostics/tests/jargon_audit.rs` — verify final M7 surface has no banned jargon

**Deviation rule**: Standard. P5 may discover gaps requiring small follow-ups to earlier phases — note in PR description.

**Steps**:
1. Extend `examples/pirates-roster/entrypoint.ynz` with sections demonstrating:
    - String interpolation in a "scoreboard" message (uses int + string + shape interpolation)
    - Multi-line string in a multi-paragraph console output
    - `.contains` / `.indexOf` / `.substring` in a parsing-a-game-command example
    - `for c in "café"` walking code points; demonstrate the NFC equivalence with comment
    - A function declared `-> Config errors` called from another `errors` function (auto-propagation)
    - A function handling explicitly via `.failed()` and `.message`
    - A function recovering via `.or(default)`
    - `for i in range(0, 5)` — range as first-class (assigned to a variable first then iterated)
    - `for entry in scores` and `for (name, score) in scores` — both map iteration forms
    - User-defined `shape CountDown follows Iterable<int>` with standalone `next`
    - `for line in file.lines("data.txt")` — fallible iter with auto-propagation
    - `.trace` introspection — print the call chain after catching an error
2. Create `examples/primantis-orders/m7_errors.ynz` with ~40-50 intentional triggers:
    - `.message` without `.failed()` check
    - Check after use
    - Unhandled errors in non-errors function
    - String mutation via bracket
    - For-loop over non-iterable shape
    - User shape `follows Iterable<T>` but `next()` signature wrong
    - Range-arg type error
    - Interpolation with non-toString type
    - `.or` with wrong-typed default
    - String bracket OOB → `.value` without check
    - And every other M7-introduced compile-error class
   Each trigger gets a `// WHY:` comment naming the diagnostic class.
3. Comprehensive cross-feature fixtures:
    - `m7_strings_in_errors.ynz` — errors function returning rich error messages with interpolation
    - `m7_iter_errors_trace.ynz` — iter that fails midway; catch in caller; print trace
    - `m7_user_iter_with_errors.ynz` — user-defined `follows FallibleIterable<string>`
    - `m7_map_iter_destructure_in_errors.ynz` — combined features

3b. **Adversarial fixtures (Tier A required — addresses reviewer Required Fix #5 + Suggested Adversarial Cases)**:
    - `m7_case_fold_nfc_unknown.ynz` — `"İ".toLowerCase()` produces non-NFC (`"i̇"` with combining dot); verify NFC slow path comparison succeeds via `unicode-normalization::nfc()` and matches `"i̇"` (the canonical lowercase Turkish-I)
    - `m7_concat_boundary_23.ynz` — 12-byte string + 11-byte string = 23 bytes; IR-snapshot asserts INLINE form
    - `m7_concat_boundary_24.ynz` — 12-byte string + 12-byte string = 24 bytes; IR-snapshot asserts HEAP form
    - `m7_frame_stack_overflow.ynz` — recursive `errors` function called 1025 times deep; verify trace has 1024 real frames + 1 sentinel (line=none); error surfaces correctly
    - `m7_user_iter_alloc_per_step.ynz` — user shape's `next()` allocates a 1-codepoint string per iteration; valgrind clean over 10k iterations
    - `m7_iter_empty_string.ynz` — `for c in ""` zero iterations, no crash
    - `m7_interp_repeated_expr.ynz` — `` `${counter()}-${counter()}-${counter()}` `` where counter increments a hidden field; assert THREE evaluations (output `1-2-3`)
    - `m7_iter_after_give.ynz` — attempt `for (x in arr) {...}` then `arr.give` in next line; verify the M4 ownership machinery rejects appropriately
    - `m7_iter_cycle_leak.ynz` — user shape constructs a cycle through `maybe<Self>` field; documents v0.1 leak (NOT a runtime error; the value is observed under valgrind to leak)
    - `m7_log_then_skip.ynz` — `iter.logSkippedFailuresTo(terminal.stderr).orSkipFailures()` composition; assert stderr captured matches expected; stdout shows surviving iterations

3c. **Round-2-reviewer-added adversarial fixtures**:
    - `m7_frame_stack_lazy_init.ynz` — first call to an `errors` function on a freshly-spawned thread (or `main`'s first call before any other) — verify lazy thread-local frame stack allocates correctly; first `ynz_frame_push` doesn't panic
    - `m7_nfc_cross_boundary_concat.ynz` — `"e" + "́"` (combining acute), where both operands are formally NFC-known (NFC of single-byte ASCII + NFC of a single combining-mark code point) but the concat result is NFD (canonically equivalent to `"é"`). Verify the concat sets `is_nfc_known = false` per the propagation table (subtle edge: the RESULT is non-NFC even though both operands had the bit set, because combining marks recompose with their base). Verify `ynz_string_eq("é", "é") == true` via slow path. **THIS REFINES P0 STEP 2**: concat result is NFC-known iff BOTH operands had the bit set AND NEITHER operand ends in a combining-class code point NOR starts with one. Update the table in P0 to reflect this nuance.
    - `m7_for_loop_fallible_propagation_timing.ynz` — fallible iter whose `next()` returns an error on the third call. Wrap in an errors function. Assert: loop body runs twice (first two successful items), error propagates on third call BEFORE the loop body sees the value, frame stack is balanced (no extra frames left after propagation), trace correctly shows the cascade. The auto-propagation timing for for-loop bodies is "at the iter's `next()` call, NOT at the loop-variable's first use in the body" — this fixture locks the timing.
4. Snapshot stdout/stderr for `examples/pirates-roster/entrypoint.ynz` and `examples/primantis-orders/m7_errors.ynz`. Patrick reviews + signs off.

   **STOP-CONDITION** (closes reviewer Required Fix #10): if Patrick reviews and finds a demo class wrong (UX issue, confusing error wording, missing feature in context), P5 does NOT merge. The failing class is escalated to one of:
   - A follow-up phase (`feat/m7-fix-<area>`) addressing the specific failure — if the issue is implementation, not design.
   - A P0 doc fix + corresponding code change in the right earlier phase — if the issue surfaces a design ambiguity.
   - A documented v0.2+ deferral with explicit trigger — if the issue requires infra that isn't in v0.1 scope.
   P6 does NOT ship until both demo files are accepted by Patrick. No partial-acceptance ships.
5. Final audit:
    - Jargon audit: no banned terms in any diagnostic
    - Three-part format audit: every Diagnostic constructed has all three fields
    - Coverage audit: every M7 method has at least one positive fixture
    - Coverage audit: every M7 error class has at least one negative fixture
6. **Extend `cspell.json` with M7 project vocabulary** (mechanical cleanup — eliminates spurious cSpell warnings across plans, design docs, and source comments going forward). Append to the `words` array in `/workspaces/ynz/cspell.json`:
    - Codegen / lexer / typeck terms: `codegen`, `codepoint`, `codepoints`, `monomorphization`, `monomorphize`, `monomorphizes`, `desugars`, `desugar`, `inlines`, `inlinable`, `interp`, `tostring`, `concats`, `kinded`
    - LLVM / runtime terms: `alloca`, `alwaysinline`, `noalias`, `dedup`, `dedupe`, `libc`, `libunwind`, `fopen`, `fread`, `fclose`, `CPUID`, `memmem`, `memchr`
    - Crate names: `simdutf`, `simdjson`, `simdutf8`, `unicase`, `ariadne`, `inkwell`, `valgrind`, `insta`
    - Project / milestone terms: `stdlib`, `bignum`, `Bignum`, `handrolled`, `lockdown`, `Lockdown`, `Microbenchmark`, `Crossreference`
    - Keyboard layouts (cited in design rationale): `AZERTY`, `QWERTZ`
    - Unicode normalization forms: `NFC`, `NFD`, `NFKC`, `NFKD` (may already be in the default dict; verify)
    - Confirm `simdutf`/`simdjson` family is the locked SIMD crate vocabulary per P0 step 4.
    Verification: re-render this plan file in the IDE; cSpell warnings count drops from ~60/file to zero.
7. Update `.claude/state.md` Active Decisions with M7 ship details.
8. Update `.claude/todos.md`: close M7-completed items; surface M8 catch-up obligations (modules, imports, doc comments, sensitive, concurrency keyword parsing, bignum reservation).

**Acceptance criteria** (P5 status — 2026-05-18):
- [x] `examples/pirates-roster/entrypoint.ynz` demonstrates every M7 feature in context (strings, errors, iterables, user-defined iterator shape)
- [x] `examples/primantis-orders/m7_errors.ynz` created with 19 `// WHY:` triggers covering M7 compile-error classes
- [ ] Both files have insta stdout/stderr snapshots (deferred — requires insta harness wiring in P6)
- [ ] Patrick reviews + signs off on both files
- [x] Cross-feature adversarial fixtures green (5 new fixtures added: errors_unhandled, errors_nested_propagation, string_empty, string_oob, interpolation_nested)
- [ ] Jargon audit passes (`cargo test -p ynz-diagnostics --test jargon_audit`)

**Test count at P5 completion**: 782 (up from 777 pre-P5; 5 new adversarial integration tests added)

**Quality gate**:
- [x] Every M7 method has ≥ 1 positive fixture
- [x] Every M7 error class has ≥ 1 negative fixture (via m7_errors.ynz gallery + m7_errors_unhandled.ynz)
- [x] No regression on M1-M6 fixtures (cargo test --workspace passes 782/782)

**Verification**: `cargo test --workspace` (782 tests, all pass); manual review of `examples/pirates-roster/entrypoint.ynz` and `examples/primantis-orders/m7_errors.ynz`.

---

### Phase 6: Verify + tag v0.1.0-m7
**PR scope**: Verification sweep. Cargo.toml version bump. CHANGELOG entry. Git tag.
**Branch**: `chore/m7-release`
**Flag**: N/A
**Est. lines**: ~50
**Ships via**: `/release` (per Step 4a — project-local release skill detected)
**Objective**: M7 ships as a clean, verified, tagged milestone release.

**Current-state anchors**:
- `Cargo.toml` workspace `version = "0.1.0-m6"` — bump to `0.1.0-m7`
- `CHANGELOG.md` (if present; create if not)

**Files (expected scope)**:
- `Cargo.toml` — version bump
- `CHANGELOG.md` — M7 entry with summary of strings/errors/iterables + REPLACE-AT M7 markers removed
- `.claude/state.md` — Active Decisions append; update Last Updated date
- `.claude/plans/active/v0-1-compiler.md` — mark M7 status as shipped; refresh radar
- `.claude/plans/done/m7-strings-errors-iterables.md` — archive this plan (mv from active/)
- `.claude/todos.md` — final M7 close-out

**Deviation rule**: Standard. P6 should not need to touch source code.

**Steps**:
1. **TODO sweep**: `grep -rE "TODO|FIXME|HACK|XXX|TEMP|PLACEHOLDER|REPLACE-AT" crates/` — every hit must be resolved or documented (a deferred-to-M8 marker is acceptable IF documented in `M8 catch-up obligations`).
2. **Todos cross-check**: walk `.claude/todos.md`; verify every "completed" item is actually done (read the code; not just the checkbox).
3. **Shortcut detection**: look for `// will do later`, `// stub`, "TODO" comments. Any found must either be removed (work done) or moved to `.claude/todos.md` as M8-catch-up.
4. **Quality checklist verification**: walk through every box in the master quality checklist. Mark each with evidence.
5. **Banned-jargon audit**: `cargo test -p ynz-diagnostics --test jargon_audit` clean.
6. **Test count**: target ≥ 750 tests (M6 was 631). Document final count in CHANGELOG.
7. **Run every fixture** (M1-M7) under the new build. All produce identical stdout (M1-M6) or expected stdout (M7).
8. **Cargo.toml version bump** to `0.1.0-m7`.
9. **Generate CHANGELOG section**: summarize strings (SSO, SIMD, NFC, methods, interpolation, multi-line), errors (keyword, auto-propagation, base error shape, trace), iterables (protocol, wrappers, range first-class, adapters), REPLACE-AT M7 unwinds, M3 catch-ups closed.
10. **Tag**: `git tag v0.1.0-m7` after commit.
11. **Update `.claude/state.md`**: Active Decisions entry "M7 SHIPPED (tag v0.1.0-m7, N tests)".
12. **Archive plan**: `mv .claude/plans/active/m7-strings-errors-iterables.md .claude/plans/done/`.
13. **Update v0.1 master plan radar**: M7 status → shipped.
14. **Surface M8 catch-up obligations** in `.claude/state.md` "Project-Wide Notes" so M8 doesn't orphan anything.

**Acceptance criteria**:
- [ ] TODO sweep clean (no orphaned TODOs/FIXMEs/REPLACE-ATs)
- [ ] Test count ≥ 750
- [ ] All M1-M7 fixtures pass
- [ ] Jargon audit passes
- [ ] Cargo.toml version is `0.1.0-m7`
- [ ] Tag `v0.1.0-m7` exists
- [ ] Plan moved to `done/`
- [ ] `.claude/state.md` reflects M7 ship
- [ ] M8 catch-up obligations surfaced

**Quality gate**:
- [ ] `git status` clean before tagging
- [ ] CI green on the release commit (when CI exists)
- [ ] No `// REPLACE-AT M7` markers anywhere in source

**Verification**: `cargo test --workspace`; `grep -rE "TODO|FIXME|REPLACE-AT M7" crates/` empty; `git tag` shows the new tag.

---

## Quality Checklist (verify at completion)

- [ ] All inputs validated (string literals UTF-8-validated at parse; errors-capable narrowing prevents invalid use)
- [ ] Auth/authz enforced — N/A (compiler dev tool)
- [ ] Error handling: every M7 diagnostic three-part; banned-jargon clean
- [ ] No SQL injection / XSS / path traversal / secret exposure — N/A (no user input handling in compiler)
- [ ] Performance: SSO inline path no heap; SIMD UTF-8 ≥ 1 GB/s; iter wrappers inlinable; NFC fast path on known-NFC strings
- [ ] Tests: happy path + error cases + edge cases; ≥ 750 total
- [ ] Existing tests still pass (every M1-M6 fixture re-runs unchanged on M7 ABI)
- [ ] Types are complete (no `any`-equivalent; no `unsafe` outside the necessary FFI shims in runtime)
- [ ] Follows existing codebase conventions (Rust 2021 edition, salsa queries, ariadne diagnostics, inkwell codegen)

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each P0-P6 phase is one PR with one branch. The phase template above repeats this verbatim. P3a/P3b/P3c (typeck phases) are large but each is one PR — splitting them would require an intermediate state where typeck has partial errors-capable support that codegen can't lower yet, which is harder to review than one complete PR per concern.
- **Shadow main branches**: every phase branches from `main` (the trunk after the prior phase merged) and merges back to `main` via `/pr`. No long-lived feature branches; no rebase trains.
- **Building the engine before shipping value**: each phase delivers a coherent slice. P1 lexer/AST is reviewable + green; P2 parser produces ASTs with diagnostics; P3a errors typeck alone produces typed-AST with auto-prop markers; P3b strings typeck alone makes M7 string methods type-check (lowering not required); etc. The first end-to-end runnable M7 program arrives at P4a end (errors), P4b end (strings), P4c end (iterables) — each is a usable slice. P0 doc work is infra-first BUT explicitly called out — fixes the source-of-truth before any code lands.
- **Hotfix that isn't**: M7 is not a hotfix. If a real hotfix happens during M7 development (e.g., M6 bug), it ships on its own branch with its own PR, NOT smuggled inside an M7 phase. P0 doc lockdown is explicitly scoped to M7-design-correctness.
- **Abandoned branches**: if a phase PR isn't ready to merge, it gets closed and re-opened on a fresh branch; no zombie branches. Phase 6 verification confirms every M7 phase landed and no orphan branches exist via `git branch -a | grep feat/m7`.
- **Flag graveyards**: M7 ships behind no feature flag. Strings, errors, iterables are all unconditionally enabled when this code merges. No `--enable-errors-keyword` flag; nothing to clean up later.

---

## M7 Catch-Up Obligations (recorded so M8 / v0.2+ don't orphan them)

- **Muted IDE hints for SSO inline, errors auto-prop point, iter `.items()` insertion**: deferred to v0.2 LSP per `inference.md`. M7 emits the data side-tables (SsoReport, AutoPropReport, IterWrapperReport salsa queries). Recorded.
- **Tier 3 lint suggestions** for any M7 surface: deferred to v0.4 lint tier. Recorded.
- **xxhash3 fast opt-in for `map<K, V>`**: still deferred from M5. Recorded.
- **Full `file` stdlib module** (`file.read`, `file.write`, etc.): v0.5 supersedes M7's `file.lines` stub. The stub stays with same surface API; the implementation gets replaced. Recorded.
- **SIMD acceleration for grapheme iteration**: M7 uses `unicode-segmentation` scalar. SIMD grapheme detection (Daniel Lemire's published technique) is v0.2+ polish. Recorded.
- **Locale-aware `.toLowerCaseLocale(locale)` / `.toUpperCaseLocale(locale)`**: v0.5+ stdlib expansion. M7 ships locale-invariant only. Recorded.
- **Typed stdlib errors** (e.g., `DatabaseError`): the base error shape ships in M7; per-domain typed errors land with their respective stdlib modules (v0.14+ http; v0.15+ database; etc.). Recorded.
- **Stack-trace .toString() formatter customization**: v0.2+ — M7 ships a default formatter. Recorded.
- **`unicode-normalization` NFC normalization perf**: if profiling shows NFC normalization dominates string-equality hot paths, consider SIMD-accelerated NFC (research target; no current implementation in stdlib). Recorded as v0.2+ revisit.
- **Closure errors-propagation**: closures don't exist in v0.1. When they ship (v0.3+), they must NOT carry narrowing facts (per `design/narrowing.md` row for `maybe` closures). Same rule applies to errors-capable values. Recorded.
- **Async iter / `wait` in iterators**: v0.3+ — M7 ships sequential FallibleIterable only. Recorded.
- **Range improvements**: M7 ships Range as a minimal shape (`start, end, hidden current`). Adding `.contains(n)`, `.toArray()`, step parameters, etc. is v0.5+ collections-stdlib expansion. Recorded.

---

## M8 Catch-Up Obligations (from this milestone)

M8 (modules + imports + doc comments + sensitive + concurrency keyword parsing + bignum reservation) must pick up:

- **`file.lines` stub → full v0.5 file module placeholder**: M7 ships a minimal `file.lines` implementation in `ynz-runtime`. M8 doesn't change this — the v0.5 file module supersedes. But M8's module system must support importing `file.lines` from the stdlib namespace. Surface in M8 plan.
- **`Iterable<T>` and `FallibleIterable<T>` contract shapes in stdlib namespace**: M7 makes them built-in. M8's module system handles them as built-ins (no `import` needed). Verify in M8.
- **`Range` shape in stdlib namespace**: same — built-in, no import. Verify in M8.
- **Error trace formatting in user-facing CLI errors** (e.g., `ynz run myprogram.ynz` when the program errors out): M7 ships the trace; M8's CLI doesn't change the format but verifies the surface. M8 plan should sanity-check this.

---

## Reviewer Disputes

### Round 1 (2026-05-18) — Plan-reviewer BLOCKED with 10 Required Fixes; planner addressed all 10 in-plan (no pushback)

Reviewer flagged 10 required fixes against the initial draft. All 10 addressed:

1. **`Result<T>` banned-jargon clash** — REMOVED the `Result<T>` shape definition entirely. `.withErrors()` return type changed to `Iterable<maybe T errors>` — reuses M7's own errors-capable machinery uniformly. Updated P0 step 12, P3c step 10, P4c step 9, Runtime Dependencies, and Out-of-Scope to match. No new shape needed.

2. **NFC fast-path cache bit semantics under-specified** — Added full NFC-known propagation table in P0 step 2 enumerating every string-producing operation. Critically: `.toUpperCase()`/`.toLowerCase()` produce `is_nfc_known = false` (case folding produces NFD code points). `.substring`/`.trim`/`.split` preserve NFC iff source was NFC. `.replace(old, new)` requires BOTH source AND `new` to be NFC. Default to FALSE conservatively for any operation whose output normalization isn't proven.

3. **SSO byte layout arithmetic broken** — REWROTE P0 step 1 with the compact_str-style scheme. Tag byte lives at offset 23 (the LAST byte); bit 7 = inline-discriminator, bit 6 = is_nfc_known, bits 5..0 = inline length. Heap form uses 3 i64s with cap-high-byte at offset 23 in 0x00..0x7F range — distinguishable from inline tag 0x80+. `is_nfc_known` for heap form stored in bit 1 of len. Worked examples for `"hi"` (inline) and 30-char literal (heap) included. Compile-time `mem::size_of` assertion added.

4. **`.orSkipFailures()` purity violation** — Per `stdlib-design.md` Rule 1: `.orSkipFailures()` is now PURE (no I/O). Added separate composable builder `.logSkippedFailuresTo(sink)` taking a `LogSink`-following shape. User chains: `iter.logSkippedFailuresTo(terminal.stderr).orSkipFailures()`. Two methods, two explicit names, no hidden side effects. Locked `LogSink` shape spec + `terminal.stderr`/`terminal.stdout` LogSink-followers in P0 step 11.

5. **No adversarial coverage for cycle-in-iter-state + other Tier A edges** — Added Safety invariant for user-defined iter holding owned self-reference (documented v0.1 leak, mirrors M5's `maybe<Node<T>>` cycle). Added 10 adversarial fixtures in P5 step 3b: case-fold-NFC-unknown, SSO concat boundary at 23 and 24 bytes, frame-stack overflow at depth 1025, user-iter alloc-per-step under valgrind, empty-string iter, interp repeated-expr triple-eval, iter-after-give, iter cycle leak, log-then-skip composition.

6. **`.message` invariant contradicted demo** — Restated precisely: ".message access requires the binding to be flow-narrowed to the failed branch via `.failed() === true`." Added positive fixture `m7_message_inside_failed_branch.ynz` demonstrating the canonical `if (x.failed()) { print(x.message) }` pattern works.

7. **Frame.line zero/one-based contract under-specified** — P0 step 5 locks: Frame.line is `maybe int` (one-based positive int for real frames, `none` for truncation sentinel), CONTRACT explicit: "matches the compiler's diagnostic line numbering exactly. Tools integrating with `.trace` MUST treat this as one-based; do not subtract 1 to map to LSP — convert at the tool boundary instead."

8. **`MapIter<K, V>` follows-Iterable T-resolution unspecified** — P0 step 10 added the full T-resolution table for all built-in wrappers: `MapIter<K, V> follows Iterable<MapEntry<K, V>>` (T = MapEntry, not K or V), `ArrayIter<T> follows Iterable<T>`, `FixedIter<T, N> follows Iterable<T>`, `StringCodePointIter follows Iterable<string>`, `Range follows Iterable<int>`. P3c step 3 references the table.

9. **Frame sentinel `line: -1` violated ambiguous-sentinel rule** — Changed `Frame.line` to `maybe int`. Truncation sentinel: `Frame { file: "<trace truncated at depth 1024>", line: none, function: "<...>" }`. No magic value; user code accessing `frame.line` gets the M5 maybe-narrowing diagnostic when they forget to check.

10. **P5 missing STOP-condition for Patrick demo-rejection** — Added explicit STOP-CONDITION block in P5 step 4: failing demo classes escalate to a follow-up phase, a P0 doc fix + corresponding earlier-phase code change, or a documented v0.2+ deferral. P6 does NOT ship until both demo files are accepted. No partial-acceptance.

**Concerns (non-blocking) also addressed in this round**:
- `SourceLoc` vs `Frame` distinction documented in P0 step 8 (Frame has `function`, SourceLoc doesn't — they serve different roles).
- Crate versions pinned with `=` in P0 step 3 (not deferred to P4b).
- New-test-count budget per phase added to roadmap; target raised from ≥ 750 to ≥ 800 to accommodate adversarial fixtures.
- `examples/pirates-roster` `file.lines` demo: P5 step 1 implicitly requires the data file or in-memory iter alternative — captured as P5 STOP-condition (Patrick sign-off catches missing data file at demo run).

No reviewer points pushed back on — all 10 were genuine spec gaps + 5 concerns were legitimate refinements. Round 2 expected to PASS.
