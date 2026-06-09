# Changelog

## [0.3.0-m5] — 2026-06-09 — M3e + M3b: Cross-Module Frame Serialization + Auto-Parallelization

Commit range: v0.3.0-m4..v0.3.0-m5

### What changed

v0.3.0-m5 bundles two concurrency milestones. **M3e** lets a suspending function be
called across module boundaries — the M2/M3a substrate could only suspend within one
module. **M3b** makes independent suspending statements overlap automatically: write
straight-line code, get concurrent I/O for free, with no `async`/`await` and no spawn
syntax. Programs that compiled before are byte-identical; the new `--no-auto-parallel`
build flag is a permanent escape hatch and the cross-implementation consistency oracle.

#### Features (M3e — cross-module frame serialization)

- **Cross-module `wait`** — a function in one module can now `wait` on a suspending
  function defined in another module. The callee's full task-frame layout is serialized
  through the export table so the state-machine codegen can rebuild it on the caller's
  side. The universal-reject floor M3b shipped as an honest partial is lifted.
- **Conservative cross-boundary safety** — an unresolvable edge (dynamic dispatch, FFI)
  still produces a clean teaching error, never a silent non-suspension.

#### Features (M3b — auto-parallelization)

- **Independent suspending statements overlap** — two statements that each suspend and
  don't depend on each other run concurrently via a single-threaded interleaved poll
  (default ≈ the slower of the two, vs `--no-auto-parallel` ≈ the sum). Zero new runtime
  symbols — no thread spawn.
- **`wait` is the ordering barrier** — `wait foo()` completes (and joins all in-flight
  overlapped work) before the next statement starts. Loop iterations stay sequential.
- **`background` give/copy figured out for you** — a value handed to a `background` task
  is automatically given (move) when unused after, or deep-copied when still used. Heap
  values get a real deep copy, freed when the task completes — no use-after-free, no leak.
- **Two new IDE hints** — `wait_points` (where the compiler suspends on I/O) and
  `background_routing` (which pool a `background` task lands in) render as muted inlay
  hints that read the *effective* suspend set, so they can't drift from actual behavior.

#### Soundness (M3b)

Auto-parallelization is sound by a **type-based conservative floor**: any argument whose
type is mutable-heap (a `shape`, `array`, `map`, `maybe`, union, or `dynamic`) is treated
as a potential aliased write, so a statement pair touching one runs sequentially. Golden
Rule 5 (compile-time safety) outranks Golden Rule 10 (efficiency) — the compiler forfeits
read-only-heap overlap rather than risk a data race. Primitive and `string` arguments
still overlap.

#### Known limitations / deferrals

- **`background`-handle result collection** (`ec-wrapper-collect-on-completion`) — moved to
  v0.3-M4. Collecting a `background`-spawned `-> T errors` task's result via a handle needs
  the handle-collection syntax (`.send`/`.receive`), which ships separately. The inline-poll
  path for `-> T errors` works today.
- **Read-only-heap overlap forfeited** — the conservative floor sequentializes mutable-heap
  arguments even when the callee only reads them. Reversal path: a type+alias-aware ownership
  analysis. Documented in `design/concurrency.md`.

#### Pre-existing bugs surfaced (not introduced by this release)

Two silent-wrong-output bugs were found during M3b adversarial review and confirmed to
pre-date auto-parallelization (identical wrong output with and without `--no-auto-parallel`).
Tracked on the roadmap as milestone M3f (blocks the `v0.3.0` final release) and in
`.claude/todos.md`: (1) same-callee wide-`errors` (`-> number errors`) result slot reuse;
(2) a crossing local inside control flow after a `wait`. Neither is a regression.

### Tests

1929 passing across the workspace (0 failed, 6 ignored). Cross-implementation oracle:
`--no-auto-parallel` equals default output on 196 + 31 fixtures, zero real divergence.
M3b passed a 5-reviewer cumulative review; M3e passed its own at close-out.

## [0.3.0-m4] — 2026-06-04 — M3a: `wait` in Loops + Frame-Backed Crossing Locals

Commit range: v0.3.0-m3..v0.3.0-m4

### What changed

v0.3.0-m4 lifts the two harshest M2 compile-error guards (`LocalCrossesWait` and `WaitInsideLoop`) by completing the state-machine codegen substrate: frame-backed mutable locals and loop suspension. Every program that previously errored with one of these guards now compiles and produces correct output. Programs that compiled before are byte-identical.

This is v0.3-M3a — the first half of the original M3 plan, split out as its own shippable slice. M3b (statement-level auto-parallelization) builds on this substrate.

#### Features

- **Frame-backed crossing locals** — a `let` local declared before a `wait` and used after now compiles correctly. The compiler flushes the local's value into the task frame before suspension and reloads it on resume. Every Yinz type is handled: `int`, `bool` (i1 zero-extended), `float` (bitcast), `number` (decimal128, 2 i64 slots), `string` (pointer via `ptr_to_int`), `Shape` (direct struct), `array<T>` (heap pointer), `map<K,V>` (heap pointer), and `options` (int-encoded). ONE `ynz_alloc` per task tree even for local-heavy functions — no per-crossing-local allocation.
- **`wait` inside `while` loops** — the loop counter and any loop-carried locals are frame-backed; each `wait` suspension preserves them. Resume re-enters the loop body for the current iteration. Iterations run **sequentially** (design/concurrency.md "Loop Iterations — Sequential by Default") — NOT parallelized. M3b handles auto-parallelization of independent statements.
- **`wait` inside `for` loops** — `for (x in array<T>)` and `for (entry in map<K,V>)` loops with `wait` in the body. The array index and heap pointer are frame-backed; resume re-enters for the current index. Iteration order is preserved; output matches the hand-unrolled sequential form.
- **`wait` inside `match` arms** — a `wait` in any `match` arm resumes into the correct arm. Any bindings live across the suspension are frame-backed via the P1 mechanism.
- **`sleepAsync`→`sleep` / `sleepMs`→`sleepBlocking` rename** (P0) — the two sleep intrinsics now have their final names. `sleep(ms)` is the yielding form (suspends the function, OS thread freed). `sleepBlocking(ms)` blocks the OS thread (use only where thread-blocking is correct, e.g., background analytics). `wait sleepBlocking(...)` still emits the "`wait` has no effect" warning — intentional.

#### New diagnostics (WHAT/WHAT-INSTEAD/WHY format)

Kept-guard diagnostics reworded (P0) — no more false "ships in v0.3-M3":
- **`SubExprSuspendViolation`** — reworded WHY states the step-by-step design rationale (Golden Rule 7) and notes M3b auto-parallelization of independent statements.
- **`MutualSuspensionCycle`** — reworded WHY states that self-recursion works and mutual cycles are always restructurable.

New P3 deferral diagnostics (clean compile errors, never silent wrong output):
- **`FixedArrayIterWithWait`** — `fixed<T>` array as a `for`-iterator with a `wait`; stack pointer dangles after suspension. Use `array<T>`.
- **`StoredRangeWithWait`** — a `range` local iterated by a `for` with `wait`; inline the range expression.
- **`ExpressionIterWithWait`** — a call-expression collection (`for (x in makeArray())`) with `wait`; bind to a local first.
- **`UnsupportedCrossingLocalType` (fixed)** — a `fixed<T>` local crossing a `wait`; use `array<T>`.
- **`MapEntryFieldAfterWait`** — `entry.key`/`entry.value` read after a `wait` in a `for (entry in map)` body; read fields before the `wait`.
- **`RangeCrossingLocalWithWait`** — a `range` local crossing a top-level `wait`; inline the range.
- **`ArrayShapeRuntimeFieldWithWait`** — `array<Shape>` crossing a `wait` with runtime-computed element fields; use all-literal field values. Conservative interim guard; lifts entirely when the `m3c-array-by-value` milestone ships.

#### Known limitations (seven P3 deferrals — compile errors, not silent wrong output)

All seven are loud compile errors with WHAT/WHAT-INSTEAD/WHY diagnostics pointing to `design/concurrency.md` sections and named future milestones. None produce silent wrong output or crashes:

1. `fixed<T>` as for-iterator with a wait — stack pointer dangles; use `array<T>`
2. Stored range local then iterated with a wait — inline the range
3. Call-expression collection with a wait — bind to a local first
4. `fixed<T>` crossing local with a wait — use `array<T>`
5. Map `entry.key`/`entry.value` after a wait — read before `wait` or use an outer accumulator
6. `range` crossing local with a wait — inline the range
7. `array<Shape>` with runtime-computed element fields crossing a wait (conservative guard) — lifted by `m3c-array-by-value`

#### IDE surface

M3a adds NO new muted-hint domain and NO new lint rule. The `wait_points` muted-hint domain and `background_routing` fire in M3b. The `prefer-yielding-sleep` Tier-3 lint rides M4's `[[lint_rule]]` infra (not yet built). `sleep`/`sleepBlocking` hover docs updated with their renamed names and per-stdlib-design-rule danger annotation for `sleepBlocking`.

#### Demo / gallery

- `examples/pirates-roster/entrypoint.ynz` extended with v0.3-M3a Phase 4 section (`m3a_p4_demo`): a roster scouting loop that `wait sleep`s per item, with a crossing-local `total` accumulator updated across 5 suspensions. Demonstrates `for`+`wait`+crossing-local in realistic context (85+92+78+95+88=438).
- `examples/primantis-orders/v0_3_m3a_errors.ynz` updated: removed the `WaitInsideLoop` trigger for `for` loops (P3 lifted it); added `SubExprSuspendViolation` and `MutualSuspensionCycle` triggers for the two permanent kept guards; added triggers for four representative P3 deferrals (`FixedArrayIterWithWait`, `StoredRangeWithWait`, `MapEntryFieldAfterWait`, `ArrayShapeRuntimeFieldWithWait`).
- Two cross-impl consistency tests added (`v03_m3a_p4_no_auto_parallel_*`): assert `--no-auto-parallel` produces byte-identical output to default on a crossing-local fixture and a for-loop fixture. Gate wired for M3b.

## [0.3.0-m3] — 2026-06-01 — `wait` Actually Suspends + State Machine Codegen

Commit range: v0.3.0-m1..v0.3.0-m3

### What changed

v0.3.0-m3 makes `wait expr` actually suspend the calling function and yield the OS thread back to the scheduler — instead of blocking the thread for the duration of the awaited operation. After this release, `wait sleepAsync(200)` pauses for 200ms WITHOUT holding an OS thread captive; the freed thread runs other ready tasks during the pause; the function resumes when the timer fires. This release ships the entire state-machine coroutine infrastructure that v0.3-M3 (cross-module may-block propagation + auto-parallelization) builds on.

The inference model: the compiler analyzes the full call graph of each compilation unit to determine which functions reach a suspension point (transitive may-block analysis). Those functions are automatically compiled to stackless state machines. No function-coloring: every function is still declared with `function`, and the compiler handles the rest.

#### Features

- **`wait` actually suspends** — a function containing `wait sleepAsync(ms)` yields its OS thread for `ms` milliseconds and resumes when the timer fires. Previously (v0.3-M1), `wait` was a pass-through; the thread blocked. Now the thread is freed for other tasks. The OS-visible semantic is unchanged: code after `wait expr` runs only after `expr` completes.
- **State-machine codegen** — functions that transitively reach `sleepAsync` are compiled to stackless coroutines (same model as Rust async, without function coloring). Frame: heap-allocated per spawned task tree via `ynz_alloc`; freed by RAII guard on completion or cancellation. Composed frames: a call tree that is one background task = one `ynz_alloc`.
- **Transitive may-block analysis** — the compiler builds an intra-unit call graph and propagates the `suspends` property from `sleepAsync` upward to a fixpoint. Functions that transitively reach `sleepAsync` become state machines automatically; pure-CPU functions compile to straight-line code with zero overhead. Cross-module propagation ships in v0.3-M3.
- **`sleepAsync(ms: int) -> nothing` intrinsic** — non-blocking sleep. `wait sleepAsync(200)` suspends for 200ms, thread freed. Pairs with `sleepMs(ms)` (blocking, ships M1) for cases where thread-blocking is correct.
- **Inferred `wait`** — calling a suspending function without writing `wait` is valid: the compiler infers the suspension point. The IDE shows a muted `wait` hint at the call site. Both forms — explicit and inferred — produce identical code.
- **Value-returning state machines** — `function fetchData() -> int errors { wait sleepAsync(50); return 42 }` suspends and returns a typed value. The `{i64, i64}` error-capable ABI threads through `Poll<T>` across suspension boundaries.
- **Errors cascade through suspension** — an errors-capable state machine that produces an error AFTER a wait suspension correctly propagates the error through the SM return slot. The `Frame` trace and `SourceLoc` data survive the resume boundary.
- **Nested state machines** — a state machine calling another state machine uses inline poll-and-yield (composed sub-frames), not a OS bridge. The call tree is one composed struct; no per-call-site allocation beyond the task-tree root.
- **`background` from inside a state machine** — spawning a background task from inside a suspending function is now correct (was a deadlock in M1 due to runtime mutex contention).
- **Recursive state machines** — a function calling itself with `wait` allocates one heap frame per recursive call; frames are freed on cancellation via the RAII guard + `live_mask` bitfield for non-trivial locals.

#### Improvements

- **Composed frames = one allocation per task tree** — the whole SM call tree (entrypoint → middle → inner → sleepAsync) is ONE composed struct and ONE `ynz_alloc`, not one allocation per call. Matches `design/future/concurrency.md`'s "low memory, fast spawn — like Rust's async."
- **Errors cascade correctly** — errors produced after a `wait` suspension are propagated through the SM return slot to callers. The error frame pointer (`{i64,i64}` ABI field0) survives the `Poll<T>` boundary.
- **No OS bridge** — the `ynz_rt_call_state_machine_sync` sync bridge shipped in earlier drafts was fully removed (Phase 8). Every suspending call uses inline poll-and-yield. Only `RUNTIME.block_on(entrypoint)` at program entry is the top-level driver. This matches `design/future/concurrency.md`'s no-bridge model.
- **Concurrency proof** — 8 background tasks each calling `wait sleepAsync(100)` complete in ~100ms total (not 800ms), proving they share OS threads via state-machine suspension.

#### New diagnostics (WHAT/WHAT-INSTEAD/WHY format)

- **`SubExpressionSuspendPosition`** — a suspending call nested inside a larger expression (arithmetic, string interpolation, condition) is rejected with a teaching error. Give it its own `let` line. Expression-position suspension ships in v0.3-M3.
- **`MutualRecursionSuspendingCycle`** — two or more distinct suspending functions that call each other form a mixed-layout cycle that corrupts heap on cancellation. Self-recursion (one function calling itself) is the M2-supported form. Per-frame layout metadata for mixed cycles ships in v0.3-M3.
- **`WaitInsideLoop`** — `wait` inside a `for`/`while`/`match` body is rejected. The loop counter requires frame-backed mutable locals (flush-before-suspend + reload-at-resume transform), which ships in v0.3-M3.
- **`LocalCrossesWait`** — a `let` local declared before an explicit `wait` and used after is rejected. Function parameters are exempt (frame-backed at every resume point). The full mutable-local transform ships in v0.3-M3.
- **`WaitOnNonCallExpression`** — `wait 42` (applying `wait` to a non-call expression) is rejected.
- **`WaitOnNonMayBlock`** — `wait print(...)` (applying `wait` to a CPU-bound call) emits a Tier 3 warning; program still runs.
- **`CantInferDynamicDispatch`** — calling a method via `dynamic Contract` vtable from a suspending function is rejected. The callee's suspension status can't be determined at compile time.
- **`CantInferCrossModule`** — calling a function defined in another compilation unit from a suspending function is rejected. Cross-module suspension propagation ships in v0.3-M3.

#### Known limitations (retained Option-B guards — deferred to v0.3-M3)

- **`WaitInsideLoop` and `LocalCrossesWait` are compile errors** — the frame-backed mutable locals transform (flush-before-suspend + reload-at-resume) is M3-scope. These guards are intentional teaching errors, not bugs.
- **Sub-expression suspending calls rejected** — `let x = 1 + suspendingFn()` and `if (suspendingFn() > 0)` are compile errors. Expression-position suspension ships in v0.3-M3.
- **Non-self mutual recursion rejected** — `foo()` calls `bar()` calls `foo()` where both suspend is a compile error. Mixed-layout cycle frames require per-frame size metadata that ships in v0.3-M3.
- **Recursive state machines use heap-boxed frames** — self-recursion (`foo()` calling itself) allocates one frame per recursive call on the heap. Stack recursion with suspension is not supported; each recursive depth needs its own live state.
- **Cross-module suspension analysis deferred** — calling a function from another file that transitively suspends produces a compile error in v0.3-M2. Cross-module propagation requires the M8 multi-file query mechanism and ships in v0.3-M3.
- **Dynamic dispatch through `dynamic Contract` vtable** — suspension status of dynamic-dispatch callees can't be determined at compile time; rejected from suspending callers. Support ships in a future version.

#### Internal-only

- **`__testFallibleAsync(bool) -> int errors`** — internal test intrinsic used by state-machine error-cascade tests. NOT in the feature registry, NOT in LSP completions, NOT for user code. Deletes when v0.5 ships real fallible async I/O intrinsics.
- **`ynz_rt_spawn`, `ynz_rt_async_sleep_create`, `ynz_rt_async_sleep_poll`** — new C-ABI runtime shims in `libynz_runtime.a` for state-machine coroutine driving. Internal; no user-facing API surface.

#### Demo / gallery

- `examples/pirates-roster/entrypoint.ynz` extended with v0.3-M2 section (`m3m2_demo`): 8 background pirates each `wait sleepAsync(100)` concurrently, inference demo (no explicit `wait`), value-returning SM, nested SM.
- `examples/primantis-orders/v0_3_m2_errors.ynz` — compile-error gallery covering all 7 M2 diagnostic classes: SubExprSuspend, MutualRecursion, WaitInsideLoop, LocalCrossesWait, WaitOnNonCallExpression, WaitOnNonMayBlock, CantInferDynamic.
## [0.3.0-m2] — 2026-05-31 — Teaching-Surface Bug Hunt

Commit range: v0.3.0-m1..v0.3.0-m2 (PR #68)

### What changed

v0.3.0-m2 fixes every bug found by the four-agent teaching-surface audit — 14 cataloged bugs plus 3 same-class siblings caught during execution. These are all teaching-surface regressions: a false warning on valid code, or an inlay hint (`let → const`, `array → fixed`) firing on a binding that is actually mutated. A teaching language whose teaching surface lies is worse than one with no hints; this milestone closes that gap. Shipped from the `v0.2.1-m10` (LSP-gap-closure roadmap) work, folded straight into the 0.3 line.

#### Fixes — unused-import false positives

- **Imports used only in a type position no longer warn "imported but never used."** Seven positions were invisible to the reference tracker: options-variant access (`Timeframe.fiveMinute` — the user-reported repro), `is`-narrowing, `follows`/`extends`, shape field-type, module-`const`, `dynamic`, generic position, and union-alias RHS (`shape X = A | B`). Genuinely-unused imports still warn (no over-suppression).

#### Fixes — inlay hints never fire on a mutated binding

- **`else =>` catch-all arms** are now visited by all six inlay-hint walkers (a `let` mutated only in an `else` arm no longer shows "effectively const").
- **Nested mutation paths** (`player.address.street = x`, `arr[i][j] = v`) now mark their root binding mutated.
- **Ownership-aware suppression**: a `let` passed to a call only suppresses its `let → const` hint when the callee's parameter is `lend`/`give` — so `print(count)` keeps the hint. Previously almost every `let` lost its hint.
- **Literal-argument mutations** (inside struct/array/map literals) are now tracked.
- **Concurrency-wrapper mutations**: a `lend`/`give` mutation hidden inside `wait foo(x)`, `background foo(x)`, an `is` expression, or string interpolation `${foo(x)}` is now tracked across all three inlay-hint expression walkers (was invisible — `wait heal(buf)` had been printing a misleading "effectively const" hint).

#### Fixes — hover, completion, ownership/copy hints

- **Hover**: a variable named like a contextual keyword (`let share = 5; share + 1`) now hovers to its type instead of the keyword doc; the keyword hover is preserved in genuine signature-modifier positions; the end-of-token cursor returns content.
- **Completion** no longer triggers on space (only `.`).
- **Ownership hints** now fire for UFCS method calls (`player.heal(20)`) and generic-function calls, matching the free-function form.
- **Copy hints** now recurse into nested call arguments (`outer(inner(n))`).

#### Features — teaching surface

- **Banned-jargon quick-fix**: a banned-jargon identifier now offers a one-click code action that replaces it with the Yinz term AND carries the WHY (sourced from the registry `[[banned_jargon]]` entry) — the fix teaches, not just swaps.
- **`array → fixed` click-to-make-explicit**: the inlay hint now carries a `TextEdit` (parity with `let → const`).

#### Behavior changes worth noting

- **`let → const` hints now appear far more often** — the over-suppression bug meant the flagship auto-promotion hint almost never fired in real code; it now fires whenever a binding is provably never mutated.
- **Inlay hints render in Pittsburgh gold (`#ffd23f`) and anchor at end-of-line** instead of on the `let` keyword (VSCode `[ynz]`-scoped contribution).

#### Cleanup

- `booleanean` typo → `boolean` in the `print` diagnostic; banned `infers`/`inferred` removed from user-facing diagnostic + registry-description + inlay-hover text (internal/design uses correctly retained). Four jargon-audit tests guard against reintroduction.

#### Tests

- ~60 new regression tests across 10 files (`ynz-typeck`, `ynz-lsp`, `ynz-diagnostics`, `ynz-driver`), each fail-before / pass-after. The `examples/pirates-roster/` demo gained a "Three Rivers schedule" section exercising 7 import patterns with a zero-spurious-warning snapshot guard.

#### Known follow-ups (deferred, tracked in `.claude/todos.md`)

- Inlay-hint walker completeness: ownership/copy hints don't yet cover every statement form / container expression / UFCS-copy (missed hints, not lies). Cross-file `follows`/`extends` remains a same-file-only compile constraint pending the M8 cross-file-resolution work.

---

## [0.3.0-m1] — 2026-05-21 — Runtime Bootstrap + Working `background`

Commit range: v0.2.0..v0.3.0-m1

### What changed

v0.3.0-m1 makes `background fn(args)` actually run on a separate OS thread. Previously (v0.1, v0.2), `background` and `wait` parsed and type-checked but ran sequentially — a correctness illusion. M1 ends that: `background` now schedules work onto a Tokio blocking thread pool; main continues immediately.

#### Features

- **Working `background`** — `background fn(value.give)` and `background fn(value.copy())` now spawn on a separate thread. Main continues without waiting. Fire-and-forget; no handles in M1 (handle-form ships in v0.3-M4 with channels).
- **Thread-pool runtime (`libynz_rt`)** — Tokio multi-thread runtime embedded in `libynz_runtime.a`; users never see Tokio types. C-ABI bridge: `ynz_rt_init` / `ynz_rt_spawn_blocking` / `ynz_rt_check_preempt` / `ynz_rt_shutdown`.
- **`sleepMs(ms: int)` intrinsic** — synchronous blocking sleep for demos and timing tests. Maps to `ynz_thread_sleep_ms`.
- **Large-copy warning** — Tier 3 lint: `background fn(largeStruct.copy())` where estimated copy size > 64 bytes emits a warning suggesting `.give` to transfer ownership instead.
- **`.give` inlay hint** — LSP `ownership_call_site` domain extended: when the large-copy warning fires, an inline `.give (transfers ownership; no copy)` muted annotation appears at the arg site.

#### Safety errors (new in M1)

- **Lend-cross-thread** — `background fn(...)` where `fn` has a `lend` parameter is now a compile error. A mutable borrow across a thread boundary can outlive the owner — same safety hole as `share` (which was already rejected).
- **Kernel-mode rejections** — `background` and `wait` in `--kernel` mode produce teaching errors (thread-pool runtime doesn't run in kernel mode). Flag is hidden in M1; exposed in v0.3+.

#### Improvements

- **Parser termination guarantee** — Two `_ =>` arms in `parse_block` and `parse_call` lacked forward-progress guarantees. Fixed with `pos_before` check + forced `advance()` + 10,000-iteration `debug_assert!` cap. Previously-skipped error-gallery fixtures in `ynz-fmt` tests now run.
- **Corpus determinism harness** — New test suite runs each of 69 corpus files (driver fixtures + examples) twice and asserts byte-identical output. Guards against non-determinism from background thread scheduling.
- **Keyword hover docs** — `wait` and `background` hover docs updated with WHAT/WHAT-INSTEAD/WHY per Rule 11. Registry `KeywordEntry` schema extended with optional hover fields; backward-compatible (existing keywords use legacy format).

#### Fixes

- Parser infinite-loop on `background` function with error-recovery path.
- `m8_combo_modules_sensitive_concurrency` relaxed from strict-sequential to presence-only (background is genuinely concurrent now).

#### Demo / gallery

- `examples/pirates-roster/entrypoint.ynz` extended with v0.3-M1 section (`m3m1_demo`): main prints before background analytics done.
- `examples/primantis-orders/v0_3_m1_errors.ynz` — new error gallery covering share-param, lend-cross-thread, and large-copy warning.

---

## [0.2.0] — 2026-05-21 — LSP Full Experience + Compiler Bug Fixes

Commit range: v0.2.0-m4..v0.2.0

### What changed

v0.2.0 closes out the v0.2 dev-loop series (M1 feature registry, M2 LSP thin slice, M3 formatter, M4 watch daemon) by completing the LSP into a full editor experience and fixing three correctness bugs that were deferred from earlier milestones.

#### 8 new LSP capabilities

- **`textDocument/definition`** — Cmd+click any identifier to jump to its declaration. Works across files using `exports.rs` + `resolve_import.rs` cross-file resolution.
- **`textDocument/references`** — Right-click → "Find All References" lists every use-site across the entire project. Emits `$/progress` notifications for large scans.
- **`textDocument/rename`** + **`textDocument/prepareRename`** — F2 to rename; all references update atomically via `WorkspaceEdit`. Validates new name against Yinz keywords and banned jargon. Rejects imports-at-origin errors with precise "rename at the declaration file" guidance.
- **`textDocument/formatting`** + **`textDocument/rangeFormatting`** — Format-on-save delegates to `ynz-fmt`. `ynz-fmt::format_range` added for range format. Emits `window/showMessage` when formatting is skipped due to parse errors (no silent empty-edits). Yinz files are LF-only; format-on-save normalizes CRLF → LF on Windows — **first save on a CRLF file will produce a large diff; this is intentional, not a bug**.
- **`textDocument/inlayHint`** — 5 firing domains (variable types, ownership call-site modifiers, copy-point markers, `array<T>` → `fixed<T>` promotion, `let` → `const` promotion) + 4 protocol-only domains awaiting v0.3+ data (function param types, wait points, lifetimes, allocators). Click-to-make-explicit supported for the 5 firing domains.
- **`textDocument/codeAction`** — Quick-fix lightbulb for every diagnostic with a WHAT-INSTEAD. One click applies the registered replacement (e.g., `class` → `shape`).
- **`textDocument/semanticTokens/full`** + **`textDocument/semanticTokens/range`** — Richer-than-TextMate highlighting: keywords, types, functions, variables, parameters, fields, options variants each get their own token type. Semantic tokens refine the TextMate grammar; they never disagree on keyword spans.

#### Hover and completion polish

- **Doc-comment hover**: `///` doc comments attached to functions, shapes, options, and constants now appear above the signature in hover popups.
- **Completion receiver narrowing**: after-dot completions filter to the receiver's type (e.g. `score.` where `score: int` shows only int methods).

#### `ynz build --json`

New `--json` flag on `ynz build`: emits NDJSON diagnostic events + a summary line. Schema is stable at `"v0.2.0"`. Replaces regex-parsing ariadne text for CI/tooling consumers. Default human-readable output is unchanged.

```json
{"type":"diagnostic","schema_version":"v0.2.0","severity":"error","kind":"UnknownDeclarationKeyword","code":"UnknownDeclarationKeyword","span":{"file":"/path/to/file.ynz","start_byte":0,"end_byte":5},"message":"...","data":{"what":"...","what_instead":"...","why":"..."}}
{"type":"summary","schema_version":"v0.2.0","errors":1,"warnings":0,"suggestions":0,"exit_code":1}
```

#### Structured LSP diagnostic fields

Every LSP diagnostic now populates `code` (DiagnosticKind name), and `data` (structured `{what, what_instead, why}` object) so rich-UI clients can render WHAT-INSTEAD/WHY without re-parsing `message`.

#### Three compiler correctness fixes

- **Hidden-field default eval** (`codegen`): `shape Foo { hidden bar: string = "default" }` constructed as `Foo {}` previously zero-initialized `bar` (null pointer for strings). Now evaluates the default expression. Works through `extends` inheritance chains. Audit confirmed 0 live consumers of the broken behavior — no existing program output changes.
- **Dynamic-dispatch call-site coercion** (`typeck`): passing a `ConcreteFoo` to a `dynamic Foo` parameter was a typeck error even when `ConcreteFoo follows Foo`. Fixed in two parts: `resolve_ast_type` now resolves `AstType::Dynamic` correctly (was returning `Type::Error`), and the call-arg checker accepts the coerce when `shape.follows.contains(contract)`. Note: method dispatch ON a `dynamic` receiver (`d.method()`) remains deferred to v0.3.
- **UFCS const-lend check** (`typeck`): `const p; p.heal(20)` where `heal` declares `lend self` was silently accepted — only the free-function form `heal(p, 20)` produced an error. Both forms now produce byte-identical diagnostics via a shared `check_arg_ownership` helper.

#### VSCode extension v0.2.0

Extension version bumped to `0.2.0`. Install via `.vsix` from the GitHub release.

---

## [0.2.0-m4] — 2026-05-20 — `ynz watch` Rebuild-on-Save Daemon

Commit range: v0.2.0-m3..v0.2.0-m4

### What changed

Before v0.2-M4: developers had to manually re-run `ynz run` after every source change. The compiler had no incremental-rebuild daemon and no way to watch the filesystem.

After v0.2-M4: `ynz watch` ships — a long-running terminal command that recompiles `.ynz` files on every save and re-executes the program. Sub-second warm rebuilds via a long-lived salsa `CompilerDb`. Designed for vim/neovim users, CI pipelines, and anyone who prefers a terminal-first dev loop.

**New crate: `ynz-watch`** — the watch daemon:
- `ynz watch <file.ynz>` — watch a single file; rebuild + re-run on every save
- `ynz watch <project/>` — project mode (requires `yinz.toml`); watches all `.ynz` files
- `--check` — build only, no execution (CI gate; exits 1 on first-build compile failure)
- `--json` — emit NDJSON event stream on stdout: `watch-ready`, `build-start`, `diagnostic`, `build-end`, `child-spawn`, `child-exit`, `watch-shutdown`; schema version field `"v0.2-m4-unstable"` (semver-stable at v0.2.0)
- `--no-clear` — preserve terminal scrollback between rebuild cycles (CI logs, debugging the watcher)

**Salsa LRU caps**: lex(128), parse(128), module_signatures(128), check(64), codegen(32) — compiler emits less memory over long sessions.

**Three-layer memory defense** for 24h+ continuous operation:
- Layer 1: salsa LRU caps per query (above)
- Layer 2: periodic `CompilerDb` drop+recreate every N=500 rebuilds or 4h (shadow `HashMap<PathBuf, String>` preserves source state across rebuild — zero source-state loss)
- Layer 3: RSS polling via `memory-stats` crate; soft warn at 1GB (rate-limited 1/60s), hard-stop at 4GB with WHAT/WHAT-INSTEAD/WHY exit message

**Child process safety**: child spawned in own process group (`setsid`/`CREATE_NEW_PROCESS_GROUP`); SIGTERM → 2s grace → SIGKILL on every rebuild cycle; Drop impl prevents zombie processes on panic.

**EPIPE handling**: `ynz watch --json | head -1` → `head` exits → watch detects `BrokenPipe` on next emit, emits `WatchShutdown { reason: "pipe-closed" }` to stderr, exits 0.

**Demo**: `examples/incline-watcher/` — minimal yinz.toml project; edit the print message and save to see the rebuild cycle live.

### Deferred to follow-up

- `YNZ_WATCH_LRU_*` runtime env-var LRU tuning: documented in `design/watch.md`, not yet wired to `set_lru_capacity` — tracked in `todos.md` as `watch-lru-runtime-tuning`
- Interactive watch commands (`r` to rebuild, `q` to quit) — tracked as `watch-interactive-commands`
- `yinz.toml` hot-reload during watch — deferred to v0.5 package-manager milestone
- Windows full validation pass — tracked as `watch-windows-validation`

## [0.2.0-m3] — 2026-05-20 — `ynz fmt` Formatter

Commit range: v0.2.0-m2..v0.2.0-m3

### What changed

Before v0.2-M3: Yinz had no canonical style enforcement. Every developer formatted `.ynz` files by hand; pre-commit hooks had no machine-readable gate; the LSP had no format-on-save target to call.

After v0.2-M3: `ynz fmt` ships as both a zero-config CLI subcommand and a stable library API (`ynz-fmt`) that v0.2-M5's LSP `textDocument/formatting` handler will consume.

**New crate: `ynz-fmt`** — zero-config Yinz source formatter:
- `format(source: &str) -> Result<String, FmtError>` — the stable library API consumed by v0.2-M5 LSP
- `check(source: &str) -> Result<CheckResult, FmtError>` — read-only canonicality check
- Prettier-style full AST reflow: same program → same output regardless of original whitespace
- Comment preservation: leading, inline, and trailing `//` comments attached to their AST nodes; floating comments emitted in-place
- Backtick strings: never reflowed (content preserved byte-exact)
- Idempotency guaranteed: `format(format(x)) == format(x)` for all parser-valid inputs
- Semantic safety: `parse(format(x)).ast == parse(x).ast` modulo trivia (verified by round-trip property test)

**CLI: `ynz fmt`**:
- `ynz fmt <file>` — rewrite in-place with atomic same-dir tempfile rename
- `ynz fmt --all [dir]` — walk `yinz.toml` project and format every `.ynz` file (continues on per-file parse errors)
- `ynz fmt --check [--all] <path>` — read-only CI gate: exits 1 if any file would change; prints `Would reformat: <path>`
- `ynz fmt --stdin` — format stdin, write canonical output to stdout (used by editor pipelines)

**Mass-rewrite**: all existing `.ynz` source files in the repo were canonicalized by the formatter in this milestone. From this PR forward, every `.ynz` commit is canonical.

**Semver stability**: `ynz-fmt` library API is frozen at v0.2-M3. Breaking changes require a major-version bump once v0.2.0 ships.

Note: `textDocument/formatting` (LSP format-on-save) ships in v0.2-M5; M3 ships the library that M5 will call.

---

## [0.2.0-m2] — 2026-05-20 — LSP Thin Slice + VSCode Extension

Commit range: v0.2.0-m1..v0.2.0-m2
PRs: #47 (P0), #54 (P1–P6), #55 (P7), #56 (P8), #57 (P9)

### What changed

Before v0.2-M2: Yinz had a compiler but no editor story. Every feature in the SSOT registry (built in M1) had teaching content — `why` fields, `substitute` recommendations, `ships_in` metadata — but none of it was visible in an editor.

After v0.2-M2: `.ynz` files in VSCode (or Cursor) get inline red squiggles with the full WHAT/WHAT-INSTEAD/WHY teaching content, autocomplete of every keyword and primitive method, and hover docs sourced directly from the SSOT registry. Adding a keyword or deferred feature to `registry/features.toml` in any future version makes it appear in the editor automatically — no manual LSP changes needed.

**New crate: `ynz-lsp`** — JSON-RPC-over-stdio Language Server backed by the existing salsa queries:
- `textDocument/didOpen` / `didChange` / `didClose` — updates the live salsa DB, triggers incremental re-check
- `textDocument/publishDiagnostics` — pushes WHAT/WHAT-INSTEAD/WHY diagnostics from `check_query` to the editor after every change
- `textDocument/completion` — registry-driven: keywords, primitive intrinsics filtered by receiver type, type-attached constants, deferred features (marked deprecated), user-defined symbols from `module_signatures_query`
- `textDocument/hover` — registry lookup first (all 9 entry kinds), falls back to typeck symbol table for user-defined functions and shapes
- Position encoding: UTF-8 preferred, UTF-16 fallback — byte-accurate to the compiler's internal `SourceSpan` representation

**New crate: `ynz-tmgrammar`** — binary that reads `ynz-registry` and emits `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json`. A snapshot test fails CI if the committed grammar drifts from registry content.

**New subdir: `tooling/vscode-ynz/`** — VSCode extension that spawns `ynz-lsp` and wires it to `.ynz` files:
- Syntax highlighting from the registry-derived TextMate grammar
- LSP client connecting to `ynz-lsp` over stdio
- Configurable `yinz.server.path` setting
- Devcontainer: auto-builds `ynz-lsp` on container create, auto-installs the extension from the marketplace once published

**Distribution:** `.vsix` at `https://github.com/yinzers/yinz-lang/releases/latest/download/yinz-latest.vsix` (always the most recent build). Marketplace publish deferred — Azure DevOps org provisioning blocked; tracked in todos.

### New registry adapter (`ynz-registry`)

Two new adapter functions added to `ynz-registry/src/lsp_adapter.rs` (projections of existing data — no new registry entries):
- `lsp_completion_items(CompletionContext)` → `Vec<RegistryCompletionItem>` — filters by context (bare identifier vs after-dot)
- `lsp_hover_for_token(name)` → `Option<HoverContent>` — covers all 9 entry kinds

`ynz-registry` has no dependency on `lsp-types` — the LSP crate translates registry shapes to LSP protocol shapes.

### Tests

**1028 tests total, 0 failures.** +198 new LSP-specific tests across the milestone:
- Lifecycle (initialize/didOpen/didChange/didClose/shutdown)
- Diagnostics (WHAT/WHAT-INSTEAD/WHY content, cross-file clear-on-fix, concurrent didChange)
- Completion (registry-driven, receiver-type filtering, deprecated deferred features)
- Hover (all 9 registry entry kinds, user-defined symbols, markdown injection safety)
- Integration sweep (every example fixture: zero-error baseline, error-gallery has-errors, completion/hover no-crash)
- Regression (zero-diagnostic pin, teaching-content pin, LSP-vs-CLI boolean divergence)
- Performance (#[ignore]; cold init <500ms, incremental <100ms, completion/hover <50ms)
- Stdio smoke (full wire-format sequence via real subprocess)

---

## [0.2.0-m1] — 2026-05-20 — Feature Inventory & Sync (SSOT Registry)

Commit range: v0.1.0..v0.2.0-m1
PRs: #37 (P0), #38 (P1), #39 (P2), #40 (P3), #41 (P4), #42 (P5a), #43 (P5b), #44 (P6), #45 (P7), #46 (P8)

### What changed

Before v0.2-M1: feature inventories lived in 7+ scattered locations — `banned_jargon.rs`, `intrinsics.rs`, `check.rs`, `lexer.rs`, `builtins.rs`, `emit.rs`, and a design-doc-only muted-hint catalog. Adding `int.max` in M4 P5 touched five of them. No tool enforced sync.

After v0.2-M1: one file (`registry/features.toml`) is the canonical source for all of those. Every consumer derives from it via `crates/ynz-registry/`'s `build.rs`-driven code generation.

**New crate: `ynz-registry`** — parses `registry/features.toml` at compile time, emits typed static arrays, exposes adapter functions per `*Table` convention.

**`registry/features.toml`** — 158+ entries across 9 entry kinds:
- `[[keyword]]` — 29 valid Yinz keywords with token variant and milestone tag
- `[[banned_declaration_keyword]]` — 17 OOP/concurrency/visibility keywords rejected at lex time with teaching errors
- `[[banned_jargon]]` — 55 words banned from user-facing diagnostic prose (with replacements and reasons)
- `[[primitive_intrinsic]]` — 37 built-in functions/methods on primitive types (print_type, free_fn, method, method_1arg)
- `[[type_attached_constant]]` — 8 constants (`int.max/min`, `float.max/min/epsilon`, `number.max/min/epsilon`) with exact value_literals for both typeck and codegen
- `[[deferred_language_feature]]` — 18 reserved language features with substitute/why/ships_in/design_doc (sized numerics, test, scratch, foreign, gpu, self-referential shapes)
- `[[deferred_tooling_feature]]` — 3 reserved tooling features (`--kernel`, `--release`, package binary format)
- `[[diagnostic_template]]` — 10 canonical WHAT/WHAT-INSTEAD/WHY templates for all DiagnosticKind variants
- `[[muted_hint_domain]]` — 9 IDE inference domains from `.claude/rules/inference.md` (v0.2-M2 LSP wires consumers)

**Migrated consumers** (data removed from Rust, reads from registry):
- `crates/ynz-diagnostics/src/banned_jargon.rs` — thin adapter (5 lines)
- `crates/ynz-typeck/src/intrinsics.rs` — `PrimitiveIntrinsicTable` built from registry at construction time
- `crates/ynz-typeck/src/check.rs` — `type_attached_const_type()` is a registry lookup
- `crates/ynz-codegen/src/emit.rs` — `emit_type_const()` parses `value_literal` from registry
- `crates/ynz-typeck/src/builtins.rs` — `STRING_METHODS` const removed; check.rs reads registry iterator
- `crates/ynz-parser/src/lexer.rs` — deferred-feature handlers read `substitute`/`why` from registry

**New tests** (44 total):
- `crates/ynz-registry/tests/consistency.rs` — 10 tests enforcing invariants across all 9 entry kinds
- `crates/ynz-registry/tests/design_future_sync.rs` — bidirectional sync: every `design/future/*.md` maps to a registry entry or an explicit SKIP with rationale
- `crates/ynz-parser/tests/keyword_sync.rs` — 10 tests pinning keyword + banned-declaration-keyword counts; prevents registry/lexer drift
- `crates/ynz-registry/tests/schema_smoke.rs` — extended with real-entry assertions for each migrated kind

**874 tests total, 0 failures.** All pre-existing fixtures produce identical output.

---

## [0.1.0] — 2026-05-18 — v0.1.0: All M1–M8 language features shipped

Commit range: v0.1.0-m7..v0.1.0

### What's new in M8

M8 closes out v0.1 with six orthogonal pillars that turn Yinz from a single-file
language into a structured, safe, high-precision multi-module language.

**`number<N>` bignum.** `number<N>` for N ∈ (34, 4096] uses a Rust-native
schoolbook decimal arithmetic engine with half-even (banker's) rounding and
IEEE 754-2008 conformance. `0.1 + 0.2 == 0.3` is exact. The `number[N]`
square-bracket syntax produces a migration diagnostic pointing at the correct
`number<N>` form. P8 audit found and fixed: u32→u64 accumulator promotion in
mul to prevent silent carry truncation at high precision; `saturating_sub` in
div to prevent usize underflow; `add_digits` index formula corrected for
right-aligned alignment.

**Multi-file module system.** `yinz.toml` marks the project root. `ynz run <dir>`
compiles and links all `.ynz` files under `src/`. `import { foo } from \`module\``
and `export function/shape/options/const/base` syntax is fully parsed and
type-checked. Cross-file symbol *calls* are deferred to v0.2 (the syntax is
locked and validated; the typeck resolver is a stub). Library files (those with
`export` declarations) do not require an `entrypoint` function.

**Doc comments.** `///` trivia tokens attach to the next declaration (function,
shape, options, field) via the AST's `doc: Option<String>` field. Blank-line
separation (`break_after` flag) correctly orphans isolated comment blocks.

**`sensitive T` type modifier.** `sensitive string` auto-redacts to `[REDACTED]`
in `print()`. `.reveal()` strips the modifier and returns the raw string.
`sensitive(value)` is the constructor. P8 audit fixed: `sensitive string`
function parameters now correctly lower to `Type::Sensitive` instead of
`Type::Error` in codegen; shape fields with `number<N>` types correctly use
a pointer field layout instead of i128 for N > 34.

**`wait` / `background` concurrency keywords.** Both parse and type-check;
in M8 they lower to sequential direct calls (same semantics as an unadorned
call). The compiler reserves the keywords and produces teaching diagnostics for
banned synonyms (`async`, `await`, `promise`, `future`, `goroutine`,
`pub`, `private`, `protected`, `public`). The v0.3 scheduler wires these up
to actual task spawning.

**P8 audit fixes (additional).** Mixed-precision arithmetic: when `number<100>`
and `number<34>` operands appear in a binary expression, codegen now coerces the
N≤34 side to a bignum string before calling `ynz_bignum_*`. The `add_digits`
algorithm in the bignum engine used an incorrect array indexing formula that
caused subtraction overflow for arrays of different lengths; corrected to a
right-aligned offset formula.

### Language surface (M8)

- `number<N>` for N ∈ (34, 4096] — exact decimal arithmetic via bignum engine
- `number[N]` migration diagnostic → `number<N>`
- Multi-file projects: `yinz.toml`, `ynz run <dir>`, `import`/`export` syntax
- `///` doc comments attached to declarations in the AST
- `sensitive T` — auto-redact modifier; `.reveal()` method
- `wait expr` / `background call()` — concurrency keyword reservation
- 9 banned-keyword diagnostics (async, await, pub, private, protected, etc.)

### Runtime and codegen improvements (M8)

- Bignum arithmetic engine (`ynz-numerics` crate): add, sub, mul, div with
  half-even rounding; `ynz_bignum_add/sub/mul/div` C-ABI functions in `ynz-runtime`
- Shape LLVM struct layout: `number<N>` fields with N > 34 now use `ptr` (was `i128`)
- Function parameter types: `sensitive T` and `number<N>` now correctly lower
  in `ast_type_to_typeck_type` and `materialize_param`
- Mixed-precision coercion in `lower_binop`: N≤34 operand auto-converted to
  bignum string when paired with N>34 operand

### Tests (M8)

- 830 tests total (was 737 in M7)
- 93 integration tests including 4 new:
  - `examples_basics_runs_end_to_end` — golden stdout assertion
  - `m8_combo_modules_sensitive_concurrency`
  - `m8_combo_modules_bignum_interpolation`
  - `m8_combo_doc_sensitive_bignum`
- 14 bignum deterministic test vectors (0.1+0.2=0.3, half-even rounding, etc.)

---

## v0.1.0-m7 — Full Strings, errors Keyword, Iterables Protocol

Commit range: v0.1.0-m6..v0.1.0-m7

### What's new

M7 is the largest milestone yet — three interlocking pillars that make Yinz programs
feel like a real language: production-quality strings, a first-class failure-handling
system, and a uniform iteration protocol.

**Full Unicode strings.** Yinz now has one string form: backtick-quoted `` `...` ``
with built-in `${}` interpolation and escape sequences (`\n`, `\t`, `\\`, `` \` ``,
`\${`). The old double-quote form no longer exists — a diagnostic redirects any
`"..."` to the backtick form. String equality uses NFC canonical normalisation, so
`` `é` == `é` `` is always `true` regardless of byte representation. All 16 string
methods ship: `.contains`, `.indexOf`, `.startsWith`, `.endsWith`, `.toUpperCase`,
`.toLowerCase`, `.substring`, `.trim`, `.split`, `.replace`, `.byteAt`, `.get`,
`.graphemeAt`, `.count`, `.byteCount`, `.graphemeCount`. The runtime uses
`simdutf8` for UTF-8 validation and `memchr` for SIMD-accelerated search on
patterns ≥ 16 bytes.

**`errors` keyword.** Functions that can fail declare `-> T errors`. Calling an
`errors` function from another `errors` function auto-propagates on failure —
the happy path reads without any boilerplate. Calling from a non-`errors` function
requires explicit handling: `.or(default)` for a fallback or `.failed()` for an
explicit check. The error value carries `.message`, `.suggestions`, `.trace`
(a call chain as `array<Frame>`), and `.source` (where the failure originated).
The flow-sensitive narrowing engine (M6) is extended to `errors`-capable bindings:
`.message` is only valid inside the `if (x.failed())` true-branch.

**Iterable protocol.** `for` loops now dispatch uniformly through the `Iterable<T>`
and `FallibleIterable<T>` contracts. Built-in collections (`array<T>`, `fixed<T>`,
`map<K,V>`, `string`, `Range`) all follow `Iterable<T>` via synthesized wrappers.
User shapes can implement `Iterable<T>` by writing a standalone `next(lend self: Foo)
-> maybe T` function. `range()` is now first-class — storable, passable, returnable.
The four REPLACE-AT M7 markers left by M5 are all gone. For-loop iteration over
strings walks code points.

### Language surface (M7)

- Backtick-only strings with `${}` interpolation, escape sequences, multi-line
- 16 string methods on the built-in `string` type
- String equality: NFC canonical (`` `é` == `é` `` is `true`)
- `for c in someString` — walks code points
- `-> T errors` function return type — fallible functions
- Auto-propagation: first use of an errors-capable value in an `errors` function
  triggers implicit early-return-on-failure
- `.failed()`, `.or(default)`, `.message`, `.suggestions`, `.trace`, `.source`
- `range()` first-class — `let r = range(0, 10); for i in r { ... }`
- User shapes follow `Iterable<T>` via standalone `next()` function
- `for ((k, v) in m)` tuple-destructure for map iteration

### Design decisions locked

M7 P0 locked 24 design questions before a line of code landed:
- SSO 24-byte string struct layout (ABI locked; tag byte at offset 23)
- SIMD crates pinned: `simdutf8=0.1.4`, `memchr=2.7.4`, `unicode-normalization=0.1.24`, `unicase=2.7.0`
- `Frame { file: string, line: maybe int, function: string }` shape
- `SourceLoc { file: string, line: maybe int }` shape
- Frame stack: thread-local, cap-1024, compile-time-emitted (not libunwind)
- `.orSkipFailures()` is pure (no I/O side effects); `.logSkippedFailuresTo(sink)` for logging
- `.withErrors()` returns `Iterable<maybe T errors>`, not `Iterable<Result<T>>`
- 12 new banned-jargon entries: monad, lift, wrap, Result, Option, Either, exception, try, catch, throw, UTF-16, unwrap

### Compiler features

- **`ynz-parser`**: backtick string lexer with interpolation (brace-depth stack for
  nested `{}`), escape processing, `Token::Errors/BacktickString/InterpolationStart/
  InterpolationEnd` (60→64 tokens); `-> T errors` return type; `Expr::InterpolatedString`,
  `Type::ErrorCapable`, `FunctionDecl.errors_capable`; `for ((k,v) in m)` desugaring
- **`ynz-ast`**: `StringPart` enum; `Expr::InterpolatedString`; `Type::ErrorCapable`
- **`ynz-typeck`**: `Type::ErrorsCapable` (18→20 types); flow-sensitive errors tracking
  (`errors_success_narrowed`, `errors_consumed`); string method dispatch (16 methods);
  interpolation stringifiability check; `Iterable<T>` contract verification for user
  shapes; Frame/SourceLoc built-in field dispatch; range-outside-for restriction removed
- **`ynz-codegen`**: `{i64,i64}` errors ABI (mirrors `maybe<T>`); frame push/pop at
  function entry/exit; auto-propagation branch emission; string method call lowering;
  interpolated string builder (`ynz_string_builder_*`); string iteration via
  `ynz_string_codepoint_at`; first-class range as `{i64,i64}` struct; user shape
  `next()` dispatch
- **`ynz-runtime`**: `YnzError`/`YnzFrame` structs; `ynz_frame_push/pop` (cap-1024);
  `ynz_error_new/drop/message`; `ynz_unhandled_error`; NFC equality via
  `unicode-normalization`; 17 string method functions; SIMD search via `memchr`;
  locale-invariant case via `unicase`; `ynz_string_builder_*` family

### Tests

782 tests across 7 crates, all passing. New in M7: 12 lexer tests, 19 parser tests,
29 errors typeck tests, 28 string typeck tests, 21 iterable typeck tests, 6 runtime
unit tests, and 16 integration fixtures (basic/propagation/nested errors, string
methods/interpolation/NFC/boundary, string iteration, first-class range, user
iterables, adversarial OOB/empty/nested).

---

## v0.1.0-m6 — Options + Unions + Narrowing

Commit range: v0.1.0-m5..v0.1.0-m6

### What's new

M6 ships type-driven discrimination — the ability to declare finite sets of named
states (`options`), work with values that can be one of several distinct shapes
(`|` union types), and discriminate between them at compile time with exhaustiveness
checking and flow-sensitive narrowing.

M6 also closes the fallible-conversion catch-up from M2: `(float).toInt()`,
`(number).toInt()`, `string.toInt()`, `string.toFloat()`, and `string.toNumber()`
all return `maybe<T>` and follow locked parsing rules documented in `design/narrowing.md`.

- **`options` types**: `options Status { active, inactive, banned }` declares a finite
  set of named values. Values are `Status.active` etc.; multi-case `if` is exhaustive
  (missing variants are compile errors naming each missing variant). Built-in:
  `SortOrder { asc, desc }` and `Comparison { equal, greater, less }`.
- **`options.toString()`**: returns the variant name as a string at runtime.
- **Union types**: `shape Figure = Circle | Square | Triangle` declares a value that
  can hold any of the listed shapes. `|` in type position. Exhaustive multi-case
  `if` with `is TypeName =>` arms; `else =>` as catch-all.
- **`is`-narrowing**: inside an `is Circle =>` arm, the scrutinee's type is narrowed to
  `Circle` — field access is safe without any cast or `.value`. Works in both multi-case
  form (`if (x) { is Foo => ... }`) and condition form (`if (x is Foo) { ... }`).
- **Shape aliases**: `shape Figure = Circle | Square` declares a named union type using
  the existing `shape` keyword — one keyword for all type declarations.
- **Fallible conversions (M2 catch-up)**:
  - `(int).toInt()` → `int` (identity, infallible)
  - `(float).toInt()` → `maybe<int>` (NaN → none, OOR → none, truncates toward zero)
  - `(number).toInt()` → `maybe<int>` (via decimal128 → float → range-check → truncate)
  - `string.toInt()` → `maybe<int>` (ASCII whitespace strip; `[+-]?[0-9]+` only; no hex/decimal)
  - `string.toFloat()` → `maybe<float>` (decimal + scientific notation; no 0x/0o/0b)
  - `string.toNumber()` → `maybe<number>` (same rules as `.toFloat()`)
- **Early-return narrowing (M5 catch-up)**: `if (!m.exists()) { return }` followed by
  `m.value` is now valid — the compiler proves `m` is non-none after the early exit.
- **M3 catch-up**: `m3_is_type_deferral.ynz` is now a runnable `Circle | Square` union
  demo; the M3 deferral diagnostic is gone.

### Design decisions locked

Three new design files document every M6 decision before any code landed:
`design/options.md` (LLVM i8 lowering, exhaustiveness, ambiguous-shorthand resolution),
`design/unions.md` (tagged-struct layout, `is`-exact-type rule, single-variant rejection),
`design/narrowing.md` (18-row flow-sensitive rules table, recognized-exit set, locked
`||` non-propagation diagnostic text).

### Compiler features

- **`ynz-parser`**: `Token::Options`, `Token::Is` (58→60 tokens); options declaration
  parser; union type in type position; `Is`/`OptionName` arm forms; `Expr::Is` for
  `if (x is Foo)` condition form; `shape Name = Type` alias form; M3 deferral removed.
- **`ynz-ast`**: `Item::OptionsDecl`, `Type::Union`, `TypePath`, `MatchPatternKind::Is`/
  `OptionName`, `Expr::Is`, `ShapeDecl.alias_ty`.
- **`ynz-typeck`**: `OptionsTable` (collection + validation); union alias resolution via
  `ShapeTable.union_aliases`; options/union exhaustiveness; `is`-narrowing; early-return
  narrowing accumulator in `check_stmts`; fallible conversion intrinsics; `check_is_expr`.
- **`ynz-codegen`**: options i8 constants + multi-case switch + `toString` via conditional
  branch to `ynz_string_from_static`; union `{ i64 tag, i64 data }` construction on
  assignment; `Is`-arm tag load + compare; `(float).toInt()` locked IR sequence
  (`fcmp uno` + range-check + raw `fptosi` — NOT `fptosi.sat`); string conversion dispatch.
- **`ynz-runtime`**: `ynz_string_to_int/float/number` (locked parsing rules), `ynz_string_from_static`,
  `ynz_decimal_to_float`.

### Tests

631 tests across 8 crates, all passing. New in M6: 24 runtime unit tests for
string-parsing locked test vectors; 13 typeck tests for options/union semantics;
2 new integration tests for string-conversion catch-up fixtures; 7 new parser tests.


## v0.1.0-m4 — Shapes, Methods, Ownership

Commit range: v0.1.0-m3..v0.1.0-m4

### What's new

- **Shape declarations**: `shape Foo { field: Type }` defines a user data type.
  All fields are required in struct literals. Structural typing: `let p: Player = { name: "x", health: 1 }`.
- **Methods via UFCS**: standalone functions with `self: ShapeName` as first param
  are callable as `value.method()` or `method(value)`. Both are equivalent.
  Yinz is not object-oriented — methods live outside shape bodies.
- **Ownership modifiers**: `share self` (read-only borrow), `lend self` (mutable borrow),
  `give p: T` (ownership transfer). Inferred at call sites; declared in signatures.
- **Ownership analysis**: `const` bindings block all mutation paths. Use-after-give
  is a compile error with both give-site and use-site named in the diagnostic.
- **`extends` (data-only inheritance)**: child inherits parent fields prepended to its own.
- **`follows` (structural contracts)**: verified at compile time against standalone functions.
- **`base shape`**: cannot be instantiated; must be extended. Compile error on attempt.
- **`hidden` fields**: visible only inside the declaring shape's own methods.
- **LLVM ownership attributes**: `share T` → `readonly + noalias`; `lend T` → `noalias`;
  `give T` → neither. Verified by IR snapshot.
- **Runtime shims**: `ynz_alloc` / `ynz_free` added; stack allocation used by default.
- **`.copy()` and `.freeze()`**: trivial struct memcopy; binding mutability lock.
- **M2 catch-up — overflow escape**: `.wrappingAdd/Sub/Mul()` and `.saturatingAdd/Sub/Mul()`
  on `int` via LLVM wrapping arithmetic and `sadd.sat` / `ssub.sat` / `smul.fix.sat`.
- **M2 catch-up — type-attached constants**: `int.max`, `int.min`, `number.epsilon`,
  `number.max`, `number.min`, `float.max`, `float.min`, `float.epsilon`.

### Test count

M3: 310 tests → M4: **316 tests** (added 6 positive + 10 negative M4 integration
fixtures, codegen golden tests, jargon audit).

### Breaking changes

None — all M3 programs compile unchanged under M4.

---

## v0.1.0-m3 — Control Flow + User Functions

### What's new

- **User-defined functions**: Multiple functions per file, parameters with type
  annotations, return types declared on every function, early `return` statements,
  mutual recursion supported via two-pass signature pre-pass.
- **`if` statement**: `if (condition) { body }` with no standalone `else` block —
  early-return and pre-assignment patterns handle alternation.
- **Multi-case `if`**: `if (scrutinee) { 1 => ...; 2 => ...; else => ... }` for
  value-based branching on `int`, `string`, `float`, and `bool`. String comparison
  uses byte-equality via `ynz_string_eq` (Unicode canonical equivalence in M7).
- **`while` loop**: `while (condition) { body }` with full type checking on the bool condition.
- **`for` loop**: `for (i in range(0, n)) { body }` with a temporary `range` builtin
  (replaced by `Iterable[T]` protocol in M7). Loop variable is immutable inside the body.
- **Block scoping**: each `{}` block pushes/pops a scope; shadowing is allowed.
- **Return-path analysis**: non-`nothing` functions must return on every path or get
  a compile error naming the uncovered path. Dead code after a definite `return`
  emits a warning.
- **Parameter read-only enforcement**: assignment to a parameter is a compile error
  with an M4-deferral diagnostic pointing at the `lend` ownership modifier.
- **Dead-code warnings**: code after a definite return renders to stderr even on
  successful builds.
- **Deferred-feature teaching diagnostics**: `is TypeName =>` arms point to M6,
  `share`/`lend`/`give` parameter annotations point to M4, `range` outside
  for-loop position points to M7.
- **`match`/`switch` banned-keyword diagnostics**: teaching messages redirect to
  multi-case `if`.

### Compiler internals

- Two-pass typeck: `module_signatures_query` (salsa) collects all function
  signatures before any body is checked; body typeck depends on this query for
  cross-function call site resolution.
- Return-path analysis in `crates/ynz-typeck/src/return_paths.rs`: pure CFG walk
  over `Block`, no typeck context needed. 7 dedicated unit tests.
- LLVM codegen: two-pass `build_module` forward-declares all functions first
  (mutual recursion), then emits bodies. `lower_stmt_{if,match,while,for,return}`
  helpers. All control-flow uses `alloca`-per-local for uniform variable model;
  LLVM mem2reg elides copies.
- `ynz_string_eq` added to `libynz_rt.a` (pointer arithmetic only; kernel-mode safe).
- Linker now passes `-no-pie` on Linux to match LLVM's non-PIC object output
  (PIE vs non-PIE relocation alignment fix).

## v0.1.0-m2 — Literals, Variables, Arithmetic

### What's new

- **Numeric types**: `int` (i64), `float` (f64), `number` (IEEE 754 decimal128, hand-rolled from scratch with full conformance test suite)
- **Variables**: `let` and `const` declarations with optional type annotations; block-scoped; Levenshtein "did you mean" suggestions on undefined names
- **Arithmetic**: full operator set (`+`, `-`, `*`, `/`, `%`), integer overflow panics, float follows IEEE 754 (no panic on infinity), decimal exact arithmetic (`0.1 + 0.2 == 0.3`)
- **Comparisons and booleans**: `<`, `<=`, `>`, `>=`, `==`, `!=`, `&&`, `||`, `!`, short-circuit evaluation
- **Bitwise operators**: `&`, `|`, `^`, `~`, `<<`, `>>`
- **Type inference**: `let x = 42` infers `int`; `let x = 3.14` infers `number`; annotation overrides default
- **Mixed-type errors**: `int + number` is a compile error with a specific `.toNumber()` suggestion; `number + float` lists both conversion directions and explains the tradeoff
- **Conversion methods**: `.toNumber()`, `.toFloat()`, `.toString()` on all primitive types
- **Polymorphic `print`**: accepts `int`, `float`, `number`, `bool`, and `string`
- **Comments**: `//` line comments

### Compiler internals

- Pratt precedence climber (12-level table, mechanically verified against `spec/operators.md`)
- `PrimitiveIntrinsicTable` replaces M1's `BuiltinTable`; single source of truth for all built-in method dispatch
- Block-scoped variable environment with `is_const` tracking
- LLVM codegen for all M2 constructs: int overflow via `llvm.sadd/ssub/smul.with.overflow.i64`, decimal128 via `ynz-runtime` C ABI, short-circuit `&&`/`||` with phi nodes
- Runtime panic stubs for overflow and division by zero (three-part diagnostic to stderr + abort)
- `expr_types` keyed by `(span.start, span.end)` — fixes span collision between BinOp parent and leftmost child

### Spec

- `spec/operators.md`: added `%` to operator lists and precedence table (level 3)
- `spec/variables.md`: corrected `// compiler knows: number` → `// compiler knows: int`
- `spec/numeric-types.md`: replaced wrong "promotes to most capable" claim with compile-error behavior + example

### Deferred (tracked as catch-up entries)

- `number[N]` for N > 34 (bignum) — M8
- Overflow escape valves (`.wrappingAdd()` etc.) — M4
- Fallible conversions (`.toInt()`) — M6
- Type-attached constants (`int.max`) — M4

### Integration test

```
$ ynz run m2_smoke.ynz
0.3
1763
true
```

---

## v0.1.0-m1 — Hello World

- `ynz run hello.ynz` → `hello, yinz`
- Full pipeline: lex → parse → typecheck → LLVM codegen → link → execute
- All passes wired as salsa queries for incremental rebuilds
- Three-part diagnostic format (WHAT / WHAT-INSTEAD / WHY) with ariadne rendering
- Banned-jargon CI gate
