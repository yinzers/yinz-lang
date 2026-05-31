---
slug: v0-3-m2-wait-and-state-machines
type: execution
owner: Patrick Rizzardi
status: active
created: 2026-05-30
last_updated: 2026-05-31 (P0 ✅committed 6328666, P1 ✅committed b740e3d. P2 codegen IMPLEMENTED but UNCOMMITTED (staged) + code-reviewer BLOCK — 2 confirmed correctness bugs: (#1) `wait` inside if/while/for is a silent no-op (flat statement-split, not CFG transform); (#3) `background sm_fn()` leaks the heap frame. Happy-path top-level `wait` works (concurrency proof passes, top-level wait=124ms). DECISION (Patrick 2026-05-31): FIX PROPERLY (real stackless-coroutine transform). P2-FIX DESIGN locked in Phase 2 Findings Log — 3 increments: (A) wait-in-if/if-cond, (B) wait-in-loop w/ induction-var-in-frame, (C) locals-crossing-wait → frame slots; plus #2 snapshot behavioral asserts, #3 SpawnStateFnFuture Drop, #4 param_to_i64_bits collapse. Verify each increment with a BARE-BINARY TIMING test (golden IR snapshots froze bug #1 — timing is the only no-op catcher). PROGRESS (uncommitted, all verified): #4✅(one marshaller) #3-frame✅(SpawnStateFnFuture::Drop) Increment-A✅(wait-in-if: 86ms real suspend, IR poll+live sm_pending, edge cases pass). NEXT: AWAITING SCOPE DECISION (clarification requested) — Option B clean-error+defer-loops-to-M3 [RECOMMENDED] vs Option A full-coroutine-transform-now. See Phase 2 Findings "SCOPE DECISION PENDING". Verified: loops/let-crossing-wait = M3-scale (lower_stmt_for is 566 lines × ~5 iteration forms + needs frame-backed-mutable-locals subsystem); M2 demo uses TOP-LEVEL waits only. After decision: finish remaining Phase 2 fixes, then /execute-plan runs reviewers + carries P3/P4/P5 straight through (all-phases-then-review). Do NOT commit P2 until done + review re-run. EXECUTING in worktree on branch v0.3-m2-wait-and-state-machines, whole-plan mode.)
roadmap: v0-3-concurrency-perf
files:
  - crates/ynz-runtime/**
  - crates/ynz-codegen/src/**
  - crates/ynz-typeck/src/**
  - crates/ynz-driver/src/**
  - crates/ynz-driver/tests/**
  - crates/ynz-lsp/src/**
  - crates/ynz-diagnostics/src/**
  - crates/ynz-registry/**
  - registry/features.toml
  - tooling/vscode-ynz/**
  - examples/pirates-roster/entrypoint.ynz
  - examples/primantis-orders/v0_3_m2_errors.ynz
  - design/stdlib/filesystem.md
  - design/stdlib/network.md
  - design/stdlib/database.md
---

# Plan: v0.3-M2 — `wait` Suspension + State Machine Codegen

Created: 2026-05-30
Status: APPROVED (Patrick, 2026-05-30, after plan-reviewer Rounds 1-5 → PASS)
Roadmap: `v0-3-concurrency-perf`

## Context & Why

**Goal**: Make explicit user-written `wait expr` actually suspend the function and yield the OS thread back to the scheduler — instead of blocking the thread for the duration of the awaited operation. After M2, a function containing `wait sleepAsync(200)` pauses for 200ms WITHOUT holding an OS thread captive; the freed thread runs other ready tasks during the pause; the function resumes (possibly on a different thread) when the timer fires.

**Why**: v0.3's positioning is "Rust-level performance with TypeScript-level readability — no function coloring." M1 made `background` run on a separate thread; M2 makes `wait` correctly suspend instead of blocking. Without M2, every concurrent `wait` consumes an entire OS thread doing nothing — 1000 background tasks each `wait sleepAsync(100)` would need 1000 threads (impossible) instead of running on the native ~num_cpus thread pool. M3 then ships may-block analysis + auto-`wait` insertion, eliminating the last vestige of user-visible coloring. M2 is the load-bearing infrastructure milestone that M3 builds on.

**Background — what M1 left in place**:

- `wait expr` parses (M8), typechecks (M3 → carries through M1's kernel-mode rejection), and lowers as identity (`emit.rs:3139` — `Expr::Wait(inner, _) => lower_expr(cg, inner)`). The keyword exists; the runtime behavior is "pass-through."
- `ynz_rt_init` / `ynz_rt_shutdown` / `ynz_rt_spawn_blocking` / `ynz_rt_check_preempt` ship in `libynz_runtime.a` (M1 P1). `ynz_rt_spawn` (the non-blocking I/O-pool spawn) does NOT exist yet.
- `sleepMs(int) -> nothing` ships as an intrinsic (M1 P2). It calls `std::thread::sleep` — blocks the calling OS thread. Stays exactly as-is in M2 (it's the right tool for "block the thread" use cases like the M1 background-timing demo).
- Tokio multi-thread runtime + blocking thread pool are already booted. The I/O-pool spawn just isn't wired through yet.

**Constraints** (from roadmap):

- **No function coloring.** Every function is still just `function`. No `async fn` distinction in the source. The compiler decides which functions become state machines based on whether their body contains `Expr::Wait` — a local syntactic check, not a transitive one (transitive may-block analysis is M3).
- **`wait` semantics preserved exactly.** Code after `wait expr` does not run until `expr` completes. This is identical in M1, M2, M3. M2 only changes what the OS thread does WHILE waiting (releases vs holds).
- **`block_on` bridge keeps the no-coloring promise during the M2→M3 transition.** When a function that doesn't contain `wait` calls a function that DOES contain `wait` (i.e., a state machine), the call site uses tokio's `block_on` — the calling thread blocks until the future completes. Correct behavior, suboptimal performance. M3's may-block analysis + auto-`wait` insertion replaces `block_on` call sites with proper suspension points, at codegen level — zero source change required.
- **Existing programs produce identical stdout/stderr/exit-code.** Verified by the cross-impl consistency harness extended from M1 (`--no-auto-parallel` flag is no-op in M2 too; gets meaningful behavior in M3).
- **No GC, no per-task OS stack.** State machines are stackless coroutines (Rust-async model). Frame allocated via `ynz_alloc` on spawn; freed via the closure's RAII drop guard when the state machine completes (whether normally or via panic).
- **`--kernel` mode still rejects `wait` and `background`** (carries forward from M1; no Tokio in kernel mode).
- **Full teaching surface ships in M2** — registry hover-doc updates (the M1 entry pointed forward to M2 semantics; now those semantics are real), new diagnostic templates, LSP wiring, VSCode extension bump + screenshot, demo extension, error gallery.

**Success criteria**:

- A function containing `wait sleepAsync(200)` PAUSES for ~200ms but the underlying OS thread is FREE during that pause. Verified by: spawn 8 background tasks each calling `wait sleepAsync(100)`; total wall-clock to all-complete ≤ 150ms on a 4-core CI runner (proves they share threads, don't each hold one).
- A function returning `T errors` with a `wait expr` in the middle correctly propagates errors through suspension. Verified by: a fixture using `__testFallibleAsync()` that exercises the success path AND the error-cascade path through a state-machine frame.
- A non-`wait` caller of a state-machine function uses `block_on` and gets correct behavior (blocks until completion). Verified by: a fixture where main calls a state-machine fn directly (no `wait`), main blocks for the full duration but completes correctly.
- `cargo test --workspace` passes (existing 1220+ tests + new M2 tests).
- `ynz build --no-auto-parallel hello.ynz` and `ynz build hello.ynz` produce identical stdout/stderr/exit-code on every fixture (cross-impl consistency, carried forward from M1).
- `examples/pirates-roster/entrypoint.ynz` has a v0.3-M2 section that demonstrates 8 background tasks each `wait sleepAsync(100)`-ing concurrently — all finish in ~100ms wall-clock, NOT 800ms (8 demonstrates thread-sharing visibly on multi-core CI per Round 2 Concern #5).
- `examples/primantis-orders/v0_3_m2_errors.ynz` triggers every new compile error/warning class introduced in M2.
- Tag `v0.3.0-m2` cut via `/release`; VSCode extension republished as `yinz-0.3.0-m2.vsix` and `yinz-latest.vsix`.

---

## Research Findings

- **Current `Expr::Wait` codegen** (`crates/ynz-codegen/src/emit.rs:3139`): pass-through to the inner expression. M2 replaces this with the state-machine resume-point insertion when the enclosing function contains any `wait`.
- **Current `Expr::Wait` typeck** (`crates/ynz-typeck/src/check.rs:1261-1271`): infers inner type, emits kernel-mode rejection, otherwise transparent. M2 extends this to (a) flag the enclosing function as "contains wait" (used by codegen routing) and (b) emit a "wait on non-may-block expression" warning when the wrapped call's callee is not a may-block intrinsic.
- **`ynz_rt_spawn_blocking` runtime ABI** (`crates/ynz-runtime/src/runtime.rs:110`): takes `extern "C" fn(*mut u8)` + ctx pointer + size. Tokio's `spawn_blocking` invokes the closure on the blocking thread pool. M2 adds a parallel `ynz_rt_spawn` that takes a state-machine `resume()` function pointer + frame pointer and uses `tokio::task::spawn` (work-stealing I/O scheduler) — the difference matters because state machines must be polled, not just called once.
- **Tokio Future polling protocol** (research from `tokio` 1.x docs):
  - `Future::poll(self, cx: &mut Context) -> Poll<Output>` is the trait.
  - A state machine emitted by Yinz codegen implements this as: take a `*mut StateFrame` ctx + `&mut Context` waker → switch on the frame's `resume_point` discriminant → execute statements between resume points → on `wait`, save state, register waker, return `Poll::Pending` → on completion, return `Poll::Ready(output)`.
  - Tokio's `spawn` wraps a `Future` in a task; the runtime polls it whenever the waker is woken (timer fires, I/O ready, etc.).
- **`tokio::time::sleep(Duration)`**: returns `Sleep`, a `Future<Output = ()>` that yields control until the deadline. M2's `sleepAsync(int)` Yinz intrinsic wraps this — the codegen emits IR that calls `ynz_rt_async_sleep_create(ms) -> *mut SleepHandle`, then the state-machine `wait` lowering polls the handle and yields/wakes through Tokio's standard interface.
- **`tokio::runtime::Runtime::block_on(future)`**: synchronously drives a future to completion on the current thread. M2's `block_on` bridge invokes this when a non-state-machine caller calls a state-machine function — wraps the called function's state-machine `resume()` in a Future and runs it to completion.
- **Existing `lower_function`** (`crates/ynz-codegen/src/emit.rs`): the standard codegen path. M2 adds `lower_function_with_waits` parallel to it. Selection: if `body` contains any `Expr::Wait` (recursive AST walk during pre-typeck → cached on the function table), use the state-machine path; else use the standard path.
- **State machine frame layout** (research from Rust async desugaring + Tokio docs):
  - Heap-allocated struct via `ynz_alloc(frame_size)` at task-spawn or `block_on` time
  - Fields: `resume_point: i32` (which state to resume into); each live local across a wait boundary; the awaited handle's storage slot
  - Frame freed via `ynz_free` inside the closure RAII guard (same pattern as M1 `ynz_rt_spawn_blocking`'s `CtxDropGuard`) — drops on completion AND on panic.
- **`errors` keyword integration** (`design/errors.md` + M7 implementation): `errors`-returning functions use a `{i64, i64}` ABI (value + error-frame tag). State-machine functions returning `T errors` must thread this through `Poll<Result<T>>` → at the IR level, `Poll::Ready` carries `{i64, i64}` payload; `Poll::Pending` carries no payload but the frame's resume-point preserves where the error-handling logic resumes.
- **`design/stdlib/filesystem.md`** currently has no `async`/`wait` references — M2 P4 adds a "Deferred to v0.5: async I/O surface — `readFileAsync`, `writeFileAsync`, etc. — backed by tokio::fs" stub note so future-us doesn't reinvent this conversation. Same for `network.md` and `database.md`.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| State machine LLVM IR is materially more complex than the M1 spike anticipated; P0 spike fails | Medium | High | **P0 is a hard accept/reject gate.** Five contracts must hold before any production code lands: single-wait function, multi-wait sequential, wait-inside-if branching, block_on bridge correctness, errors-cascade through state machine. If any fails, halt and escalate to user with detail. M1's spike-gate pattern carries forward — failing fast at P0 is much cheaper than discovering the design's wrong mid-P2. |
| Tokio `Future` polling protocol mismatches Yinz state machine ABI (waker registration, drop-on-cancel, panic propagation in poll) | Medium | High | P0 hand-writes the simplest case in raw LLVM IR (or Rust that mimics what codegen will emit) and validates end-to-end against Tokio's `spawn` + `block_on`. Includes the panic path: panic inside `poll()` is caught by Tokio's task wrapper; state machine frame's RAII guard frees memory. |
| `block_on` bridge causes thread starvation when many concurrent non-`wait` callers invoke state-machine functions (each `block_on` ties up a thread) | Medium | Medium | **Documented limitation**, not a bug. M3's auto-`wait` insertion eliminates `block_on` call sites at codegen level — no source change. M2 ships with a CHANGELOG note explaining the M2→M3 evolution. Mitigation in the meantime: use `background fn()` for any function you don't want to block on. Worst case in M2: ~num_cpus simultaneous `block_on`s sustainable; beyond that, thread-pool exhaustion → tasks queue. M2 tests do NOT exercise > 16 simultaneous `block_on` calls (matches typical CI core count). |
| `errors`-returning state-machine functions corrupt the error-trace frame on suspension/resume | Medium | High | P0 spike contract #5 specifically exercises `__testFallibleAsync()` returning success AND failure through a state-machine boundary; asserts `errors`-trace `Frame.line` / `SourceLoc` data is intact after resume. M7 shipped `Frame`+`SourceLoc` with `{i64, i64}` ABI — state-machine frame stores them as plain i64 slots. |
| `lower_function_with_waits` codegen takes >10× longer than `lower_function` for trivial functions | Low | Medium | Path selection happens early (function flagged as "contains wait" during AST walk). Functions without `wait` use the existing fast path unchanged. Measure: typeck+codegen wall-clock on `examples/pirates-roster/` corpus before/after M2. Threshold: ≤ 10% overall slowdown on the corpus (most functions don't contain wait). |
| `sleepAsync(int)` Yinz intrinsic ships an API the future stdlib `wait` module will want to rename | Low | Low | **Locked: stays in stdlib/concurrency forever.** `sleepAsync` is the canonical name for "non-blocking sleep" — paired with `sleepMs` (blocking sleep) — consistent with the M1 naming decision (rationale in M1 `runtime_decls.rs` near `ynz_thread_sleep_ms`). The "Async" suffix matches the muted-hint convention; not jargon (already widely understood) and explicitly approved here. |
| `__testFallibleAsync()` internal intrinsic accidentally leaks to user-facing docs / autocomplete | Low | Medium | Double-underscore prefix matches Python/Java/C convention for "internal." Registry entry is OMITTED (not just hidden) — the LSP can't autocomplete it. Filed under `crates/ynz-typeck/src/intrinsics.rs` with comment `// INTERNAL — DO NOT REGISTER. Used by M2 state-machine codegen tests only. Deletes when v0.5 ships real fallible async I/O intrinsics.` Visibility test: P3 includes an LSP completion test that asserts `__testFallibleAsync` does NOT appear in any completion list. |
| State-machine frame size grows unboundedly with function-local complexity (Rust async has hit ~10KB frame size for deeply-nested awaits) | Low | Medium | M2 frame layout is "one slot per live local across a wait boundary." Functions without `wait` have zero overhead. Functions with `wait` get exactly the slots they need. P0 spike includes a 5-wait sequential test that measures frame size; threshold: frame ≤ 256 bytes for the spike fixture. Frame-size lint deferred to v0.4 linting tier per `design/mvp-scope.md`. |
| Cross-impl consistency harness (M1) breaks on M2-introduced state-machine fixtures | Low | Low | Same exclusion mechanism M1 introduced: `const TIMING_DEPENDENT_FIXTURES: &[&str]` allowlist at the top of `cross_impl_consistency.rs`. M2 fixtures using `sleepAsync` join the list; the harness still covers the rest of the corpus byte-for-byte. M3 wires the harness to real auto-parallel codegen at which point richer assertions become possible. |
| Background-spawn routing (state-machine fn → `ynz_rt_spawn`, regular fn → `ynz_rt_spawn_blocking`) gets the wrong route and a CPU-bound background task starves the I/O scheduler | Low | High | Decision logic: a function is "state machine" iff its AST contains `Expr::Wait` (local syntactic check, NOT transitive). If a wait-free function is spawned via `background`, it ALWAYS routes to `ynz_rt_spawn_blocking` (existing M1 behavior). Transitive may-block analysis (M3) refines this. Integration test: `background_routing_decision` asserts a 4-thread CPU-bound loop background-spawned does NOT block I/O timers. |
| `main` containing `wait` + existing `ynz_rt_init/shutdown` interact badly (block_on inside main, runtime already booted) | Medium | Medium | P0 spike contract #1 specifically validates: `main` becomes a state machine if it contains `wait`; codegen emits `runtime.block_on(main_state_machine_resume(...))` at main's entry; existing `ynz_rt_init` is called BEFORE block_on, `ynz_rt_shutdown` AFTER. Main's exit code propagates through the future's `Output` type. |
| State-machine local variables containing heap pointers (strings, arrays, maps) don't survive suspension correctly (frame slots are i64; fat pointers are ptr+len) | Medium | High | P0 spike contract #8 (new per round-1 adversarial case 7) — hand-write a state machine whose live local across a wait boundary is `string` (a 16-byte SSO+heap struct per M7 ABI). Assert: post-resume, the string's length, byte content, and SSO/heap discriminant are all intact. Frame layout strategy: each non-scalar local crossing a wait gets a slot sized to the type's stored size (NOT just i64). State machine codegen emits per-type slot sizing — locked at P0 spike. |
| `wait` of a `wait` (nested) — what does codegen emit? | Low | Medium | Decision: this is a `wait_on_non_call_expression` error in M2. `wait expr` requires `expr` to be a `Call`/`MethodCall`/`FreeFn-call`; a nested `Expr::Wait` is none of those. The existing P3 typeck check catches it. Test: `wait_of_wait_rejected` triggers `wait_on_non_call_expression`. |
| Concurrent `block_on` from multiple thread contexts → Tokio panic (`block_on` inside another `block_on` is documented; `block_in_place` from blocking-pool is documented) | Medium | High | M2 ships `ynz_rt_call_state_machine_sync` shim per Question Resolution #9 (Shape B): `match Handle::try_current() { Ok(h) => h.block_on(future), Err(_) => RUNTIME.block_on(future) }`. NO `block_in_place` — Round 3 Required Fix #1 confirmed `block_in_place` panics on `spawn_blocking`-pool threads even though `Handle::try_current()` returns Ok. `Handle::block_on` works on every thread context (worker / spawn_blocking-pool / no-runtime). Cost: scheduler pressure when called from worker (ties up worker for wait duration). Validated by P0 Spike Contracts #4a (worker), #4b (spawn_blocking-pool), #4c (no-runtime), #4d (state-machine inside state-machine reaching nested `block_on`). |
| `wait_required_on_state_machine_call` typeck check false-positives on `background state_machine_fn()` from inside a state machine | Medium | Medium | P3 Step 8 passes `inside_background: bool` down typeck recursion. When the call is the immediate inner of `Expr::Background`, the check is exempted — `background smB()` from inside a state machine is the legal route-to-I/O-pool pattern (P2 Step 5). Test: `state_machine_can_background_state_machine_without_wait` validates compile-clean + runtime concurrent execution. |
| Background-spawn of a regular fn that internally calls a state-machine fn (transitive case the local-check doesn't see) | Low | Medium | M2 codegen uses `ynz_rt_call_state_machine_sync` Shape B everywhere a synchronous bridge fires — INCLUDING inside the body of a regular fn called from `ynz_rt_spawn_blocking`. The shim uses `Handle::block_on` (NOT `block_in_place`), which is documented to work on blocking-pool threads. Test: `background_regular_fn_that_internally_calls_state_machine_does_not_crash` integration test in P5. |
| Background of a state-machine fn returning `T errors` — error is discarded; user has no way to observe failure | Medium | Medium | M2 carries M1's `background` semantic: fire-and-forget, panic+error discarded (logged via `eprintln!`). Same for errors-returning state-machine fns spawned via background. Documented in registry hover doc for `background` keyword: "`background` discards return values AND errors. Use the handle form (`let h = background fn()`) in v0.3-M4 to observe results." Test: `background_state_machine_fn_with_error_does_not_propagate` validates main reaches its marker even when the spawned task errors. |

---

## Questions

Resolved before drafting (via Patrick AskUserQuestion conversation 2026-05-30):

1. **wait-callsite behavior when caller doesn't write `wait`** → `block_on` bridge. Locked per Architectural Decisions. WHY: only no-function-coloring-preserving option in M2; M3's auto-`wait` insertion lifts it transparently with zero source change.
2. **M2 public intrinsic scope** → `sleepAsync(int) -> nothing` only. Real async file/net/db intrinsics deferred to v0.5+ stdlib modules. WHY: minimal surface to validate the state-machine ABI end-to-end; avoids committing to a public file-I/O API the v0.5 file module will need to revisit.
3. **errors+wait validation in M2** → internal-only `__testFallibleAsync(bool) -> int errors` intrinsic. Never registered, never advertised. WHY: errors-through-state-machine is the hardest ABI interaction; M2 must validate it or M3 / v0.5 will discover the ABI is broken under deadline pressure. Synthetic internal intrinsic validates without committing to a public stdlib API.
4. **State-machine ABI / frame model** → heap-allocated frame (Tokio convention), stackless coroutine, `resume(ctx, waker)` C-ABI bridge. P0 spike validates against Tokio's `Future::poll`.
5. **`background` routing in M2** → state-machine function (body contains `wait`) → `ynz_rt_spawn` (I/O pool); regular function → `ynz_rt_spawn_blocking` (existing M1 behavior). Local syntactic check, NOT transitive (M3 refines).

Resolved during plan-reviewer Round 1 + Round 2 (2026-05-30) — see `## Reviewer Disputes`:

6. **state-machine fn calling state-machine fn WITHOUT `wait`** → **TEACHING WARNING** (Tier 3) per `wait_required_on_state_machine_call` diagnostic (P3 Step 8). **Reclassified from error to warning per Round 2 + Round 3** because Shape B of the `ynz_rt_call_state_machine_sync` shim (Question Resolution #9) makes the runtime path panic-safe at scheduler-pressure cost. The warning steers users toward writing `wait` (which uses inline poll-and-yield, no shim, no scheduler pressure) but is NOT a correctness gate — programs that ignore the warning still run correctly. M3's auto-`wait` insertion eliminates the warning entirely with no source change. Exempted when the call is the immediate inner of `Expr::Background` (background routes to `ynz_rt_spawn`, no shim involved).
7. **M2 may-block predicate** → in-code constant set: `const M2_MAY_BLOCK_INTRINSICS: &[&str] = &["sleepAsync", "__testFallibleAsync"]` lives in `crates/ynz-typeck/src/intrinsics.rs`. Predicate: "callee is a name in `M2_MAY_BLOCK_INTRINSICS` AT THE SYNTACTIC CALL SITE." Local, intrinsic-set check. NOT transitive. WHY: avoids adding a `may_block: bool` schema field to `FreeFnSig` (registry change) for a 2-element set; M3's transitive analysis ships the proper field then.
8. **`__testFallibleAsync` registration mechanism** → new `internal_fns: Vec<(&'static str, FreeFnSig)>` field on `PrimitiveIntrinsicTable` (NOT `#[cfg(test)]`-gated, because cross-crate driver tests need it at production-typeck time). Lookup helper `lookup_free_fn_including_internal(name, arg_count) -> Option<&FreeFnSig>` searches both `free_fns` and `internal_fns`. LSP completion handler (`crates/ynz-lsp/tests/completion.rs`) uses only `free_fn_names()` (which doesn't include `internal_fns`). WHY: existing `#[cfg(test)] test_fns` mechanism is wrong for cross-crate use; the new `internal_fns` field gives production typeck access while keeping LSP autocomplete clean.
9. **Runtime-aware sync bridge — Shape B (revised per plan-reviewer Round 3 Required Fix #1)** → new runtime shim `ynz_rt_call_state_machine_sync(resume_fn, frame_ptr, frame_size) -> i32` (replaces the originally-planned bare `ynz_rt_block_on`). Body uses `Handle::block_on` EVERYWHERE inside a Tokio context (NO `block_in_place`):
   ```rust
   match tokio::runtime::Handle::try_current() {
       Ok(handle) => handle.block_on(StateFnFuture { ... }),  // works on BOTH worker AND blocking-pool threads
       Err(_)     => RUNTIME.get().expect("ynz_rt_init not called").block_on(StateFnFuture { ... }),
   }
   ```
   **WHY Shape B over Shape A**: Round 2 originally specified `tokio::task::block_in_place(|| handle.block_on(...))` on the `Ok` arm, which Round 3 flagged as catastrophically wrong — `block_in_place` PANICS when called from a `spawn_blocking`-pool thread. Three thread contexts exist (worker / spawn_blocking-pool / no-runtime), and `Handle::try_current()` returns `Ok` from BOTH worker and spawn_blocking-pool threads — the two-branch shim can't distinguish them and would panic in the (background regular fn → state-machine fn) transitive case.

   Shape B drops `block_in_place` entirely and uses bare `Handle::block_on` in the Tokio branch. `Handle::block_on` is documented to work on any thread (including worker AND blocking-pool threads). Tradeoff: when called from a worker thread, it ties up that worker for the wait duration (scheduler pressure: that worker can't service other tasks while the nested poll runs). Cost is real but bounded: (a) M2 ships the `wait_required_on_state_machine_call` warning that teaches users to write `wait` (which uses inline poll-and-yield, no shim); (b) M3's may-block analysis + auto-`wait` insertion eliminates most call sites for the shim entirely. The remaining call sites (main's entry wrap, edge cases) are infrequent enough that scheduler pressure is acceptable.

   Side-effect: the M2 `wait_required_on_state_machine_call` typeck warning becomes a TEACHING surface (guides users toward writing `wait` for perf reasons), not a CORRECTNESS gate (Shape B makes the no-wait case work correctly, just slower). Validated by P0 Spike Contracts #4a/4b/4c (one per thread context). Shape B chosen explicitly over Shape A; A would require a thread-local RAII marker installed in `ynz_rt_spawn_blocking` to distinguish worker vs blocking-pool — more moving parts, more places to leak the flag (RAII installation site must be correct on all panic paths). Shape B trades a known scheduler-pressure cost (measurable) for a smaller code surface (less to get wrong).

Open architectural questions inherited from roadmap — resolved in this milestone:

- **State machine ABI with Tokio's `Future`** — resolved in P0 spike. Locks the exact LLVM IR shape, frame allocation strategy, and waker registration protocol.

No open questions at execution-plan level at draft time.

---

## Risk Assessment & Rollout Strategy

**Risk level: HIGH** (mitigated to MEDIUM via P0 spike gate + cross-impl consistency harness + comprehensive test coverage + backward-compat bridge).

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | |
| Touches auth/permissions | No | |
| Raw SQL / literals | No | |
| Modifies existing data | No | Adds codegen path + runtime shims; existing fixtures still pass |
| Third-party integration | Yes | Deeper Tokio integration (now uses `task::spawn` for I/O pool, not just `spawn_blocking`; uses `time::sleep` and `Future::poll` API directly) |
| Changes existing endpoints | Yes | `Expr::Wait` semantic changes from identity-passthrough to actual suspension. Existing programs using `wait` get new behavior — but per the no-coloring design, the source-visible semantic is unchanged. |
| New feature with no existing equivalent | Partial | State-machine codegen is new; the `wait` keyword is not. |

**Mitigations applied**:
- P0 spike gate (5 contracts must hold before P1) → HIGH → MEDIUM
- Cross-impl consistency harness (M1) extended → MEDIUM → reinforced
- Test coverage (state-machine fixtures, errors+wait, block_on bridge, panic-through-state-machine, multi-wait sequential, wait-in-if) → MEDIUM → reinforced
- `--no-auto-parallel` kill switch carries forward (no-op in M2 but plumbing already exists) → MEDIUM → reinforced
- block_on bridge preserves correctness even when codegen makes suboptimal routing decisions → HIGH → MEDIUM

**Rollout plan**:
1. Internal testing: 2-3 days using `examples/pirates-roster/` + driver fixtures + new state-machine fixtures on CI + local
2. Tag `v0.3.0-m2` and publish VSCode extension
3. Full rollout: milestone tag IS the rollout. M3 lands on top.

---

## Invariants This Milestone Must Preserve

### Safety

- **`wait`-source-semantic preservation.** Every source program containing `wait expr` produces identical observable behavior under M2 codegen vs M1 codegen, modulo timing (M2 may interleave with other tasks during the wait, M1 holds the thread). Order of side effects in a single function body is preserved. Test: every fixture in `examples/` AND `crates/ynz-codegen/tests/fixtures/` that uses `wait` produces identical stdout/stderr/exit-code under M2 vs M1 build (verified by re-running M1 binaries against the M2 build of the same source — codegen-determinism property test).
- **State-machine frame ownership.** The frame allocated for a state machine is freed exactly once: on completion (happy path) OR on cancellation/drop (Tokio task dropped before completion) OR on panic during `poll()`. Verified by: P0 spike contract `state_machine_no_leak` — spawn 1000 state machines each with a 64-byte frame; cancel half mid-flight via `runtime.shutdown_timeout(0)`; assert net `ynz_alloc - ynz_free` count == 0.
- **`errors`-cascade through suspension.** A state-machine function returning `T errors` that produces an error after a `wait` correctly populates the error's `Frame` trace AND `SourceLoc` data. Test: `errors_cascade_through_state_machine` — call a fn that does `wait __testFallibleAsync()` with the failure flag; assert returned error has expected trace.
- **`block_on` bridge correctness.** A non-state-machine function calling a state-machine function gets identical return value as if the state machine had been driven by tokio::task::spawn + JoinHandle::await. Test: `block_on_bridge_returns_same_value_as_spawn`.
- **Use-after-free prevention on state-machine values.** A local in a state-machine frame that crosses a `wait` boundary survives the suspension (frame slot retains the value); a local declared and dropped before the `wait` is NOT in the frame. Test: `value_lives_across_wait_boundary` — IR snapshot asserts the frame contains exactly the right slots for the test fixture.
- **`--kernel` mode rejection** carries forward from M1 unchanged. Test: `kernel_mode_rejects_wait_unchanged_from_m1` passes — same diagnostic emitted as in M1.
- **Existing v0.1+v0.2+v0.3-M1 programs.** Every `.ynz` fixture not using `wait` produces byte-identical output under M2 build vs M1 build. Verified by: cross-impl consistency harness extended to compare M2-codegen-output vs the V1 baseline snapshot from M1.

### Performance

- **`lower_function_with_waits` path is only taken when needed.** Functions whose body contains no `Expr::Wait` use the standard `lower_function` path with zero added overhead. Verified by: `cargo bench`-style measurement (or `time cargo build -p ynz-driver`) on `examples/pirates-roster/` — typeck+codegen wall-clock M2 vs M1 baseline, threshold: ≤ 10% slowdown.
- **State machine frame size minimal.** Frame contains exactly: `resume_point: i32` + one slot per live local across a wait boundary + one slot for the awaited handle. Verified by: P0 spike measures frame sizes for representative fixtures; integration test `state_machine_frame_size_bounded` asserts frame ≤ 256 bytes for the spike fixture (5 sequential waits, 3 live locals).
- **Multi-task concurrency under wait.** 8 background tasks each calling `wait sleepAsync(100)` complete in ≤ 150ms wall-clock total on the CI runner (proves they share threads — if each held its own thread they'd complete in 100ms each but tied up 8 threads; the actual win is 8 tasks on 4 threads finish in ~100ms because they pause concurrently). Threshold tolerances are 1.5× the ideal 100ms to account for CI scheduler noise.
- **Sync-bridge overhead.** Calling a state-machine fn via `ynz_rt_call_state_machine_sync` adds ≤ 1% of a 100ms wait = ≤ 1000µs absolute overhead per call vs direct future polling. Single threshold per Round 3 reconciliation (Round 2 had 100µs + 1000µs + 5000µs all inconsistently — locked to 1000µs absolute / 1% relative). Verified by: microbenchmark in P0 spike Step 8.
- **`ynz_rt_check_preempt` per-call cost** unchanged from M1 (~1ns; no-op stub). Full preemption semantics still ship later — M2 doesn't touch the preempt mechanism.

**Auto-promotion analysis** (per `.claude/rules/auto-promotion.md`):

The M1 large-copy-warning hybrid (muted hint + Tier 3 lint for `.copy` → `.give`) carries forward unchanged. M2-specific auto-promotion candidates:

- **State machine vs straight-line codegen path selection** — the compiler auto-picks the state-machine path when `Expr::Wait` is detected. There's no user-typeable form for "force straight-line on this function" because forcing it would be incorrect (the `wait` semantics need the state machine). This is a codegen-only auto-decision; no muted hint, no Tier 3 lint surface. Documented as "always state machine when body contains wait; user has no override because the alternative is broken."
- **`background` routing (I/O pool vs blocking pool)** — auto-decided by the same "contains wait" syntactic check. There IS a user-typeable form for "force I/O pool" or "force blocking pool" candidate: `background.cpuBound fn()` / `background.ioBound fn()` per `design/future/concurrency.md`. But final naming is M3 territory (per the roadmap "Explicit override... final naming TBD"). M2 does NOT add user-typeable override forms — that would lock naming before M3's analysis informs it. Muted hint surface: deferred to M3 (`background_routing` muted-hint domain locks in M3 per roadmap). M2 routing decisions ARE made but are invisible at the user surface. Documented in registry as "M2 routing happens silently per syntactic-wait check; user-facing IDE hint ships in M3."
- **Other auto-promotion candidates in M2**: none. Runtime additions (`ynz_rt_spawn`, `ynz_rt_async_sleep_create`) have no stricter form the compiler could prove fits.

**M2 may-block predicate (precise spec — per plan-reviewer Required Fix #6)**:

The compiler decides whether a call site "may block" using the following predicate ONLY in M2:

```rust
// crates/ynz-typeck/src/intrinsics.rs
pub const M2_MAY_BLOCK_INTRINSICS: &[&str] = &["sleepAsync", "__testFallibleAsync"];

pub fn is_may_block_callee(callee_name: &str, callee_contains_wait: bool) -> bool {
    M2_MAY_BLOCK_INTRINSICS.contains(&callee_name) || callee_contains_wait
}
```

The predicate uses TWO signals AT THE SYNTACTIC CALL SITE:
1. **Callee is in the may-block intrinsic set** (`sleepAsync` or `__testFallibleAsync`)
2. **Callee is a user-defined function whose body syntactically contains `Expr::Wait`** (the `FunctionSig.contains_wait` flag populated by typeck — see Phase 3)

This is **NOT transitive.** If `foo()` calls `bar()` and `bar()` contains `wait`, then `bar` is may-block (sig.contains_wait=true) and a `wait bar()` call site is correctly handled. But if `foo()` calls `baz()` where `baz()` calls `sleepAsync(100)` (no wait), `baz` is NOT flagged as may-block in M2 (no contains_wait, not in intrinsic set). M3 ships the transitive predicate.

The `wait_on_non_may_block_warning` warning fires when `wait X()` is written but `is_may_block_callee(X, X.contains_wait)` returns false. Test fixture `transitive_no_wait_does_not_trigger_warning` validates the M2 predicate's intentional limitation (so M3 can swap the predicate without changing test expectations).

### Teaching

Every new diagnostic introduced in M2 follows WHAT/WHAT-INSTEAD/WHY. Audit performed by jargon test in `crates/ynz-diagnostics/tests/jargon_audit.rs` (existing infrastructure from M1).

New diagnostics in M2:

- `wait_on_non_may_block_warning` (P3, warning Tier 3): "`wait` on a function that does not suspend — the `wait` has no effect."
  - WHAT: "The function `{callee_name}` does not contain a suspension point; `wait` here changes nothing."
  - WHAT INSTEAD: "Remove the `wait` keyword — call `{callee_name}({args})` directly."
  - WHY: "`wait` only has effect when the awaited expression can suspend (calls a may-block intrinsic or another function whose body contains `wait`). Currently, `{callee_name}` is purely CPU-bound; the runtime semantics are identical with or without `wait`."
- `wait_on_non_call_expression` (P3, error): existing M1 check that `background` requires a call has a parallel — `wait` requires a call. M2 ships this as: if the inner of `wait` is anything other than a `Call`/`MethodCall`/`FreeFn-call`, emit an error.
  - WHAT: "`wait` must be followed by a function call."
  - WHAT INSTEAD: "Write `wait someFn()` to wait for `someFn` to complete."
  - WHY: "`wait` schedules a suspension point. It only applies to function calls whose result must be waited for."
- `unawaited_sleep_async` (P3, warning Tier 3): "calling `sleepAsync` without `wait` has no effect."
  - WHAT: "`sleepAsync(ms)` creates a sleep handle but discards it without waiting — the function returns immediately, the sleep never fires."
  - WHAT INSTEAD: "Write `wait sleepAsync({ms})` to actually pause for `{ms}` milliseconds. Or use `sleepMs({ms})` for the blocking-sleep semantics if `wait` isn't appropriate here."
  - WHY: "`sleepAsync` is a may-block intrinsic — calling it without `wait` constructs the sleep object then drops it. The `wait` keyword is the user-visible signal that a suspension point happens here."
- `wait_required_on_state_machine_call` (P3, **warning Tier 3** — reclassified Round 2 + locked Round 3 per Required Fix #5; runtime is panic-safe via `ynz_rt_call_state_machine_sync` Shape B, so this is a TEACHING surface that guides users toward writing `wait` for perf reasons, not a CORRECTNESS gate): emitted when a state-machine function (caller's `FunctionSig.contains_wait == true`) calls another state-machine function (callee's `FunctionSig.contains_wait == true`) WITHOUT wrapping the call in `wait` AND WITHOUT being the immediate inner of `Expr::Background`. Warning exit code = 0; stderr substring = `warning:`.
  - WHAT: "`{callee_name}` may suspend (its body contains `wait`); calling it from another state-machine function without `wait` is not allowed in v0.3-M2."
  - WHAT INSTEAD: "Write `wait {callee_name}({args})` to suspend the caller at this call site."
  - WHY: "v0.3-M2 keeps the no-coloring promise via a `block_on` bridge — but only for callers that aren't already state machines. State-machine-to-state-machine without `wait` would require auto-`wait` insertion that ships in v0.3-M3. Until then, the keyword must be explicit at these call sites only. M3 auto-insertion lifts this requirement with zero source change."

Updated diagnostics (hover docs):

- `wait` keyword: hover text fully replaces the M1 text. M2 now ships the real suspension semantics — the WHAT/WHAT-INSTEAD/WHY block in `### Feature Registry Entries` reflects that.
- `background` keyword: minor update — the routing-distinction note ("state-machine functions go to I/O pool; CPU-bound to blocking pool") is added to the WHY clause; "returns AND errors are discarded" clarification added (per Risks table mitigation for background-of-state-machine-fn returning T errors).

IDE muted-hint domains: `wait_points` and `background_routing` both still stay protocol-only (auto-insertion + cross-fn routing analysis are M3). M2 does NOT activate either domain. Documented in registry as "active in M3."

### Runtime Dependencies

- `lower_function_with_waits` codegen path requires `ynz_rt_spawn`, `ynz_rt_async_sleep_create`, `ynz_rt_async_sleep_poll`, `ynz_rt_call_state_machine_sync` C-ABI shims from `libynz_runtime.a`. All new in M2; live in `crates/ynz-runtime/src/runtime.rs`.
- Tokio runtime (already booted in `main` by M1's `ynz_rt_init`) must support `task::spawn` (I/O pool, multi-thread scheduler) AND `time::sleep` (timer wheel). Both features are in the existing Tokio dep (`rt-multi-thread` feature) — no Cargo.toml change required for M2 (verified: Tokio's `time` module is included in `rt-multi-thread` per Tokio 1.x feature dependencies).
- Heap allocation via `ynz_alloc` / `ynz_free` (existing M4 infrastructure) for state-machine frames. Requires malloc (libc) — no change from M1's runtime-dep story.

### Kernel-Mode Behavior

- `--kernel` mode rejection of `wait` and `background` carries forward from M1 unchanged. Same WHAT/WHAT-INSTEAD/WHY diagnostic. Same `check_with_kernel_mode` test path.
- New `sleepAsync(int)` intrinsic is also rejected in `--kernel` mode (no Tokio runtime in kernel mode). Test: `kernel_mode_rejects_sleep_async` — covers the new diagnostic path.
- All `wait`-free programs continue to work identically in `--kernel` mode (verified by extending the M1 kernel-mode tests to cover M2 build paths).

### Demo & Error Gallery

Per `.claude/rules/plan-invariants.md` `### Demo & Error Gallery`:

1. **`examples/pirates-roster/entrypoint.ynz`** — extended with a v0.3-M2 concurrency section that:
   - Spawns 8 background tasks, each calling `wait sleepAsync(100)` (8 per Round 2 Concern #5 — proves thread-sharing visibly on multi-core CI)
   - Prints a "scheduled 8 pirates" marker BEFORE the background tasks complete; then each task prints a marker after its wait
   - Demonstrates timing: all 8 background tasks complete in ~100ms wall-clock total, NOT ~800ms (proves they share threads via state-machine suspension)
   - Section header: `// ────── v0.3-M2: wait actually suspends ──────`
   - **Locked: demo uses `wait sleepAsync(100)` for the suspension-observable pause, NOT `sleepMs` or any other M1 intrinsic.** Rationale: `sleepMs` blocks the thread (M1 semantics); `wait sleepAsync` yields the thread (M2 semantics). Demonstrating the difference is the entire point of the section.

2. **`examples/primantis-orders/v0_3_m2_errors.ynz`** — new file, intentional triggers for every new M2 compile error/warning class:
   - `wait_on_non_may_block_warning` (a `wait print("hi")` — `print` is CPU-bound)
   - `wait_on_non_call_expression` (a `wait 42` — primitive value, not a call)
   - The existing M1 error gallery's triggers carry forward in `v0_3_m1_errors.ynz` (already shipped).
   - `// WHY:` comment on each trigger naming the diagnostic class.

3. **Verification**: Both files get `insta` snapshot tests (stdout + stderr) added in P4 and P5.

### Feature Registry Entries

Per `.claude/rules/plan-invariants.md` `### Feature Registry Entries`:

**SCHEMA**: M1 already extended `KeywordEntry` with `hover_what`/`hover_what_instead`/`hover_why` optional fields. M2 uses the existing schema — no new schema extension required.

Concrete entries this plan adds (modifies + new):

- **Modify `[[keyword]]` `wait`** (`registry/features.toml:166`): fully replace M1's forward-pointing hover text with M2's "now real" semantics.
  - WHAT: "Suspends the calling function until the awaited expression completes. The OS thread is freed for other tasks during the suspension."
  - WHAT INSTEAD: "Write `wait foo()` at a call site to suspend until `foo()` returns. Code after the `wait` runs only after `foo()` completes."
  - WHY: "`wait` makes the function pause without blocking the OS thread — the thread runs other ready tasks during the pause. If the caller of this function is not itself a state machine, the caller blocks until completion (this gap closes in v0.3-M3 when auto-insertion ships)."
- **Modify `[[keyword]]` `background`** (`registry/features.toml:170`): minor update — add routing-distinction note to WHY clause.
  - WHAT: "Runs the function on a separate thread."
  - WHAT INSTEAD: "Write `background fn(value.give)` or `background fn(value.copy)` to schedule `fn` to run independently."
  - WHY: "The background task runs concurrently with whatever follows the `background` call. In v0.3-M2, functions containing `wait` route to the I/O pool (cooperative scheduling); functions without `wait` route to the blocking pool (dedicated OS thread per task). Handle-form (`let h = background fn()`) ships in v0.3-M4."
- **New `[[diagnostic_template]]` `wait_on_non_may_block_warning`** — canonical text:
  - WHAT: "`wait` on a function that does not suspend — the `wait` has no effect."
  - WHAT INSTEAD: "Remove the `wait` keyword — call `{callee_name}({args})` directly."
  - WHY: "`wait` only has effect when the awaited expression can suspend (calls a may-block intrinsic or another function whose body contains `wait`). Currently, the callee is purely CPU-bound; the runtime semantics are identical with or without `wait`."
- **New `[[diagnostic_template]]` `wait_on_non_call_expression`** — canonical text:
  - WHAT: "`wait` must be followed by a function call."
  - WHAT INSTEAD: "Write `wait someFn()` to wait for `someFn` to complete."
  - WHY: "`wait` schedules a suspension point. It only applies to function calls whose result must be waited for."
- **New `[[diagnostic_template]]` `unawaited_sleep_async`** — canonical text:
  - WHAT: "`sleepAsync(ms)` creates a sleep handle but discards it without waiting — the function returns immediately, the sleep never fires."
  - WHAT INSTEAD: "Write `wait sleepAsync({ms})` to actually pause for `{ms}` milliseconds. Or use `sleepMs({ms})` for the blocking-sleep semantics if `wait` isn't appropriate here."
  - WHY: "`sleepAsync` is a may-block intrinsic — calling it without `wait` constructs the sleep object then drops it. The `wait` keyword is the user-visible signal that a suspension point happens here."
- **New `[[diagnostic_template]]` `wait_required_on_state_machine_call`** — canonical text:
  - WHAT: "`{callee_name}` may suspend (its body contains `wait`); calling it from another state-machine function without `wait` is not allowed in v0.3-M2."
  - WHAT INSTEAD: "Write `wait {callee_name}({args})` to suspend the caller at this call site."
  - WHY: "v0.3-M2 keeps the no-coloring promise via a `block_on` bridge — but only for callers that aren't already state machines. State-machine-to-state-machine without `wait` would require auto-`wait` insertion that ships in v0.3-M3. Until then, the keyword must be explicit at these call sites only. M3 auto-insertion lifts this requirement with zero source change."
- **New `[[primitive_intrinsic]]` `sleepAsync`** — free-fn intrinsic. Signature: `sleepAsync(int) -> nothing`. Kind = "free_fn". TOML schema:
  ```toml
  [[primitive_intrinsic]]
  name = "sleepAsync"
  kind = "free_fn"
  param_types = ["int"]
  return_type = "nothing"
  since = "v0.3-M2"
  ```
  - Summary: "Non-blocking sleep. `wait sleepAsync(ms)` suspends the calling function for `ms` milliseconds; the OS thread is freed during the suspension."
  - Pairs with `sleepMs(int)` (M1 intrinsic, `registry/features.toml:635` — blocking sleep, ties up the calling OS thread).
  - May-block: TRUE (member of `M2_MAY_BLOCK_INTRINSICS` set in `crates/ynz-typeck/src/intrinsics.rs`; M3 expands the predicate via call-graph analysis).
- **New `[[deferred_tooling_feature]]` `async-io-stdlib-intrinsics-v0-5`** — registry entry per `.claude/rules/feature-registry.md`'s SSOT discipline (per plan-reviewer Concern). Documents that real `readFileAsync` / `writeFileAsync` / `readNetworkAsync` / `dbQueryAsync` intrinsics are intentionally deferred to v0.5+ stdlib modules. Required fields:
  - WHY: "Each stdlib module (file v0.5, database v0.10, http v0.15) has its own API design questions (path encoding, error variants, connection pooling) that belong to the module's design milestone, not to the state-machine ABI milestone."
  - SUBSTITUTE: "Use `sleepAsync(int)` (v0.3-M2) to validate user code's async control flow. Real async I/O ships per-module."
  - SHIPS_IN: "v0.5+ (file, database, http modules each ship their own async surface)"
  - DESIGN_DOC: "design/stdlib/filesystem.md, design/stdlib/network.md, design/stdlib/database.md (each has a 'v0.5+ Async I/O Surface' subsection added in this milestone)"
- **NOT registered**: `__testFallibleAsync` (internal test-only intrinsic, never appears in registry). The intrinsic IS declared in `crates/ynz-typeck/src/intrinsics.rs` via the new `internal_fns` Vec (see Question Resolution #8). Visibility verified by P3 LSP completion test in `crates/ynz-lsp/tests/completion.rs`.
- **NOT changed**: `background-handle-form` deferred_tooling_feature (M1, unchanged); `wait_points` muted_hint_domain (still protocol-only, activates in M3); `background_routing` muted_hint_domain (M4 entry, not touched in M2).
- **NOT added**: `[[muted_hint_domain]]` new entries for M2 — both `wait_points` and `background_routing` remain protocol-only until M3 has the analysis to fire on.

---

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block that lists the actions to execute (persist plan state → invoke 4-agent review fan-out → handle verdict → prompt commit). Run those instructions at every phase boundary.

**Final phase additionally:**
- Verify ALL phases' acceptance-criteria and quality-gate checkboxes are accurate; update the overall Quality Checklist below
- Invoke 4-agent review fan-out with the **cumulative plan diff** (Step 10f): `git diff <plan-base-commit>..HEAD`
- Flip `status: active` → `status: done` in front-matter only after final PASS; the radar moves the file to `plans/done/` on next rebuild

---

## Phases

### Phase 0: State Machine ABI Spike

**PR scope**: A research spike that hand-writes the simplest state-machine cases in Rust (mimicking what the M2 codegen will emit), validates the Tokio Future polling protocol integration end-to-end, and locks the LLVM IR shape + frame layout before any codegen change lands. NO codegen changes yet. Output: validated ABI design + accept/reject gate + "Spike Findings" section appended to this plan file.
**Branch**: `spike/v0-3-m2-state-machine-abi`
**Flag**: N/A
**Est. lines**: ~400 (mostly Rust spike code + test harness; no production code)
**Ships via**: `/pr` (as a draft, kept-or-discarded based on accept/reject gate)
**Objective**: Validate five contracts before committing to the M2 codegen design: single-wait suspension+resume, multi-wait sequential, wait-inside-if branching, block_on bridge correctness, errors-cascade through state-machine boundary. If any fails or imposes unacceptable overhead, this phase produces a "design needs rework" finding that halts M2 and escalates to user — production code does NOT proceed to P1 until the spike's five contracts hold.
**Why this phase exists**: P1 onwards make irreversible design commitments (runtime ABI shape, codegen path structure). Discovering at P3 that the chosen state-machine layout fails to integrate with Tokio's `Future::poll` would force a full codegen rewrite. Failing fast here is the cheapest correction.
**Current-state anchors**:
- `crates/ynz-runtime/src/runtime.rs:110` — existing `ynz_rt_spawn_blocking` C-ABI pattern (model for the new `ynz_rt_spawn` shim)
- `crates/ynz-runtime/src/runtime.rs:184` — `ynz_rt_check_preempt` stub (no-op M1; documented to expand in M2 but defers to M3 per M1 P1 GATE)
- Tokio docs: `tokio::task::spawn`, `tokio::time::sleep`, `tokio::runtime::Runtime::block_on`, `std::future::Future`, `std::task::{Context, Poll, Waker}`
**Files (expected scope)**:
- `crates/ynz-runtime/tests/m2_spike.rs` — new test file with five spike contracts (parallel to M1's `tests/spike.rs`)
- `crates/ynz-runtime/Cargo.toml` — verify Tokio `time` feature is included; add if not
**Steps**:

**Step 1 — Single-wait suspension+resume (contract #1)**

Hand-write a Rust struct `FnFetchEvent` that mimics what the M2 codegen will emit for:
```ynz
function fetchEvent() -> nothing {
  wait sleepAsync(100)
  print(`done`)
}
```
The struct: `{ resume_point: i32, sleep_handle: Option<Sleep> }`. Impl `Future for FnFetchEvent` with state-machine `poll`: state 0 = create sleep, state 1 = poll sleep (Pending → register waker + return; Ready → state 2), state 2 = print + return Ready.

Drive it with `tokio::task::spawn(future)` + `JoinHandle::await`. Assert: completes in ~100ms wall-clock; no OS thread tied up during the pause (measured by checking that 8 simultaneous spawns complete in ~100ms total, not 800ms).

**Step 2 — Multi-wait sequential (contract #2)**

Hand-write the struct for:
```ynz
function chain() -> nothing {
  wait sleepAsync(50)
  print(`mid`)
  wait sleepAsync(50)
  print(`end`)
}
```
State machine: 5 states (initial, awaiting-first-sleep, post-first-print, awaiting-second-sleep, completed). Assert: prints `mid` before `end`, total wall-clock ~100ms.

**Step 3 — Wait inside if (contract #3)**

Hand-write the struct for:
```ynz
function maybeWait(b: bool) -> nothing {
  if (b) {
    wait sleepAsync(100)
  }
  print(`done`)
}
```
State machine has conditional resume — when `b=false`, skips directly from initial to done; when `b=true`, suspends. Assert: `b=true` takes ~100ms; `b=false` takes < 5ms.

**Step 4 — sync bridge correctness per thread context (contracts #4a/#4b/#4c — split per plan-reviewer Round 3 Required Fix #1)**

The `ynz_rt_call_state_machine_sync` shim must work in three distinct thread contexts. Each gets its own spike test; bundling them is what hid the Round 2 `block_in_place` panic-trap.

4a. **Worker-thread sync bridge** — from inside `runtime.spawn(async { ynz_rt_call_state_machine_sync(...) })`, the shim is invoked. The shim's `Handle::try_current()` returns Ok; the `Ok` arm runs `handle.block_on(StateFnFuture)`. Assert: completion in band `[95ms, 200ms]` for a 100ms inner wait; no panic; correct return value.

4b. **`spawn_blocking`-pool sync bridge** — from inside `runtime.spawn_blocking(|| ynz_rt_call_state_machine_sync(...))`, the shim is invoked. The shim's `Handle::try_current()` returns Ok (same as 4a — can't distinguish blocking-pool vs worker via Handle alone); the `Ok` arm runs `handle.block_on(StateFnFuture)`. **This is the contract that catches the Round 2 `block_in_place` bug.** Assert: completion in band `[95ms, 200ms]`; no panic; correct return value. If `block_in_place` were used here, this contract would panic with "can call block_in_place only from a runtime worker thread of a multi-thread runtime."

4c. **No-runtime sync bridge** (original Contract #4) — from main thread directly (before/after Tokio runtime is active OR detached thread), `ynz_rt_call_state_machine_sync(...)` is invoked. The shim's `Handle::try_current()` returns Err; the `Err` arm runs `RUNTIME.get().unwrap().block_on(StateFnFuture)`. Assert: completion in band `[95ms, 200ms]`; no panic; correct return value.

4d. **State-machine inside state-machine sync bridge** (Round 3 Required Fix #2) — outer state-machine A's `poll()` runs on a worker thread; A invokes `ynz_rt_call_state_machine_sync` driving inner state-machine B (which itself contains `wait sleepAsync(50)`). Validates that B's `poll()` (driven by A's `Handle::block_on`) yields and resumes correctly while A is parked waiting. Assert: A's `poll()` returns B's value; no deadlock; no panic; total wall-clock in band `[45ms, 150ms]` for B's 50ms wait. Verifies that Tokio's reactor still drives B's wakers even though A's worker is occupied by `block_on`.

**Step 5 — errors-cascade through state-machine boundary (contract #5)**

Hand-write the struct for:
```ynz
function fetchOrFail(shouldFail: bool) -> int errors {
  const x = wait __testFallibleAsync(shouldFail)  // returns int errors
  return x + 1
}
```
The awaited intrinsic returns `Poll<Result<i64, ErrorTag>>`. State machine threads the error frame through `Poll::Ready(Err(...))`. Assert: success path returns `42`; failure path returns an error with `Frame.line` populated AND `SourceLoc` struct fields readable post-resume.

**SCOPE CLARIFICATION (per plan-reviewer Required Fix #11)**: this spike validates the SourceLoc STRUCT SHAPE and writability — that the struct's bytes survive across the `Pending → Ready(Err)` boundary intact. The fixture is hand-written Rust mimicking what codegen will emit, so `SourceLoc.file` is set to a Rust literal (e.g., `"__spike_fixture__"`) — NOT an end-to-end Yinz-source-span propagation test. End-to-end Yinz span propagation is validated separately by P5's `errors_cascade_through_state_machine` integration test (which compiles real `.ynz` source and checks the trace points to the actual `.ynz` file line).

**Step 6 — Frame size measurement**

For each spike struct, measure `std::mem::size_of::<FnXxx>()` and document. Threshold: frame ≤ 256 bytes for all spike fixtures (Single-wait, Multi-wait sequential, Wait-in-if, Heap-string-survives — see step 7d).

**Step 7 — Adversarial contracts (per plan-reviewer "Suggested Adversarial Cases")**

Spike must validate these additional contracts to clear the Tier A correctness bar:

7a. **Frame ownership / no-leak (contract #6, existing)** — spawn 1000 instances of `FnFetchEvent` with heap-allocated frame (via `Box<FnFetchEvent>` to mimic `ynz_alloc`). Drop half mid-flight via `runtime.shutdown_timeout(0)`. Assert: instrument a global atomic counter on alloc/dealloc; net count == 0 at end.

7b. **Wait inside loop body (contract #7, NEW per round-1 adversarial case 1)** — hand-write a state machine for:
```ynz
function pulse() -> nothing {
  for (i in range(0, 10)) { wait sleepAsync(10) }
}
```
The state machine has a per-iteration resume-point (NOT 10 distinct states; loop body shares a state). Frame slot for `i` is reused across iterations. Assert: completes in ~100ms wall-clock; frame size ≤ 64 bytes (proves slot reuse, not per-iteration growth).

7c. **Wait inside if condition (contract #8, NEW per round-1 adversarial case 2)** — hand-write for:
```ynz
function branch() -> nothing {
  if (wait fetchBool()) { print(`yes`) } else { print(`no`) }
}
```
where `fetchBool() -> bool errors` is a hand-written synthetic. Assert: the boolean used in the branch is the POST-suspension value (not a snapshot taken before the wait). Validates that locals don't leak forward from pre-wait state across the boundary.

7d. **Heap-string survives suspension (contract #9, NEW per round-1 adversarial case 7)** — hand-write for:
```ynz
function chat(greeting: string) -> nothing {
  print(greeting)
  wait sleepAsync(50)
  print(greeting + ` again`)
}
```
The local `greeting` (16-byte SSO+heap struct per M7 ABI) crosses the wait boundary. State machine frame allocates a slot sized exactly to the type's stored layout (NOT just i64). Assert: post-resume, `greeting`'s length, byte content, and SSO/heap discriminant are all intact. **This locks the per-type slot-sizing strategy in the ABI** — frame layout is "per live local across a wait, slot sized to type's stored layout"; if reviewer rejects this strategy, ABI changes before P2.

7e. **Recursive state-machine fn (contract #10, NEW per round-1 adversarial case 3)** — hand-write for:
```ynz
function fibA(n: int) -> int errors {
  if (n < 2) return n
  const a = wait fibA(n - 1)
  const b = wait fibA(n - 2)
  return a + b
}
```
Each call gets its own heap-allocated frame. Spawn `fibA(8)` (modest recursion depth — 67 calls), assert: returns 21, no shared-state corruption, no double-free at scope exit.

7f. **Waker propagation correctness (contract #11, NEW per plan-reviewer Round 3 Required Fix #3)** — spawn a state machine via `tokio::task::spawn` whose first wait is on a `Sleep`. Construct the test such that ONLY the waker mechanism can wake the task (DO NOT poll in a tight loop). Assert: the task completes ~100ms later, not hung. If hung, the waker re-registration is broken.

**ABI spec for `waker_ctx` (LOCKED in spike)**: `waker_ctx: *mut u8` is a type-erased pointer to `std::task::Context<'_>`. The codegen-emitted resume function casts it back via `unsafe { &mut *(waker_ctx as *mut Context<'_>) }` and passes that exact Context (no fabricated Wakers) into `Future::poll` for the awaited sleep handle's poll. P1 Step 3's `ynz_rt_async_sleep_poll(handle_ptr, waker_ctx)` C-ABI receives the same waker_ctx and forwards it to `Sleep::poll`. **Forbidden**: fabricating a `Waker::noop()` or any synthetic Waker inside the resume function — would silently hang any task awaiting under a quiet runtime.

7g. **Frame Drop impl + non-trivial local cleanup (contract #6 expanded per Round 3 Required Fix #6)** — extend Contract #6 (`state_machine_no_leak`) to cover non-trivial locals.

Frame layout decision (locked in spike): the generated state-machine struct implements `Drop`. Drop body:
- For each frame slot that holds a heap-owning Yinz value (string with heap, array, map): emit the corresponding `ynz_*_free` destructor call IF the slot is "live" (live tracked by a per-slot bit in a small `live_mask: u64` field on the frame — initial value 0; set to 1 when slot becomes live; unset to 0 when slot's value is consumed/moved out).
- For the `awaited_handle` slot: if non-null, free via `ynz_rt_async_sleep_free` (or the per-type free shim).
- Frame storage itself: freed by `Box<StateFn>` drop (the frame is owned by the Future struct).

Spike test: spawn 1000 instances of a state machine whose body declares `const greeting = ...` (heap string) BEFORE a `wait sleepAsync(50)`. Cancel half mid-wait via `runtime.shutdown_timeout(Duration::ZERO)`. Assert: instrument `ynz_alloc/ynz_free` AND `ynz_string_free` counters; net counts return to baseline for BOTH the frame heap AND the string heap. Catches the "frame freed but string heap leaked" silent-bug class.

**Step 8 — sync-bridge overhead measurement (relative threshold per plan-reviewer Round 1 Concern + Round 3 reconciliation)**

Compare: (a) `ynz_rt_call_state_machine_sync(...)` vs (b) calling `future.poll(&mut cx)` synchronously in a tight loop. Single threshold (per Round 3 Concern — Round 2 had reconciliation conflict between 100µs / 1000µs / 5000µs): **relative threshold ≤ 1% of a single 100ms wait = ≤ 1000µs absolute**. No separate "5000µs hard fail" — Round 2's two-threshold setup was inconsistent. Rationale: absolute thresholds vary by CPU; relative is portable. 1% of 100ms is the right precision for a 100ms-scale operation.

**Step 8b — codegen-invariant proof (contract #12 — Round 3 Required Fix #4)**

Validates that `ynz_rt_init` is the FIRST IR instruction in `main`'s entry block whenever any function in the compilation unit contains `wait` or `background`. IR snapshot test: compile a `.ynz` fixture containing both, inspect the LLVM IR of the generated `main`, assert the first non-allocation instruction is `call void @ynz_rt_init()`. This validates Question Resolution #9's invariant claim that makes the `RUNTIME.get().expect("ynz_rt_init not called")` panic unreachable in correct codegen.

**Step 9 — Write "Spike Findings"**

Append a `## Spike Findings` section to THIS plan file documenting:
- Contract results table (✅/❌ per contract, all 10)
- Frame sizes for each fixture
- `block_on` overhead measurement (absolute + relative)
- Locked ABI design: struct layout, per-type slot sizing strategy, `poll()` signature, waker registration pattern
- Accept/reject decision

If ANY contract fails: halt + escalate to user with the failure detail + proposed plan modification.

**Acceptance criteria**:
- [x] Contract #1 (single-wait): spike test passes; 8 simultaneous spawns complete in band `[80ms, 150ms]` wall-clock (lower bound catches broken sleep; upper bound catches sequential execution)
  - Evidence: `contract_1_single_wait_suspension_resume` — elapsed=101.4ms in band [80ms,150ms] ✓
- [x] Contract #2 (multi-wait sequential): prints `mid` before `end`; total wall-clock in band `[95ms, 150ms]` (50ms+50ms ideal)
  - Evidence: `contract_2_multi_wait_sequential` — elapsed=102.5ms; mid timestamp < end timestamp ✓
- [x] Contract #3 (wait-in-if): conditional resume works; b=true → `[95ms, 150ms]`, b=false → < 5ms
  - Evidence: `contract_3_wait_inside_if` — b=true=101.7ms, b=false=8.6µs ✓
- [x] Contract #4a (worker-thread sync bridge): spawned async task invokes `ynz_rt_call_state_machine_sync`; completes in band `[95ms, 200ms]`; no panic
  - Evidence: `contract_4a_worker_thread_sync_bridge` — elapsed=100.6ms; no panic ✓
- [x] Contract #4b (spawn_blocking-pool sync bridge): spawn_blocking task invokes shim; completes in band `[95ms, 200ms]`; no panic (catches the Round 2 block_in_place bug)
  - Evidence: `contract_4b_spawn_blocking_pool_sync_bridge` — elapsed=100.5ms; no panic; `Handle::block_on` (not `block_in_place`) confirmed safe ✓
- [x] Contract #4c (no-runtime sync bridge): main thread directly invokes shim; completes in band `[95ms, 200ms]`; no panic
  - Evidence: `contract_4c_no_runtime_sync_bridge` — elapsed=101.3ms; Err arm drove local single-thread rt ✓
- [x] Contract #4d (state-machine-inside-state-machine sync bridge): outer SM A's poll() invokes shim driving inner SM B; B's `wait sleepAsync(50)` yields + resumes; A's poll() returns B's value; total `[45ms, 150ms]`
  - Evidence: `contract_4d_state_machine_inside_state_machine_sync_bridge` — elapsed=51.5ms; result=42; no deadlock ✓
- [x] Contract #5 (errors-cascade): success path returns `42`; failure path returns error with `Frame.line` and `SourceLoc` fields readable post-resume; SourceLoc.file set to spike-fixture literal `"__spike_fixture__"` (struct-shape validation, not Yinz-span propagation)
  - Evidence: `contract_5_errors_cascade_through_sm_boundary` — success val=42; failure file=`__spike_fixture__` line=42 col=7 ✓
- [x] Contract #6 (frame ownership): 1000 spawn-then-cancel cycle has net `alloc - dealloc` == 0
  - Evidence: `contract_6_frame_ownership_no_leak` — alloc=1000 free=1000 net=0 ✓
- [x] Contract #7 (wait-in-loop): 10-iteration loop completes in band `[95ms, 150ms]`; frame size ≤ 64 bytes (slot reuse, not per-iteration growth)
  - Evidence: `contract_7_wait_in_loop` — elapsed=112ms; FnPulse=16 bytes ✓
- [x] Contract #8 (wait-in-if-condition): branch decision uses POST-suspension value, not pre-suspension snapshot
  - Evidence: `contract_8_wait_in_if_condition` — true→true, false→false post-suspension; no pre-suspension snapshot leak ✓
- [x] Contract #9 (heap-string-survives): per-type slot sizing works; greeting string's length, bytes, SSO discriminant intact post-resume
  - Evidence: `contract_9_heap_string_survives_suspension` — SSO len=5 bytes intact; heap len=28 is_heap=true intact ✓
- [x] Contract #10 (recursive state machine): `fibA(8)` returns 21; 67 frames allocated + freed without shared-state corruption or double-free
  - Evidence: `contract_10_recursive_state_machine` — fib(8)=21; alloc=67 free=67 net=0 ✓
- [x] Contract #11 (waker propagation): spawned state machine awaits real `Sleep` under quiet runtime; completes via waker mechanism (NOT tight-loop polling); waker_ctx ABI uses `*mut Context<'_>` cast to `*mut u8`, no fabricated wakers
  - Evidence: `contract_11_waker_propagation` — elapsed=101.4ms; poll_count=2 (initial + waker-triggered) ✓
- [x] Contract #12 (codegen invariant): IR snapshot of `main` containing `wait` shows first non-allocation instruction is `call void @ynz_rt_init()`
  - Evidence: DEFERRED to P2 — accepted by Patrick at the P0 gate (2026-05-30). NOT proven at P0: there is no M2 `wait` codegen at the spike layer to snapshot. The invariant is now a HARD GATE in Phase 2 via the dedicated AC `main_rt_init_is_first_instruction` IR-snapshot test (added at the P0 gate per acceptance-verifier recommendation). `contract_12_codegen_invariant_documented` records the deferral rationale; full rationale in `## Spike Findings`. Deferral is tracked, not lost.
- [x] Frame size measurement: all spike fixtures ≤ 256 bytes
  - Evidence: `frame_size_measurement_all_fixtures` — max=64 bytes (FnChat, FibFuture); all ≤ 256 bytes ✓
- [x] Sync-bridge overhead: < 1% of a 100ms wait (≤ 1000µs absolute) — single threshold per Round 3 reconciliation
  - Evidence: `sync_bridge_overhead_measurement` — overhead=196µs vs threshold=1000µs ✓
- [x] Frame Drop impl + non-trivial local cleanup (Contract #6 expanded per Round 3 Required Fix #6): 1000 state machines with `string` locals, half cancelled mid-wait; both frame alloc/free counter AND `ynz_string_free` counter return to baseline
  - Evidence: `contract_6_expanded_drop_impl_non_trivial_cleanup` — frame alloc=1000 free=1000 net=0; string_frees=1000 ✓
- [x] "Spike Findings" section written into this plan file with locked ABI design (struct layout + per-type slot sizing strategy)
  - Evidence: `## Spike Findings` section below ✓
- [x] `cargo build --workspace` succeeds
  - Evidence: `cargo build -p ynz-runtime` passes clean; full workspace build passes ✓
- [x] `cargo test --workspace` passes (no regression — new spike tests added)
  - Evidence: 19/19 spike tests pass. The 5 ynz-driver snapshot "failures" are a WORKTREE-PATH artifact — the snapshots bake the absolute fixture path (`/workspaces/ynz/crates/...`) which differs from the worktree path; coordinator confirmed they fail identically at pristine base `d509770` with P0's change stashed, so they are NOT P0 regressions (and are unrelated to ynz-runtime). Pre-existing test-harness non-portability, tracked as a separate cleanup. ✓

**Quality gate**:
- [x] No `unsafe` outside the test-fixture extern fn signatures themselves; each unsafe block has a SAFETY comment
  - All unsafe blocks in `m2_spike.rs` have `// SAFETY:` comments; unsafe only in `sync_bridge_resume` and `ynz_rt_call_state_machine_sync_spike`
- [x] Spike struct's `Future` impl correctly handles `Pin` semantics (no manual `Pin` projection violations)
  - All futures use `Pin::new(self.0.as_mut()).poll(cx)` delegation; no manual pin projection
- [x] Waker registration uses `cx.waker().clone()` per Tokio's documented protocol
  - Sleep futures delegate to Tokio's real waker via forwarded `cx`; no fabricated wakers; poll_count=2 proves single waker wake
- [x] Frame alloc uses `Box::into_raw + Box::from_raw` RAII pattern (matches M1's `CtxDropGuard`)
  - Frames allocated as `Box::new(Self {...})` and owned by wrapper structs; Drop impl frees via RAII; no manual `into_raw/from_raw` needed at spike layer (wrapper owns the Box)
- [x] Panic during `poll()` is caught by Tokio's task wrapper (validated by an explicit panic test similar to M1's `spawn+panic`)
  - `quality_gate_panic_during_poll_caught` — JoinError::is_panic() confirmed; main continues ✓
- [x] No SQL/security concerns (pure-Rust spike; no external input)

**Verification**:
- `cargo test -p ynz-runtime --test m2_spike -- --nocapture` shows all five contracts pass + measurements within budget
- `cargo test --workspace` shows no regression

**Phase Review Gates** (filled at phase completion):
- [x] code-reviewer: PASS 2026-05-30T21:40 (re-review confirmed the string_frees fix is provably non-flaky; verified the block_on-panics ABI claim independently)
- [x] rules-compliance-reviewer: PASS 2026-05-30T21:25 (no violations; Demo/Registry exemption correct for a no-Yinz-surface spike)
- [x] plan-adherence-verifier: PASS 2026-05-30T21:35 (re-run vs canonical worktree plan; all 9 steps MET/documented-deviation; 4a/4b deviation sound)
- [x] acceptance-verifier: PASS 2026-05-30T21:42 (all ACs MET; Contract #12 accepted as documented deferral-to-P2 by Patrick at the P0 gate)
- [x] Committed: 6328666f09a0b9f993b1a1b13da878e30ff2c662

**Findings Log** (filled during any fix loops):
- 2026-05-30T21:30 — acceptance-verifier round 1: BLOCK. 2 WEAK ACs. (1) Contract #6-expanded asserts `string_frees > 0` but AC requires the counter "return to baseline" (deterministically == 1000); `> 0` passes even on a 999/1000 leak. Also flagged by code-reviewer (Concern #1). `m2_spike.rs:~1640`. (2) Contract #12 IR-snapshot test passes unconditionally — a documented deferral to P2, not a P0 proof. `m2_spike.rs:1558-1588`.
- 2026-05-30T21:30 — code-reviewer round 1: PASS with concerns. Confirmed #6-expanded looseness independently (count is deterministic, not scheduler-dependent — the comment claiming otherwise is wrong). Cosmetic clippy `let_and_return` at `m2_spike.rs:~681`. Verified the block_on-panics claim via its own probe — holds.
- 2026-05-30T21:30 — rules-compliance-reviewer round 1: PASS. No violations.
- 2026-05-30T21:30 — plan-adherence-verifier round 1: PASS (re-run against canonical worktree plan copy; first run BLOCKed on a stale-main-tree-copy false-positive — coordinator synced copies). 4a/4b spawn_blocking collapse = acceptable documented deviation, no plan amendment needed.
- 2026-05-30T21:35 — fix-loop round 1 (coordinator → executor): tighten `string_frees` assertion to `assert_eq!(string_frees, 1000)` + fix the incorrect "scheduler-dependent" comment + clippy `let_and_return`. Contract #12 deferral routed to Patrick at P0 gate (AC unsatisfiable at P0 by construction).

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick `Acceptance criteria` checkboxes + `Quality gate` items. Write `## Spike Findings` section. Bump `last_updated:` to today.
2. **Invoke 4-agent review fan-out** in parallel (code-reviewer, rules-compliance-reviewer, plan-adherence-verifier, acceptance-verifier). Brief: "Audit P0 spike against the 5 contracts and 3 measurements. Critical: validate frame ownership / panic-path / waker-protocol correctness."
3. **Handle the verdict.** BLOCK → fix or push back with evidence (max 3 rounds). PASS → continue.
4. **GATE**: if Spike Findings say any contract failed → halt + escalate to user.
5. Prompt user: "P0 spike done. Findings: [accept/reject summary]. Ready to commit and start P1 (runtime layer)?"

---

## Spike Findings

**Decision: ACCEPT** — all 12 contracts + 3 measurements pass. P1 can proceed.

### Contract Results

| Contract | Description | Result | Measurement |
|---|---|---|---|
| #1 | Single-wait suspension+resume | ✅ | 8 spawns: 101ms in [80ms,150ms] |
| #2 | Multi-wait sequential | ✅ | 102ms; mid < end ✓ |
| #3 | Wait-inside-if | ✅ | b=true: 101ms; b=false: 8µs |
| #4a | Worker-thread sync bridge | ✅ | 100ms; no panic |
| #4b | spawn_blocking-pool sync bridge | ✅ | 100ms; no panic; block_in_place bug CONFIRMED CAUGHT |
| #4c | No-runtime sync bridge | ✅ | 101ms; Err arm drove local runtime |
| #4d | SM-inside-SM sync bridge | ✅ | 51ms; result=42; no deadlock |
| #5 | errors-cascade | ✅ | success=42; failure=file `__spike_fixture__` line=42 col=7 |
| #6 | Frame ownership no-leak | ✅ | alloc=1000 free=1000 net=0 |
| #7 | Wait-in-loop | ✅ | 112ms; FnPulse=16 bytes (slot reuse proven) |
| #8 | Wait-in-if-condition | ✅ | POST-suspension value used; no pre-wait snapshot leak |
| #9 | Heap-string survives suspension | ✅ | SSO+heap intact; per-type slot sizing confirmed |
| #10 | Recursive state machine | ✅ | fib(8)=21; 67 alloc/free net=0 |
| #11 | Waker propagation | ✅ | poll_count=2; real Sleep waker; no tight-loop |
| #12 | Codegen invariant | ✅ (deferred) | No M2 codegen exists yet at P0; deferred to P2 IR snapshot test |
| Frame sizes | All fixtures ≤ 256 bytes | ✅ | Max=64 bytes (FnChat, FibFuture) |
| Sync overhead | ≤ 1000µs (1% of 100ms) | ✅ | 196µs overhead (80% under budget) |
| #6 expanded | Drop + string cleanup | ✅ | frame net=0; string_frees=1000 |

### Frame Sizes (debug build)

| Fixture | Maps to Yinz function | Size (bytes) |
|---|---|---|
| FnFetchEvent | `function fetchEvent() -> nothing` | 16 |
| FnChain | `function chain() -> nothing` | 16 |
| FnMaybeWait | `function maybeWait(b: bool) -> nothing` | 16 |
| SyncBridgeTarget | generic 1-wait fixture | 24 |
| FnFetchOrFail | `function fetchOrFail(bool) -> int errors` | 56 |
| FnPulse | `function pulse() -> nothing` (10-iter loop) | 16 |
| FnBranch | `function branch() -> nothing` (if-wait) | 24 |
| FnChat | `function chat(greeting: string) -> nothing` | 64 |
| WakerProbeTask | waker-protocol fixture | 24 |
| FibFuture | `function fibA(n: int) -> int errors` (recursive) | 64 |

All frames well within the 256-byte budget; largest is 64 bytes (FnChat with heap-string slot, FibFuture with two sub-future slots).

### Sync-Bridge Overhead

```
baseline: 101ms  (direct future polling via rt.block_on)
bridge:   101ms  (via ynz_rt_call_state_machine_sync_spike)
overhead: ~196µs absolute (~0.19% of 100ms)
threshold: 1000µs (1% of 100ms)
result: PASS — 5× under budget
```

### Locked ABI Design

#### State-Machine Struct Layout

```rust
struct FnXxx {
    resume_point: i32,          // which state to resume into
    <per-live-local-field>,     // one field per live local crossing a wait boundary
    sleep_handle: Option<Pin<Box<Sleep>>>,  // the awaited handle slot (None when not waiting)
}
```

#### Per-Type Slot-Sizing Strategy (LOCKED)

Each live local crossing a wait boundary gets a slot sized to the type's stored layout, NOT just `i64`. Confirmed by contract #9:
- `bool` → 1 byte field (padded to alignment)
- `i32` / `i64` → 4/8 bytes
- `string` (SSO+heap struct, 16 bytes per M7 ABI) → 16-byte field in frame
- `array<T>`, `map<K,V>` → pointer+length = 16 bytes
- `Option<Pin<Box<Sleep>>>` → sized to its layout (pointer-sized Option with heap future)

#### `poll()` Signature

```rust
fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Output>
```

State machine loops through `match self.resume_point { ... }` states. On each `wait` site:
1. Creates the awaited future (or uses existing slot)
2. Calls `future.as_mut().poll(cx)` forwarding the real `cx`
3. On `Pending`: returns `Poll::Pending` (waker already registered by the sub-future's poll)
4. On `Ready(val)`: stores val in frame slot, transitions `resume_point` to next state

#### Waker Registration Protocol (LOCKED)

`cx` from `Future::poll` is forwarded directly to sub-future polls. The sub-future (e.g., `tokio::time::Sleep`) registers its own waker with the Tokio reactor when polled with `Poll::Pending`. No fabricated wakers. No `Waker::noop()`. Proven by contract #11: poll_count=2 (initial + waker-triggered), no tight-loop.

**`waker_ctx` C-ABI**: `*mut u8` pointer to `&mut Context<'_>`. The codegen-emitted resume function casts back via `unsafe { &mut *(waker_ctx as *mut Context<'_>) }`. This is the Shape B ABI for `ynz_rt_call_state_machine_sync`. The `StateFnFuture` wrapper in the spike demonstrates the pattern.

#### `ynz_rt_call_state_machine_sync` Shape B (LOCKED)

```rust
match tokio::runtime::Handle::try_current() {
    Ok(handle) => handle.block_on(StateFnFuture { ... }),  // worker + spawn_blocking
    Err(_)     => RUNTIME.get().expect("...").block_on(StateFnFuture { ... }),
}
```

**Why NOT `block_in_place`**: contract #4b proves `block_in_place` would panic on spawn_blocking-pool threads even though `Handle::try_current()` returns `Ok` there. `Handle::block_on` is documented to work on any thread context. Cost: scheduler pressure on worker threads (ties worker for wait duration). Acceptable because M3 auto-`wait` insertion eliminates most sync bridge call sites.

**Why NOT nested async poll**: contract #4a/4d demonstrate that `handle.block_on()` panics when called from within an async future's `poll()` (Tokio forbids nested blocking in async context). The sync bridge is ONLY called from sync (non-async) code — C-ABI callbacks, spawn_blocking closures, `main` entry point. This is structurally enforced by codegen.

#### Frame Allocation / Drop Protocol (LOCKED)

- **Allocation**: `Box::new(StateFn { ... })` at task-spawn time; Box owns the frame.
- **Freeing**: Rust's RAII Drop on the Box when the task completes or is aborted. For futures with non-trivial locals (heap strings, arrays, maps), the Drop impl calls the Yinz destructor (`ynz_string_free`, etc.) for each live slot — tracked via `Option<T>` fields (Some = live, None = consumed/dropped).
- **Cancellation safety**: Tokio task abort triggers Drop on the future struct, which runs the frame's Drop impl. Contract #6 proves no leak on 500 mid-flight cancellations.
- **String cleanup**: Contract #6-expanded proves heap string locals are freed via Drop even when the frame is cancelled mid-wait.

#### Contract #12 Deferral Rationale (codegen invariant)

Contract #12 requires compiling a `.ynz` fixture containing `wait` to LLVM IR and asserting `call void @ynz_rt_init()` is the first non-alloca instruction in `main`. This cannot be validated at the P0 spike layer because:

1. M2 codegen for `wait` does not exist — P0 is the gate BEFORE P2 codegen lands.
2. The existing `emit.rs:3139` lowering is identity pass-through; IR would not show state-machine structure.
3. The spike validates the runtime ABI design (the part that exists now), not future codegen.

**Where this invariant gets validated**: Phase 2 (codegen layer) Step 5: `main_rt_init_is_first_instruction` IR snapshot test. The P2 acceptance criteria include this test; it is the load-bearing check for the ynz_rt_init ordering guarantee.

---

### Phase 1: Runtime Layer — `ynz_rt_spawn` + `sleepAsync` Future Plumbing

**PR scope**: Promote the spike's runtime primitives into production `ynz-runtime` API. Add `ynz_rt_spawn` (I/O pool, non-blocking — uses `tokio::task::spawn`), `ynz_rt_async_sleep_create` + `ynz_rt_async_sleep_poll` (the C-ABI shims that back the `sleepAsync` Yinz intrinsic), and `ynz_rt_call_state_machine_sync` (the runtime-aware synchronous bridge — uses **Shape B**: `Handle::block_on` everywhere inside Tokio, fallback to `RUNTIME.block_on` outside, NO `block_in_place`, per plan-reviewer Round 3 Required Fix #1). Register the new C-ABI functions in `ynz-codegen`'s `runtime_decls.rs` (declarations only — no call sites yet; those land in P2).
**Branch**: `feat/v0-3-m2-runtime-layer`
**Flag**: N/A
**Est. lines**: ~250
**Ships via**: `/pr`
**Objective**: Ship the runtime API as merged code so P2 codegen can build against it. After this PR, `cargo build --workspace` builds `libynz_runtime.a` with the new shims linked; `ynz build hello.ynz` still works (shims declared in codegen but not yet emitted).
**Why this phase exists**: Separating the runtime API ship from the codegen-emits-the-calls ship gives a clean checkpoint. If P2 codegen has issues, the runtime layer is already merged and reviewable independently.
**Current-state anchors**:
- `crates/ynz-runtime/src/runtime.rs:1-200` — existing M1 shims (model for the M2 additions)
- `crates/ynz-codegen/src/runtime_decls.rs` — pattern for declaring runtime fns (M1 added 5; M2 adds 4)
**Files (expected scope)**:
- `crates/ynz-runtime/src/runtime.rs` — add `ynz_rt_spawn`, `ynz_rt_async_sleep_create`, `ynz_rt_async_sleep_poll`, `ynz_rt_call_state_machine_sync`
- `crates/ynz-runtime/src/lib.rs` — re-export the new symbols
- `crates/ynz-runtime/Cargo.toml` — verify `tokio` features include `time` and `rt-multi-thread`; add if missing
- `crates/ynz-runtime/tests/m2_runtime.rs` — promote spike tests into production runtime tests
- `crates/ynz-codegen/src/runtime_decls.rs` — declare the 4 new C-ABI shims with correct LLVM types
**Steps**:
1. **`ynz_rt_spawn(resume_fn, frame_ptr, frame_size)`** — promote the spike's I/O-pool spawn into production. Signature: `extern "C" fn ynz_rt_spawn(resume_fn: extern "C" fn(*mut u8, *mut u8) -> i32, frame_ptr: *mut u8, frame_size: i64)`. The `resume_fn` is the state-machine's resume function (called once per poll); the second `*mut u8` arg is the Tokio waker context (opaque to Yinz). Return value: `0` = Ready, `1` = Pending. Internally: wrap into a Future struct, call `runtime.spawn(future)`. Panic-catch wrapper matches M1's `ynz_rt_spawn_blocking` pattern. RAII frame cleanup via `CtxDropGuard` (rename to `FrameDropGuard` for clarity).
2. **`ynz_rt_async_sleep_create(ms) -> *mut u8`** — creates a `tokio::time::Sleep` future, boxes it into a heap allocation, returns the pointer. The pointer is opaque to the Yinz code; only the codegen's state-machine resume function calls `ynz_rt_async_sleep_poll` on it.
3. **`ynz_rt_async_sleep_poll(handle_ptr, waker_ctx) -> i32`** — polls the boxed Sleep future via `Future::poll`. Returns `0` (Ready — caller can drop the handle) or `1` (Pending — caller saves the handle for later poll). Re-registers the waker with the runtime on Pending. Panic-catch wrapper.
4. **`ynz_rt_call_state_machine_sync(resume_fn, frame_ptr, frame_size) -> i32`** (Shape B per plan-reviewer Round 3 Required Fix #1) — synchronously drives a state-machine to completion, thread-context-correct. Body:
   ```rust
   match tokio::runtime::Handle::try_current() {
       Ok(handle) => {
           // Inside Tokio (worker thread OR spawn_blocking-pool thread).
           // Handle::block_on works on both contexts. Worker case: ties up this worker
           // for the wait duration (scheduler pressure). Blocking-pool case: fine, that's what the pool is for.
           handle.block_on(StateFnFuture { resume_fn, frame_ptr, frame_size })
       }
       Err(_) => {
           // Outside Tokio (typically: main thread before/after the runtime is active,
           // OR a fully-detached thread). Use the global RUNTIME.
           let guard = RUNTIME.get().expect("ynz_rt_init not called before sync state-machine call");
           let lock = guard.lock().unwrap_or_else(|e| e.into_inner());
           let rt = lock.as_ref().expect("ynz_rt_shutdown already called");
           rt.block_on(StateFnFuture { resume_fn, frame_ptr, frame_size })
       }
   }
   ```
   Panic-catch wrapper around the entire `match`. Returns the state machine's final value.

   **WHY Shape B (Handle::block_on, no block_in_place) — Round 3 Required Fix #1**: Round 2 originally specified `tokio::task::block_in_place(|| handle.block_on(...))` on the `Ok` arm. Round 3 caught that `block_in_place` PANICS when called from a `spawn_blocking`-pool thread — exact case the (background regular_fn → state-machine fn) test hits. `Handle::try_current()` returns `Ok` from BOTH worker AND spawn_blocking-pool threads; can't distinguish them without a thread-local marker (Shape A).

   Shape B drops `block_in_place` entirely. `Handle::block_on` is documented to work on any thread. Tradeoff: when called from a worker thread, ties up that worker for the wait duration. Cost is bounded — M3 eliminates most shim call sites via auto-`wait` insertion; M2 ships `wait_required_on_state_machine_call` warning that steers users toward `wait` (which uses inline poll-and-yield, no shim).

   **`RUNTIME.get().expect(...)` invariant proof (Round 3 Required Fix #4)**: codegen-invariant guarantee — `ynz_rt_init()` is the FIRST instruction in the generated `main`'s entry block whenever any function in the compilation unit contains `wait` OR `background`. Codegen check in P2: assert that `Cg::main_entry_emit` calls `build_call(ynz_rt_init, ...)` before any other emission. P0 Contract #12 validates this invariant by IR snapshot inspection. Conclusion: the `.expect("ynz_rt_init not called...")` panic is unreachable in correct codegen; the .expect() exists as a defense-in-depth assertion against future codegen bugs, with a Rust-string message acceptable because hitting it indicates an internal compiler bug, not user error.
5. Register all 4 new C-ABI shims in `runtime_decls.rs`:
   - `ynz_rt_spawn` → `void.fn_type(&[fn_ptr_2arg, void_ptr, i64], false)`
   - `ynz_rt_async_sleep_create` → `void_ptr.fn_type(&[i64], false)`
   - `ynz_rt_async_sleep_poll` → `i32.fn_type(&[void_ptr, void_ptr], false)`
   - `ynz_rt_call_state_machine_sync` → `i32.fn_type(&[fn_ptr_2arg, void_ptr, i64], false)`
   - Where `fn_ptr_2arg` is the LLVM function-pointer type for `fn(ptr, ptr) -> i32` (resume function signature).
6. Promote spike test fixtures into `crates/ynz-runtime/tests/m2_runtime.rs` as production tests. Carries forward the panic-catch + frame-no-leak assertions from spike.
7. Verify `cargo test --workspace` passes (no behavior change in user-visible code; just new APIs available).
8. Verify `./target/debug/ynz run hello.ynz` still prints `hello, yinz` (the new shims are in `libynz_runtime.a` but unused — should be inert).

**Acceptance criteria**:
- [x] `ynz_rt_spawn` exported as `#[no_mangle] extern "C"`; signature matches spec
  - Evidence: `runtime.rs:374` — `#[no_mangle] pub unsafe extern "C" fn ynz_rt_spawn(resume_fn: unsafe extern "C" fn(*mut u8,*mut u8)->i32, frame_ptr: *mut u8, frame_size: i64)`. `nm libynz_runtime.a` shows `T ynz_rt_spawn`. Exercised by `rt_spawn_drives_state_machine_on_io_pool` (which calls `ynz_rt_spawn` directly after fix-round-1). ✓
- [x] `ynz_rt_async_sleep_create` exported; returns heap pointer to a boxed `Sleep` future
  - Evidence: `runtime.rs:424` — `#[no_mangle] pub extern "C" fn ynz_rt_async_sleep_create(ms: i64) -> *mut u8`; `Box::into_raw(Box::new(Pin<Box<Sleep>>))`. IR snapshot: `declare ptr @ynz_rt_async_sleep_create(i64)`. Test `sleep_create_returns_non_null` asserts non-null + reconstructs `Box<Pin<Box<Sleep>>>` without UB. ✓
- [x] `ynz_rt_async_sleep_poll` exported; correctly polls and re-registers waker
  - Evidence: `runtime.rs:468` — casts `waker_ctx` to `&mut Context<'_>`, calls `sleep_box.as_mut().poll(cx)` (Tokio registers the real timer waker on Pending). Test `sleep_poll_suspend_and_resume` asserts elapsed ≥ 80ms (proves waker-driven, not busy-poll); `sleep_eight_concurrent_share_threads` proves concurrent registration. ✓
- [x] `ynz_rt_call_state_machine_sync` exported; Shape B (uses `Handle::block_on` everywhere inside Tokio; falls back to `RUNTIME.block_on` outside; NO `block_in_place`); correctly drives state machine to completion synchronously
  - Evidence: `runtime.rs:545` — Ok arm `handle.block_on(future)` (line 560), Err arm `rt.block_on(future)` (line 576); `block_in_place` appears only in two explanatory comments, zero executable uses. Returns `i32` (frame slot 0) per Patrick's ABI decision. Tests `call_state_machine_sync_from_spawn_blocking` + `call_state_machine_sync_no_tokio_context` both assert `result == 42` (both thread contexts). ✓
- [x] All 4 new C-ABI shims registered in `runtime_decls.rs`
  - Evidence: `runtime_decls.rs:570+` — `ynz_rt_spawn`→`void.fn_type([ptr,ptr,i64])`, `ynz_rt_async_sleep_create`→`ptr.fn_type([i64])`, `ynz_rt_async_sleep_poll`→`i32.fn_type([ptr,ptr])`, `ynz_rt_call_state_machine_sync`→`i32.fn_type([ptr,ptr,i64])`. All 4 appear as `declare` lines in the 5 golden IR snapshots. ✓
- [x] `libynz_runtime.a` archive size increase ≤ 2MB (Tokio time feature was already in `rt-multi-thread`; should be minimal)
  - Evidence: pre-diff 45,076,802 B → post-diff 45,461,354 B; delta ~375 KB, well under the 2 MB cap. ✓
- [x] `ynz build hello.ynz` succeeds and prints `hello, yinz` (existing behavior unchanged)
  - Evidence: `./target/debug/ynz run crates/ynz-driver/tests/fixtures/hello.ynz` → `hello, yinz`; `cargo build --workspace` clean. New shims declared but no call sites emitted (inert, as planned). ✓
- [x] `cargo test --workspace` passes (no regression; new m2_runtime tests pass) — *AC text amended: the original "1220+ existing tests" was a main-branch figure; this worktree was branched (off d509770) before those tests landed and contains ~190 tests. The semantic bar is "no regression + new tests pass."*
  - Evidence: all crates pass except the 5 known worktree-path ynz-driver snapshot artifacts (snapshots bake absolute path; fail identically at base d509770 — NOT P1 regressions). New m2_runtime: 9/9 pass. ynz-runtime total 91 tests pass; ynz-codegen 22 pass. Zero NEW failures introduced by P1. ✓
- [x] `nm libynz_runtime.a | grep ynz_rt_` shows all 4 new symbols exported
  - Evidence: `nm target/debug/libynz_runtime.a | grep ynz_rt_` shows `T ynz_rt_spawn`, `T ynz_rt_async_sleep_create`, `T ynz_rt_async_sleep_poll`, `T ynz_rt_call_state_machine_sync` (all exported text symbols). ✓

**Quality gate**:
- [x] No `unsafe` outside the extern fns themselves; each unsafe block has a SAFETY comment
  - code-reviewer confirmed every `unsafe` block carries a sound `// SAFETY:` comment; the `waker_ctx`/frame-slot-0 raw-pointer casts are the C-ABI boundary, each justified.
- [x] Tier 3 doc comments on each new shim (Flow / Failure modes / Side effects per `comments.md`)
  - rules-compliance + code-reviewer confirmed Flow/Failure/Side-effects doc comments on all 4 shims; the `ynz_rt_spawn` ownership comment was corrected (fix-round-1) to state the real contract.
- [x] Panic-catch wrappers on every shim (matches M1 pattern in `ynz_rt_spawn_blocking`)
  - `catch_unwind` on `async_sleep_poll` + `call_state_machine_sync`; `ynz_rt_spawn` relies on Tokio's task wrapper (M1 pattern). Validated by `panic_during_state_machine_poll_is_caught`.
- [x] Frame cleanup uses RAII drop guard (matches M1's `CtxDropGuard`)
  - `CtxDropGuard`→`FrameDropGuard` rename complete + used in `ynz_rt_spawn_blocking`. NOTE: the new `ynz_rt_spawn` path intentionally does NOT free its frame in P1 (documented deferral — frame dealloc is the codegen resume_fn's job at terminal state, wired in P2; reviewed + accepted). The drop-guard mechanism exists and matches M1; spawn-frame dealloc is a tracked P2 obligation, not a leak-by-omission.
- [x] No Tokio types exposed in the C-ABI signatures (all params are primitive C types)
  - All 4 shim signatures use only `*mut u8`, `i64`, `i32`, fn-ptr. code-reviewer + plan-adherence confirmed.
- [x] No SQL/security concerns (pure-Rust runtime; no external input)

**Verification**:
- `cargo build --workspace --release` succeeds in < 120s on CI
- `cargo test -p ynz-runtime` shows all m2_runtime tests pass
- `ynz build hello.ynz && ./hello` prints `hello, yinz`

**Phase Review Gates** (filled at phase completion):
- [x] code-reviewer: PASS 2026-05-30T22:45 (re-review round 3 — all 3 round-1 BLOCKs resolved; i32 slot-0 read sound; c-string/drop conversions behavior-preserving; clippy silent)
- [x] rules-compliance-reviewer: PASS 2026-05-30T22:05 (round 1; post-round deltas = doc-truth correction + mechanical clippy, strictly improving)
- [x] plan-adherence-verifier: PASS 2026-05-30T22:08 (round 1; the void→i32 fix now CONFORMS to plan line 779, improving adherence; 5 snapshots mechanical)
- [x] acceptance-verifier: PASS 2026-05-30T22:50 (re-review — all 9 ACs MET; void→i32 verified result==42 both contexts)
- [x] Committed: b740e3d42ea4df35566e4264b26d061b84ded399

**Findings Log** (filled during any fix loops):
- 2026-05-30T22:10 — reviewer round 1: rules-compliance PASS; plan-adherence PASS; code-reviewer BLOCK; acceptance-verifier BLOCK (1 WEAK). Three code findings + 1 plan-text finding.
- 2026-05-30T22:10 — code-reviewer BLOCK #1 (FAKE TEST): `rt_spawn_drives_state_machine_on_io_pool` (m2_runtime.rs:~346) never calls `ynz_rt_spawn` — it spawns via `local_rt.spawn(...)` and admits it in a comment; `ynz_rt_spawn` isn't even imported. One of 4 shims has no real coverage behind a green test bearing its name.
- 2026-05-30T22:10 — code-reviewer BLOCK #2 (WRONG OWNERSHIP DOC): `ynz_rt_spawn` doc (runtime.rs:~300) claims Tokio's RAII drop of `Box<StateFnFuture>` frees the frame on completion/abort. False — `StateFnFuture` holds a raw `*mut u8` with NO Drop impl; nothing frees it. Frame dealloc is deferred to P2 (`frame_size` is `#[allow(dead_code)]` "until P2 wires dealloc"). A P2 executor trusting this skips dealloc → heap leak.
- 2026-05-30T22:10 — code-reviewer concern → CONFIRMED BLOCK (void/i32 ABI): `ynz_rt_call_state_machine_sync` shipped returning `void` (StateFnFuture Output=()), but plan locks `-> i32`/`i32.fn_type` (749/779), line 768 "returns the final value", line 888 main-exit-code propagation, and P0 spike contract #4d returned 42 via the bridge. void silently drops main's exit code; P2 would build main-wrap on the wrong signature. plan-adherence + acceptance called it a "stale plan artifact" but cited no evidence the void was intended. RESOLUTION: Patrick (2026-05-30) — CONFORM IMPL TO i32. (ynz_rt_spawn stays void — fire-and-forget.)
- 2026-05-30T22:10 — acceptance-verifier WEAK (plan-text): AC says "1220+ existing tests" but this worktree has ~190 (branched before those landed in main). Semantic intent met (no regression; 9 new m2_runtime tests pass). Coordinator amends AC text to be worktree-accurate.
- 2026-05-30T22:15 — fix-loop round 1 (coordinator → executor): (1) sync bridge void→i32 read-from-frame-slot-0, runtime_decls i32.fn_type, regen IR snapshots, mirror spike bridge; (2) make the spawn test actually call ynz_rt_spawn; (3) rewrite the ynz_rt_spawn ownership doc to state the truth (frame NOT freed by StateFnFuture drop; dealloc is P2's job). All 3 DONE: sync bridge now `SyncStateFnFuture<Output=i32>` reads frame slot 0; spawn test calls ynz_rt_spawn + signals via SignalSm; doc corrected.
- 2026-05-30T22:25 — fix-loop round 2 (clippy -D warnings cleanup): `let _ = Box::from_raw` → `drop(...)` (intentional Sleep-handle free) ×4; removed orphaned `test_sm_resume`; `redundant_async_block` fixed; 6 pre-existing `manual_c_str_literals` in lib.rs converted `b"x\0"`→`c"x"` (byte-identical). `cargo clippy -p ynz-runtime --tests -- -D warnings` = 0 warnings; 91 tests pass.
- 2026-05-30T22:30 — re-review IN FLIGHT: code-reviewer + acceptance-verifier re-running on final clean diff (rules + plan-adherence already PASS — deltas strictly improve/are mechanical). Awaiting verdicts → then write P1 gates + Evidence + amend stale "1220+ tests" AC text to worktree-accurate count + commit P1. NEXT after P1 commit = Phase 2 (codegen state-machine path). P2 INHERITS: frame-slot-0 return-value ABI (i32 at offset 0) + must add the `main_rt_init_is_first_instruction` hard-gate AC (Contract #12 landing).

**Exit Sequence**: per template (persist → 4-agent review fan-out → handle → prompt).

---

### Phase 2: Codegen — `lower_function_with_waits` State Machine Path

**PR scope**: Implement the state-machine codegen transformation. New `lower_function_with_waits` path in `emit.rs` that detects functions whose body contains `Expr::Wait` and emits an LLVM state machine instead of the standard sequential codegen. Wire the path-selection check; wire `Expr::Wait` to emit poll-and-yield IR; wire `Expr::Background` routing to choose between `ynz_rt_spawn` (state-machine fn) and `ynz_rt_spawn_blocking` (regular fn). Wire `block_on` bridge for non-state-machine callers of state-machine functions. Wire `main` to wrap in `block_on` if main contains `wait`.
**Branch**: `feat/v0-3-m2-codegen-state-machine`
**Flag**: N/A
**Est. lines**: ~600 (largest M2 phase — state-machine IR generation is dense)
**Ships via**: `/pr`
**Objective**: After this PR, a function containing `wait sleepAsync(200)` actually suspends — verified by timing tests. The state-machine path produces correct LLVM IR (verified by snapshot tests); the standard path is untouched.
**Why this phase exists**: This is the milestone's value-delivery moment. The runtime layer (P1) is the substrate; this is the codegen that uses it.
**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs:3139` — current `Expr::Wait` identity-passthrough (the rewrite site)
- `crates/ynz-codegen/src/emit.rs:lower_expr_background` — M1's background lowering (extends here for routing)
- `crates/ynz-codegen/src/emit.rs` — `lower_function` (search for `pub(crate) fn lower_function`) — the standard codegen path that the new `lower_function_with_waits` parallels
- `crates/ynz-codegen/src/emit.rs:916` — `main` initialization (where the `block_on` wrap for main goes if main contains wait)
**Files (expected scope)**:
- `crates/ynz-codegen/src/emit.rs` — `lower_function_with_waits`, `Expr::Wait` codegen, `Expr::Background` routing, `main` wrap
- `crates/ynz-codegen/src/state_machine.rs` — new module: state-machine IR helpers (frame layout, resume_point switch, waker plumbing). Extracted for clarity given the IR density.
- `crates/ynz-codegen/src/lib.rs` — register the new module
- `crates/ynz-codegen/tests/snapshots/` — new snapshot files for state-machine IR output
- `crates/ynz-driver/tests/fixtures/v0_3_m2_*.ynz` — new fixtures (single-wait, multi-wait, wait-in-if, block_on caller)
**Steps**:
1. **AST-walk helper**: add `fn function_contains_wait(body: &Block) -> bool` to `emit.rs` (or a shared utility crate). Recursive walk; returns true if any `Expr::Wait` appears in the body. Cached on the function table (computed once per function during typeck pass).
2. **Path selection** at `lower_function` entry: if `function_contains_wait(body)`, dispatch to `lower_function_with_waits`. Standard `lower_function` path stays the fallback (zero overhead for wait-free fns).
3. **`lower_function_with_waits`** core structure:
   - Generate a state-machine struct type: `{ resume_point: i32, locals_crossing_wait: [N x i64], awaited_handle: i64 }`
   - Generate a `resume(frame: *mut u8, waker_ctx: *mut u8) -> i32` function — the actual state-machine logic
   - The resume function body: switch on `resume_point`; each case executes the statements between two wait points
   - At every `Expr::Wait(inner, _)`:
     - Lower `inner` — if it's a call to a may-block intrinsic (currently only `sleepAsync` + internal `__testFallibleAsync`), call `ynz_rt_async_sleep_create` (or the internal equivalent), store the returned handle in the frame
     - Emit a poll loop: call `ynz_rt_async_sleep_poll(handle, waker_ctx)` → if Ready, continue to next state; if Pending, save state to `resume_point`, return `1` (Pending)
   - At normal function exit: free awaited handle if non-null, save final return value to first frame slot, return `0` (Ready)
4. **Frame allocation**: at the original function's "entry point" (where the standard codegen would emit prologue), instead emit:
   - `ynz_alloc(frame_size)` → frame ptr
   - Initialize `resume_point = 0`, copy args into frame slots
   - The caller decides whether to drive this via `ynz_rt_spawn` or `ynz_rt_call_state_machine_sync` — see steps 5/6 below
5. **`Expr::Background` routing**: existing `lower_expr_background` (M1) currently emits `ynz_rt_spawn_blocking` unconditionally. Extend: if the callee is a state-machine function (per the `function_contains_wait` check OR callee name is in `M2_MAY_BLOCK_INTRINSICS`), emit `ynz_rt_spawn` instead. The frame is allocated at the call site (same heap pattern as M1's ctx).
6. **Call-site dispatch decision algorithm (4 cases — per plan-reviewer Round 1 Required Fix #10 + Round 2 Required Fix #1/#3)**: when codegen encounters a `Call` expression with a state-machine callee, the algorithm depends on whether the call is `wait`-wrapped AND whether the caller is itself a state machine:

   | Caller is state machine? | Call wrapped in `wait`? | Codegen emits |
   |---|---|---|
   | No | No | `ynz_rt_call_state_machine_sync(resume_fn, frame_ptr, frame_size)` — Shape B sync bridge (uses `Handle::block_on` everywhere inside Tokio; falls back to `RUNTIME.block_on` outside; NO `block_in_place`). Correctness AND panic-safety preserved in transitive cases (per Round 3 Required Fix #1 — `block_in_place` would panic on spawn_blocking-pool threads). |
   | No | Yes | **Unreachable** — writing `wait` upgrades caller's `contains_wait` to true at typeck time (per P3 Step 5 AST walk), so this row never fires. Listed for completeness; no codegen emission. |
   | Yes | No | **Typeck warning** via `wait_required_on_state_machine_call` (P3 Step 8). Codegen ALSO emits `ynz_rt_call_state_machine_sync` as the fallback path — the diagnostic is teaching-only (guides users to write `wait` for perf), not a correctness gate. Runtime path is panic-safe via the runtime-aware shim. |
   | Yes | Yes | Inline poll-and-yield sequence (state-machine resume-point increment + frame slot update) |

   **Background-call exemption**: if the call is the immediate inner of `Expr::Background`, the (Yes, No) row's warning does NOT fire (per Round 2 Required Fix #2). `background smB()` from inside a state machine is the legal route-to-I/O-pool pattern (P2 Step 5).

   For regular (non-state-machine) callees, the decision is trivial: direct call regardless of `wait` wrapping. The `wait_on_non_may_block` warning fires from typeck before reaching codegen.

7. **`Expr::Wait` codegen — explicit `sleepAsync` dispatch arm** + **waker ABI** (per Round 3 Required Fix #3): at `emit.rs:2490` where `sleepMs` is dispatched, add a parallel `sleepAsync` arm. When the inner of `Expr::Wait` is `Call { callee: "sleepAsync", args: [ms] }`:
   - Emit `ynz_rt_async_sleep_create(ms)` → `*mut u8` handle, stored in current frame's `awaited_handle` slot
   - Emit poll loop (resume-point branch): call `ynz_rt_async_sleep_poll(handle, waker_ctx)` → check return: 0=Ready→continue; 1=Pending→save state to `resume_point`, return Pending

   **Waker ABI (LOCKED per Round 3 Required Fix #3)**: every state-machine resume function takes `waker_ctx: *mut u8` as its second parameter. The value passed in is `&mut std::task::Context<'_>` from the outer driver (either Tokio's `Future::poll` Context or `Handle::block_on`'s synthetic Context), cast to `*mut u8`. Codegen MUST forward this exact `waker_ctx` pointer (NOT a fabricated Waker) into any inner `ynz_rt_async_sleep_poll` / `ynz_rt_test_fallible_async_poll` call. The runtime-side `Sleep::poll` casts it back via `&mut *(waker_ctx as *mut Context<'_>)` and uses the Context's Waker to register the wakeup notification. **Forbidden**: fabricating `Waker::noop()` or any synthetic Waker — would silently hang any task awaiting under a quiet runtime. P0 Contract #11 validates this end-to-end.

   Parallel arm for `__testFallibleAsync` (internal): when the inner of `Expr::Wait` is `Call { callee: "__testFallibleAsync", args: [shouldFail] }`, emit calls to the internal `ynz_rt_test_fallible_async_create` + `_poll` shims (added in P1 alongside the public sleepAsync shims). Test-only ABI; not callable from non-test fixtures because of the `internal_fns` lookup gating. Same waker_ctx forwarding rule applies.
8. **`main` wrap**: if `function_contains_wait(main_body)`, transform main into a state machine. At the actual `main` LLVM function entry (after `ynz_rt_init`), emit `ynz_rt_call_state_machine_sync(main_state_machine_resume, frame_ptr, frame_size)`. Exit code propagates from the state machine's final value. (`main` is outside any Tokio context at this point — `Handle::try_current()` returns Err — so the shim falls through to `RUNTIME.block_on`. Same effect as the originally-planned bare `block_on`; routes through the unified shim for consistency.)
9. **Snapshot tests**: add IR snapshots for:
   - `function with one wait` (single state transition)
   - `function with two sequential waits` (multi-state)
   - `function with wait inside if` (branching state machine)
   - `function calling state-machine function without wait` (non-SM caller — block_on bridge emitted)
   - `main with wait` (block_on wrap at main entry)
   - `background spawn of state-machine fn` (routed to `ynz_rt_spawn`)
   - `background spawn of regular fn` (routed to `ynz_rt_spawn_blocking` — verifies M1 behavior unchanged)
10. **Driver-level concurrency-proof test (per plan-reviewer Required Fix #8)**: fixture `crates/ynz-driver/tests/fixtures/v0_3_m2_concurrent_waits_proof.ynz`:
   ```yinz
   function pause(n: int) -> nothing {
     print(`START ` + n.toString())
     wait sleepAsync(100)
     print(`DONE ` + n.toString())
   }
   function entrypoint() -> nothing {
     background pause(1)
     background pause(2)
     background pause(3)
     background pause(4)
     background pause(5)
     background pause(6)
     background pause(7)
     background pause(8)
     print(`MAIN`)
   }
   ```
   **Concurrency proof assertion (decoupled from CI core count)**: parse stdout; assert all 8 `START N` lines appear BEFORE any `DONE N` line. If state machines are working, all 8 pauses are scheduled concurrently and all 8 STARTs print before any wait completes. If state machines are broken (sequential blocking), `START 1 / DONE 1 / START 2 / DONE 2 / ...` would interleave per task — `START 2` would NOT appear before `DONE 1`. This proof is core-count-independent: even on a 1-core CI runner the cooperative scheduling fires this pattern.

   **Timing band assertion (validates state-machine perf, not just correctness)**: total wall-clock in band `[80ms, 200ms]`. Lower bound catches broken-sleep-returns-instantly; upper bound catches sequential execution (would be ~800ms).

**Acceptance criteria**:
- [x] `function_contains_wait` AST walker correctly identifies wait-containing functions; verified by unit tests
  - Evidence: `crates/ynz-codegen/src/emit.rs:8083-8151` — 3 `#[cfg(test)]` unit tests call `function_contains_wait` directly: `block_with_top_level_wait_returns_true`, `block_without_wait_returns_false`, `block_with_if_nested_wait_returns_true` (added fix-round-2 per AV1). `cargo test -p ynz-codegen` → 30/30 pass.
- [x] `lower_function_with_waits` path emits state-machine IR; verified by IR snapshot tests
  - Evidence: `crates/ynz-codegen/tests/golden.rs` — `v03_m2_single_wait_ir_snapshot`, `v03_m2_multi_wait_ir_snapshot`, `v03_m2_wait_in_if_ir_snapshot`, `v03_m2_main_with_wait_ir_snapshot` all pass (insta snapshots lock the SM IR). 30/30 codegen green.
- [x] Wait-free functions still use `lower_function` path (no change); verified by snapshot test of an unrelated wait-free fn
  - Evidence: existing wait-free golden snapshots (`m2_smoke_ir_snapshot`, `m3_fib_sha256_golden`, `v03_m1_background_ir_snapshot`) unchanged + still pass; unit test `block_without_wait_returns_false` (emit.rs:8119) proves the routing gate returns false for wait-free blocks.
- [x] `Expr::Wait` lowering emits poll-and-yield sequence; IR snapshot asserts exact instruction order
  - Evidence: `golden__v03_m2_single_wait_ir.snap` + `v03_m2_wait_in_if_ir_snapshot` lock the `ynz_rt_async_sleep_poll` + `sm_pending` branch sequence; the wait_in_if snapshot now carries behavioral asserts (`async_sleep_poll` reachable in branch, `sm_pending` has live predecessors — finding #2 fix).
- [x] `main_rt_init_is_first_instruction`: IR snapshot of a `main` whose compilation unit contains `wait` shows the first non-allocation instruction is `call void @ynz_rt_init()` (HARD GATE — this is the landing spot for P0 Contract #12, which was deferred here because no `wait` codegen existed at the spike layer; accepted by Patrick at the P0 gate 2026-05-30. Validates Question Resolution #9's invariant that makes the `RUNTIME.get().expect("ynz_rt_init not called")` panic unreachable in correct codegen.)
  - Evidence: `crates/ynz-codegen/tests/golden.rs:main_rt_init_is_first_instruction` test passes (scans IR for `ynz_rt_init` before any other non-alloc instruction); `golden__v03_m2_main_rt_init_first.snap` shows `ynz_siphash_init` / `ynz_rt_init` before `ynz_rt_call_state_machine_sync`.
- [x] `Expr::Background` of a state-machine fn → `ynz_rt_spawn`; of a regular fn → `ynz_rt_spawn_blocking` (M1 behavior preserved)
  - Evidence: `v03_m2_background_spawn_sm_fn_ir_snapshot` (asserts `ynz_rt_spawn`) + `v03_m2_background_spawn_regular_fn_ir_snapshot` (asserts `ynz_rt_spawn_blocking`) both pass.
- [x] `block_on` bridge emitted at call sites where caller-not-state-machine + callee-state-machine + no-wait-wrapper
  - Evidence: `v03_m2_non_sm_caller_block_on_ir_snapshot` shows `ynz_rt_call_state_machine_sync` emitted in the non-SM caller body (Shape B sync bridge).
- [x] `main` with wait wraps in `block_on`; exit code propagates correctly
  - Evidence: `v03_m2_main_with_wait_ir_snapshot` locks the block_on wrap; `main_rt_init_first.snap` shows `ret i32 %sm_result` (exit code flows from the SM result); concurrency-proof binary exits 0 (integration.rs).
- [x] `v0_3_m2_concurrent_waits_proof.ynz` driver test: all 8 `START N` lines appear before any `DONE N` line (concurrency proof, core-count-independent); total wall-clock in band `[80ms, 200ms]`
  - Evidence: `crates/ynz-driver/tests/integration.rs:v03_m2_concurrent_waits_proof` passes. The ORDERING assertion (`last_start < first_done` — all 8 STARTs before any DONE) is the deterministic, core-count-independent no-op + sequential-execution catcher (code-reviewer verified it fails under a no-op). NOTE: realized band is `[80ms, 2000ms]` sanity-only, NOT the plan's `[80,200]` — main's blocking `sleepMs(300)` keep-alive (required since `background` is fire-and-forget pre-M4) dominates wall-clock, so timing cannot detect a sleepAsync no-op; the ordering assert is the real guard. Misleading timing comments corrected fix-round-2 per code-reviewer Concern #1.
- [~] `wait_required_on_state_machine_call` warning fires when state-machine fn calls state-machine fn without `wait` AND without `background`; typeck emits Tier 3 warning (exit code 0; stderr substring `warning:`) verified by snapshot
  - **DEFERRED TO PHASE 3 (coordinator adjudication 2026-05-31):** this is a typeck WARNING and the plan lists it BOTH here and in Phase 3 Step 8 alongside the other M2 warnings (`wait_on_non_may_block`, `wait_on_non_call_expression`, `unawaited_sleep_async`). It belongs with them in Phase 3. The P2 codegen DOES emit the `ynz_rt_call_state_machine_sync` fallback for the no-wait SM→SM call (4-case dispatch, Step 6 — verified by `v03_m2_non_sm_caller_block_on_ir_snapshot`), so the runtime path is correct; only the teaching warning defers. Not a Phase 2 gap.
  - Evidence: deferred to Phase 3 (typeck warnings phase)
- [x] `cargo test --workspace` passes (new snapshots + new driver + typeck tests; only acceptable failures are the 5 ENVIRONMENTAL worktree-absolute-path driver snapshot tests — `broken_main`, `empty_source`, `m2_compound_assign`, `m2_const_reassignment`, `m2_mixed_int_number` — which pass on main; the M1-era "1220+ tests" figure is stale/not worktree-accurate and is not the bar)
  - Evidence: `cargo test --workspace` → 106 passed / 5 failed; the 5 are EXACTLY the named environmental tests (each failure diff is purely the absolute-path prefix `/workspaces/ynz/...` vs the worktree path — they pass on main). No other failures. codegen 30/30, typeck 28/28 (+168 check.rs) green.

**Quality gate**:
- [ ] State-machine frame slots are exactly the locals that cross a wait boundary (not all locals — wasted memory) — verified by IR inspection
- [ ] Frame allocation paired with `ynz_free` via RAII guard (matches M1 pattern)
- [ ] No N+1 codegen — poll-loop emits a single `build_call` to `ynz_rt_async_sleep_poll`, not a loop of calls
- [ ] `function_contains_wait` is cached per function; not re-walked on every codegen access
- [ ] Snapshot tests cover both state-machine and wait-free paths to prevent regression
- [ ] Codegen + typeck wall-clock on `examples/pirates-roster/` ≤ 10% slower than M1 baseline (measured)

**Verification**:
- `cargo test --workspace` shows no regression
- `./target/debug/ynz run crates/ynz-driver/tests/fixtures/v0_3_m2_concurrent_waits_proof.ynz` shows all 8 done lines + main done in ≤ 200ms wall-clock (band [80ms, 200ms])
- IR snapshot tests stable

**Phase Review Gates** (filled at phase completion):
- [x] code-reviewer: PASS 2026-05-31T (review round 2, after fix-loop round 2) — round-1 BLOCK (4 findings incl. CR1 if-nested-crossing crash) all resolved; ran 10 fresh adversarial inputs live (mixed param+local, boundary, match-nested, deep if-in-if, mutate-after-wait, shadowing) — none crash; CR1/CR2 genuinely fixed. One non-blocking Concern #1 (vacuous timing-guard comment) fixed this round via comment-accuracy correction.
- [x] rules-compliance-reviewer: PASS 2026-05-31T (review round 2, file-based diff) — R1–R4 resolved (no `// Phase N`, `.unwrap()` justified); new `WaitInsideLoop`/`LocalCrossesWait` templates registered + plain-vocabulary; no new violations. (Two earlier rules runs hit a cwd footgun — `git` ran in main checkout reviewing a phantom rollback — re-run against materialized diff file.)
- [x] plan-adherence-verifier: PASS 2026-05-31T (review round 2) — Steps 1–10 + Option B all MET; fixtures staged (PA1/PA2); plan-deletions excluded at commit (PA3/PA4); timing band noted DEVIATED-WITH-REASON (ordering assert is the real guard).
- [x] acceptance-verifier: PASS 2026-05-31T (review round 2) — OVERALL PASS; all 11 ACs MET (AV1 unit tests added, AV2 lower-bound added then comment-corrected, AV3 `wait_required` adjudicated-deferred-to-P3); workspace 106 pass / 5 environmental-only.
- [x] Committed: 5109b52

**Findings Log** (filled during any fix loops):

**2026-05-31 — Phase 2: implemented by a prior (crashed) session, UNCOMMITTED, code-reviewer BLOCK.** Happy path (top-level sequential `wait`) works end-to-end: concurrency-proof fixture passes (8 background tasks, all STARTs before any DONE, exit 0); 8 IR golden snapshots green; bare-binary top-level `wait sleepAsync(120)` measured 124ms ✓. But the review fan-out + my own verification (paper-traced) found:

- **Finding #1 (CORRECTNESS, CONFIRMED):** `wait` inside `if`/`while`/`for` is a **silent no-op**. `lower_sm_stmt_with_wait` (emit.rs:1643) matches only top-level `Stmt::Expr(Wait)` / `Stmt::Let{Wait}`; nested waits fall through `_ =>` to plain lowering → the bare `sleepAsync` arm creates a Tokio timer and **discards it**, running straight through. Evidence: `golden__v03_m2_wait_in_if_ir.snap:224-226` (`if_then` calls `async_sleep_create(100)` then `br if_merge`, **no poll**; `sm_pending` dead — "No predecessors!"). Bare-binary proof: wait-in-`if` 80ms→ran 3ms; wait-in-`for`×3 150ms→2ms; vs top-level control 120ms→124ms. Root cause = `lower_function_with_waits` does a FLAT top-level statement split, not a CFG-based suspend-point transform.
- **Finding #2:** `v03_m2_wait_in_if_ir_snapshot` (golden.rs:594) asserts symbol-presence only → froze the broken IR as golden. Needs behavioral asserts after #1.
- **Finding #3 (CORRECTNESS, CONFIRMED):** `background sm_fn()` **leaks the heap frame** — spawn path never frees it; `SpawnStateFnFuture` (runtime.rs:311) has no `Drop`; runtime.rs:350-354 punts frame-free to "Phase 2" and Phase 2 didn't wire it. Evidence: `single_wait` snapshot L248 `ynz_rt_spawn` w/ no `ynz_free`. Violates `state_machine_no_leak` Safety invariant. (Also: in-flight `Sleep` box leaks on mid-wait cancellation — runtime.rs:419.)
- **Finding #4:** `param_to_i64_bits` (emit.rs:284, wrapper path) drifted from sibling `Cg::to_i64_bits` — omits `Number`/`BuiltinFixed`/`MapEntry` → SM fn taking those params fails to compile. Parallel-impl per no-duct-tape #7; collapse to one marshaller.

**Research (2026-05-31, Patrick asked "are loops always wait / is nesting allowed?"):**
- Loops are NOT "always wait." Loop back-edges get a **preemption** safe-point (`ynz_rt_check_preempt`, `design/future/concurrency.md:198`) — a cooperative yield, a DIFFERENT mechanism from `wait`. A loop suspends on a timer only if its body has explicit `wait`.
- Control-flow nesting IS allowed (parser recursive; `if`-in-`if` runs; examples nest). The "no nesting" rule is specific to test `group` blocks (`design/testing.md:75`); `spec/control-flow.md:50` is a style guideline (prefer early-return), not a ban. → Plan P0 Contract #3 (wait-in-if) + #7b (wait-in-loop) require nested wait to WORK; descoping would make `wait` in `if`/`for` a surprising compile error.

**DECISION (Patrick, 2026-05-31): FIX PROPERLY.** Confirmed after research alignment — loops are NOT auto-wait (that's M3 auto-`wait`-insertion on I/O calls + preemption-at-back-edge, both separate from suspension); M2 must make an EXPLICIT `wait` at any nesting depth actually suspend, because M3's auto-inserted `wait` emits the same construct — a broken M2 foundation breaks the M3 "loops over I/O auto-suspend" vision. Frame-leak (#3) + param-drift (#4) fixed regardless. **Phase 2 is NOT done; do not commit until #1/#3/#4 fixed + snapshot #2 corrected + review re-run.** NOTE: stray compiled binary `crates/ynz-driver/tests/fixtures/v0_3_m2_concurrent_waits_proof` (no extension) was removed; do not commit build artifacts.

**P2-FIX DESIGN (2026-05-31, after reading state_machine.rs + lower_function_with_waits/lower_sm_body/lower_sm_stmt_with_wait/emit_wait_point):**

Current architecture (what's RIGHT, keep it): frame layout (resume_point@0, sleep_handle@8, i64 local slots@16+); `count_waits_in_block` already recurses (counts nested waits → correct state-block count); entry switch dispatches resume_point→state_blocks[i]; `emit_wait_point` does correct suspend/poll/resume for ONE wait and leaves builder at `post_wait_bb`; `emit_sleep_poll_branch` takes continue_bb/pending_bb (reusable). Waker ABI + Shape-B sync bridge + frame helpers (state_machine.rs) are all sound and stay.

What's WRONG: `lower_sm_body`/`lower_sm_stmt_with_wait` only handle TOP-LEVEL `Stmt::Expr(Wait)` + `Stmt::Let{value:Wait sleepAsync}`; `Stmt::If/While/For/Match` containing a wait fall through `_ => lower_stmt` → nested wait hits the bare create-and-discard arm. AND only params get frame slots — locals/loop-induction crossing a wait would be lost on resume (latent bug beyond #1).

Fix = real stackless-coroutine transform. Three connected increments:
- **(A) wait-in-if (P0 Contract #3) + wait-in-if-condition (#7c):** make the SM walker handle `Stmt::If`: eval cond (cond itself may contain a wait → emit_wait_point first), conditional-branch to then/else blocks, recursively SM-lower each branch block (so a nested wait gets its own pre-allocated continuation state + suspend/poll/resume), both branches branch to a merge block, position builder at merge. The `current_state` counter threads through recursion (each wait consumes the next state, regardless of nesting depth — matches `count_waits_in_block`'s pre-count).
- **(B) wait-in-loop (P0 Contract #7b):** SM-lower `Stmt::While`/`Stmt::For` with the back-edge routed so resume re-enters correctly. **Loop induction var (i / iterator cursor) MUST be a frame slot** (survives suspension), not a stack alloca. For `Stmt::For` check the desugared form (parser desugars to while + synthetic let per state.md fmt notes) — operate on the desugared AST or handle For directly.
- **(C) locals crossing a wait → frame slots:** generalize `n_locals` beyond params. Over-approximate for M2 correctness = give EVERY `let` local (and loop var) a frame slot (M3 liveness narrows it later — memory cost, not correctness). Intercept Let-binding in SM context to back it with a frame slot + reload-per-state like params. (If full generality is too big in one pass, the MINIMUM correctness bar is: any local read after a wait must be frame-backed; a local never crossing a wait can stay a stack alloca.)

Plus #2 (wait_in_if snapshot → behavioral asserts: assert `sm_suspend` + `async_sleep_poll` reachable in the branch, `sm_pending` not dead), #3 (SpawnStateFnFuture `Drop` frees frame + in-flight Sleep box on cancel), #4 (delete `param_to_i64_bits`, reuse `Cg::to_i64_bits`). New fixtures: nested-wait-in-if + wait-in-for with timing/ordering asserts (the current concurrency proof only exercises top-level wait, can't catch #1). Re-run 4-agent review fan-out before commit.

**Risk note:** this is the milestone's hardest codegen. Implement methodically; verify each increment with a bare-binary timing test (the only thing that catches a no-op — golden IR snapshots alone froze the #1 bug). May span >1 work session; that's correct, not slow.

**PROGRESS 2026-05-31 (uncommitted, on top of staged P2):**
- ✅ **#4 DONE** — extracted single `value_to_i64_bits(builder, i64_ty, val, resolved)` free fn (emit.rs); `Cg::to_i64_bits` delegates after `resolve_type`; SM wrapper param-store calls it; deleted drifted `param_to_i64_bits`. Now covers Number/BuiltinFixed/MapEntry. Build green; only IR change = `bool_ext`→`widen` (same zext, moved to shared store helper) in wait_in_if snapshot — benign, no other snapshot touched.
- ✅ **#3 DONE (frame)** — added `impl Drop for SpawnStateFnFuture` (runtime.rs) freeing the frame via `crate::ynz_free(frame_ptr, frame_size)` on completion AND cancellation; removed `#[allow(dead_code)]` on frame_size; corrected `ynz_rt_spawn` doc (no longer "intentionally leaks"). Sync path unchanged (frees at codegen call site — no double-free). Concurrency proof passes, no crash; 91 runtime tests green. **DEFERRED to Increment A:** in-flight `Sleep`-box-on-mid-wait-cancel (needs the handle-slot null-on-Ready discipline to avoid double-free; will wire in the wait-codegen rework). **TODO test:** real-codegen alloc/free-balance assert for `background sm_fn ×N` (spike has `contract_6_frame_ownership_no_leak` but that's the hand-written future, not SpawnStateFnFuture driving real frames).
- ✅ **#1 INCREMENT A DONE (wait-in-if)** — extracted reusable `lower_sm_block` (the SM-aware statement walker) from `lower_sm_body`; `lower_sm_stmt_with_wait` now has a `Stmt::If` arm that mirrors `lower_stmt_if` (cond → then/merge) but recurses into the branch with `lower_sm_block` so a nested wait gets a real continuation state + suspend/poll. Yinz `if` has no else (Match owns that). VERIFIED: bare-binary `wait sleepAsync(80)` in if = **86ms** (was 3ms no-op); IR `sm_if_then` calls `async_sleep_poll` + `sm_pending` now has live preds (was dead); b=false skips in 4ms; mixed nested-80ms + trailing-toplevel-60ms prints pre/mid/post ordered in 147ms; 29/30 codegen tests pass + concurrency proof green (only wait_in_if snapshot red = froze old broken IR).
- ⏳ **#1 INCREMENT B (wait-in-while/for) NOT STARTED** — needs loop back-edge routed through resume + **induction var (i / iterator cursor) in a frame slot** (currently only params get slots → a loop counter would be lost on resume). `lower_stmt_while` @2459, `lower_stmt_for` @2496. The `_ =>` fallback in `lower_sm_stmt_with_wait` still no-ops a wait in a loop until this lands (uncommitted WIP only; P2 commit gated on A+B+C).
- ⏳ **#1 INCREMENT C (locals crossing a wait → frame slots) NOT STARTED — and it's the FOUNDATION for B.** VERIFIED 2026-05-31: `let x = 5; wait sleepAsync(30); print(x)` currently **CRASHES codegen** ("LLVM verify: Instruction does not dominate all uses" — the `let` alloca is created in state_0's flow but the resume block uses it without domination). So today's "top-level waits work" holds ONLY when no non-param local crosses a wait. Params are READ-ONLY in Yinz (`n=9` → typeck error "parameters cannot be reassigned"), so the current reload-only/no-flush handling is correct for params — but `let` locals (and loop induction vars) are mutable and cross waits. **Required mechanism:** every `let`/loop-var in a wait-containing fn gets a frame slot (over-approximate for M2); the resume fn (a) reloads all slots at each state-block entry (already done for params), and (b) **FLUSHES all live locals back to their frame slots BEFORE each suspend** (the missing piece — a loop counter increments each iteration and must persist across the iteration's wait). emit_wait_point's suspend path currently stores only handle+resume_point, no local flush. This is the genuine hard core (frame-backed mutable locals + flush-on-suspend); do C first, then B (loop var = just another frame-backed local).

**SCOPE DECISION RESOLVED (Patrick, 2026-05-31 via /execute-plan): OPTION B.** `wait`-inside-loop AND `let`-crossing-`wait` emit a CLEAN teaching compile error (WHAT/WHAT-INSTEAD/WHY pointing to M3) instead of the current silent no-op (loop) / codegen crash (let-crossing). Emitted at TYPECK so the codegen `_ =>` no-op fallback becomes unreachable. The full frame-backed-mutable-locals coroutine transform (Increments B + C) defers to M3 alongside auto-`wait` insertion, landing onto the spike-proven loop ABI (Contract #7b). M2 demo uses top-level waits only, so this descopes nothing user-visible in the shipped demo. Recorded in state.md Active Decisions.

VERIFIED scope of B/C: `lower_stmt_for` is **566 lines across ~5 iteration forms** (string-iter, shape/Iterable-iter, BuiltinArray-iter, + range/fixed/map below), each with its own internal counter allocas. Making `wait`-inside-any-loop work = SM-aware variants of ALL those forms + `while` + the frame-backed-mutable-locals subsystem (flush-before-suspend + reload-at-resume) + the `let`-crossing-wait dominance fix. That is the general stackless-coroutine-locals transform — **M3-scale machinery**. Crucially: the M2 SHIPPED demo (P5: 8 background `pause()` tasks) uses TOP-LEVEL waits only — no loop-with-wait. The spike already proved the loop ABI (Contract #7b), so M3 wires codegen onto a proven foundation.

THE FORK:
- **Option B (RECOMMENDED) — clean-error, defer to M3:** ship M2 with top-level + `if` waits working (A done). `wait`-inside-loop AND `let`-crossing-wait emit a CLEAN teaching compile error (replaces the silent no-op — the ACTUAL bug — with an honest error). Demo ships. Heavy transform lands in M3 alongside auto-`wait`, where it belongs + ABI already proven. Phase 2 finishes ~today.
- **Option A — full coroutine transform now:** frame-backed mutable locals + all ~5 for-forms + while + let-crossing fix. Multi-session; pulls M3-scale machinery into M2; higher risk of subtle suspend/resume state bugs.

If Option B chosen: the clean error is best emitted at TYPECK (P3) — detect `wait` inside while/for/match body, and `let` whose value is read after a `wait`, → WHAT/WHAT-INSTEAD/WHY error pointing to M3. The `_ =>` no-op fallback in `lower_sm_stmt_with_wait` is then unreachable (typeck rejects first). Once Patrick decides, record the decision in state.md Active Decisions (it's an M2/M3 boundary architectural call).
- ⏳ **#2 (wait_in_if snapshot)** — still red; NOW it froze the OLD broken no-op IR but the code emits CORRECT IR, so regenerate with behavioral asserts (`assert sm_suspend + async_sleep_poll reachable in branch`, `sm_pending not dead`). Do this after B/C so the fixture can also cover loops. Do NOT blind-`insta accept`.
- ⏳ **#3 box-on-cancel** — CONFIRMED SAFE TO WIRE: `emit_wait_point` post_wait_bb nulls slot-8 on Ready (emit.rs:1856), so slot-8 is non-null only mid-Pending → `SpawnStateFnFuture::Drop` can free slot-8 Sleep box if non-null without double-free. Add to the Drop (needs a documented `FRAME_SLEEP_HANDLE_OFFSET=8` const in runtime mirroring state_machine.rs).

**FIX-LOOP RESUME STATE (2026-05-31, /execute-plan unattended):** Option B chosen + recorded (state.md + SCOPE DECISION block). Phase 2 fix executor returned DONE, **zero deviations**. Implemented: (a) Option-B typeck clean errors `WaitInsideLoop` + `LocalCrossesWait` in `check.rs` (params exempt; replace the no-op + codegen-crash paths), (b) 2 new `[[diagnostic_template]]` registry entries, (c) #2 wait_in_if snapshot → behavioral asserts (codegen 30/30 green), (d) #3 box-on-cancel Drop frees slot-8 Sleep box via `FRAME_SLEEP_HANDLE_OFFSET`. Coordinator-verified: codegen 30/30, typeck 28/28, bare-binary proofs (top-level wait ~120ms suspends; wait-in-if suspends; wait-in-loop → clean exit-1 error; local-crossing-wait → clean exit-1 error, no backend crash); concurrency-proof green. `cargo test --workspace` = 106 pass / 5 fail, where the 5 are ENVIRONMENTAL worktree-absolute-path snapshot diffs (`broken_main`, `empty_source`, `m2_compound_assign`, `m2_const_reassignment`, `m2_mixed_int_number` — pass on main; NOT regressions; do NOT accept their `.snap.new`). **4-agent review gate dispatched** (code-reviewer a77d244, rules-compliance a971e82, plan-adherence abbcb57, acceptance a3acaee); D_count=0 so no deviation-judges. Awaiting verdicts → on all-PASS: write AC ticks + Phase Review Gates, then commit Phase 2 staging ONLY Phase-2 code files (NOT the unrelated `v0-3-m1`/`webpage-foundation` plan deletions in the worktree working tree), then proceed to Phase 3.

**FIX-LOOP ROUND 2 — 4-agent gate returned BLOCK ×4 (2026-05-31).** All 4 reviewed the CORRECT diff (a prior fan-out hit a cwd-reset footgun: reviewers' `git diff` ran in the main checkout via `..HEAD`, reviewing a phantom rollback — re-dispatched with cwd-immune `git -C <abspath> diff b740e3d`). Consolidated real findings → ONE fix round:
- **CR1 (CRITICAL, code-reviewer, empirically confirmed):** `locals_crossing_wait` only tracks TOP-LEVEL waits; a local declared before an `if`-nested wait (which DOES suspend per Increment A) and read after it still CRASHES the backend ("Instruction does not dominate all uses"). 3 variants confirmed. This is the exact crash Option B was chartered to replace with a clean error — the top-level-only boundary was duct tape. FIX: generalize the crossing check to any nesting depth (declared-before-wait + read-after-wait, recursing into `if` bodies). Must NOT over-reject working cases (params; locals entirely before/after a wait). + adversarial if-nested test.
- **CR2 (code-reviewer):** `emit.rs` `_ =>` fallback arm has a stale "uncommitted WIP / P2 needs A+B+C" comment + silently `lower_stmt`s a nested wait. Under Option B that path must be unreachable (typeck rejects). FIX: rewrite comment to current constraint; make the arm `unreachable!()`/internal-error, not a silent no-op.
- **R1–R3 (rules-compliance):** banned "Phase N" comment framing — `golden.rs` "(Step 9 of Phase 2 plan)", `state_machine.rs` "// Phase 1:"/"// Phase 2:" (algorithm steps) → "Step N"/temporal-neutral.
- **R4 (rules-compliance):** `.max().unwrap()` in driver concurrency test needs a justifying safety comment.
- **AV1 (acceptance, WEAK):** AC says `function_contains_wait` "verified by unit tests" but only golden/integration tests exist. FIX: add direct unit tests (wait→true, no-wait→false, nested-if-wait→true).
- **AV2 (acceptance, WEAK):** concurrency-proof test asserts only `elapsed < 5000ms` — NO lower bound. A no-op sleep would pass (the exact bug #1 failure mode). FIX: add an execution-time lower-bound guard (≥~80ms) that fails on a no-op; tighten the upper bound. If the measurement includes compile time, restructure to time execution only (build once, then time the binary) per the plan's "bare-binary timing" intent.
- **SSOT concern (code-reviewer / feature-registry.md):** diagnostic strings literal in `check.rs` AND in `registry/features.toml`. Investigate the established pattern; align (consume registry-generated constant) if that's the norm, else document the carve-out.
- **Coordinator-handled (NOT executor):** stage only Phase-2 files at commit (exclude `v0-3-m1`/`webpage-foundation` plan deletions — plan-adherence PA3/PA4); annotate `wait_required_on_state_machine_call` AC as deferred-to-P3 (acceptance AV3 / plan-adherence — it's a typeck warning belonging with the other P3 warnings); correct stale "1220+ tests" AC text. `git add` the 2 untracked error fixtures so they're tracked + re-reviewable (PA1/PA2 — folding into the executor round).

**Exit Sequence**: per template.

---

### Phase 3: Typeck — `sleepAsync` Intrinsic, `__testFallibleAsync` Internal, May-Block Tracking, Warnings

**PR scope**: Register `sleepAsync(int) -> nothing` as a public may-block intrinsic. Register `__testFallibleAsync(bool) -> int errors` as an internal-only test intrinsic (never in registry). Add per-function "contains-wait" tracking that codegen consumes. Add `wait_on_non_may_block` warning + `wait_on_non_call_expression` error.
**Branch**: `feat/v0-3-m2-typeck`
**Flag**: N/A
**Est. lines**: ~250
**Ships via**: `/pr`
**Objective**: Cover the typeck-level surface for new intrinsics + new failure modes. Without these, `sleepAsync` doesn't exist as a callable, `__testFallibleAsync` doesn't exist for tests, and users writing nonsense `wait`s get no teaching diagnostic.
**Why this phase exists**: P2 codegen needs typeck to flag functions as state machines and to know which calls are may-block. P5 demo + error gallery needs the warnings to exist.
**Current-state anchors** (verified during plan-reviewer Round 1):
- `crates/ynz-typeck/src/check.rs:1261-1271` — current `Expr::Wait` typeck (kernel-mode + inner type infer)
- `crates/ynz-typeck/src/check.rs:1452-1453` — current `sleepMs` typeck dispatch arm (model for `sleepAsync` arm)
- `crates/ynz-typeck/src/intrinsics.rs:8` — `pub struct FreeFnSig { params, ret }` — NO `may_block` field exists today
- `crates/ynz-typeck/src/intrinsics.rs:17-26` — `pub struct PrimitiveIntrinsicTable { print_types, free_fns, methods, methods_1arg, #[cfg(test)] test_fns }`. The `test_fns` field is `#[cfg(test)]`-gated which makes it unusable for cross-crate driver tests; that's why a new non-gated `internal_fns` field is needed.
- `crates/ynz-typeck/src/intrinsics.rs:155-160` — `lookup_free_fn` — model for `lookup_free_fn_including_internal`
- `crates/ynz-typeck/src/signatures.rs:14` — `pub struct FunctionSig` — extension site for `contains_wait: bool` field
- `crates/ynz-typeck/src/signatures.rs:48` — `pub fn collect_signatures` — population site for `contains_wait` flag (during the AST walk)
- `crates/ynz-lsp/tests/completion.rs` — LSP completion test file (visibility test for `__testFallibleAsync` goes here)
- `registry/features.toml:625-636` — `sleepMs` entry — model for `sleepAsync` entry
**Files (expected scope)**:
- `crates/ynz-typeck/src/intrinsics.rs` — add `M2_MAY_BLOCK_INTRINSICS` const set; add `internal_fns: Vec<(&'static str, FreeFnSig)>` field + `lookup_free_fn_including_internal` helper; register `__testFallibleAsync` via `internal_fns`
- `crates/ynz-typeck/src/check.rs` — add `sleepAsync` typeck dispatch arm at `:1453` (parallel to `sleepMs`); add `wait_on_non_may_block_warning` + `wait_on_non_call_expression` + `unawaited_sleep_async` + `wait_required_on_state_machine_call` diagnostics
- `crates/ynz-typeck/src/signatures.rs` — add `contains_wait: bool` field to `FunctionSig`; populate in `collect_signatures` via AST walk
- `crates/ynz-typeck/tests/check.rs` — new tests
- `crates/ynz-lsp/tests/completion.rs` — visibility test: `sleepAsync` present, `__testFallibleAsync` absent in autocomplete
**Steps**:
1. **Register `sleepAsync(int) -> nothing`** in `registry/features.toml` as a new `[[primitive_intrinsic]]` entry (TOML schema in `### Feature Registry Entries`). This auto-registers in `intrinsics.rs::free_fns` via the existing `build_table` flow.
2. **Add `sleepAsync` typeck check dispatch arm** at `check.rs:1453` parallel to the existing `sleepMs` arm. Calls a new `check_sleep_async_call(call)` helper that validates `(int)` argument shape and emits `unawaited_sleep_async` when called without `wait`. Pattern mirrors `check_sleep_ms_call` exactly (different name + different warning emission).
3. **Add `M2_MAY_BLOCK_INTRINSICS` const set** in `intrinsics.rs`:
   ```rust
   /// Set of free-function intrinsic names whose calls may suspend (block) the calling state machine.
   ///
   /// v0.3-M2 has only two such intrinsics. v0.3-M3 will replace this with a transitive analysis
   /// pass on the call graph — at that point this const + is_may_block_callee can be deleted.
   ///
   /// WHY in-code const rather than registry schema field: a 2-element list does not justify a
   /// FreeFnSig field + registry schema migration. The cost of the deferral is named: every new
   /// may-block intrinsic added before M3 must edit this const explicitly (caught by code review).
   pub const M2_MAY_BLOCK_INTRINSICS: &[&str] = &["sleepAsync", "__testFallibleAsync"];
   pub fn is_may_block_callee(callee_name: &str, callee_contains_wait: bool) -> bool {
       M2_MAY_BLOCK_INTRINSICS.contains(&callee_name) || callee_contains_wait
   }
   ```
   No registry schema change; no `FreeFnSig` field addition. M3 will promote this to a transitive analysis pass. Inline rationale per Round 3 Concern — the next reader sees the decision in code, not just in this plan.
4. **Add `internal_fns` field + helper** to `PrimitiveIntrinsicTable`:
   ```rust
   pub struct PrimitiveIntrinsicTable {
       // ... existing fields ...
       /// Internal-only intrinsics (e.g., __testFallibleAsync) — not surfaced in LSP completion,
       /// not registered in registry/features.toml, only callable via lookup_free_fn_including_internal.
       internal_fns: Vec<(&'static str, FreeFnSig)>,  // NOT cfg(test) — production typeck needs access
   }
   impl PrimitiveIntrinsicTable {
       /// Lookup including internal intrinsics. USAGE GUARD: only ever called from M2 state-machine
       /// test fixtures or callees registered in M2_MAY_BLOCK_INTRINSICS. Production user-code paths
       /// should call `lookup_free_fn` (which excludes `internal_fns`), not this helper.
       #[doc(hidden)]
       pub fn lookup_free_fn_including_internal(&self, name: &str, arg_count: usize) -> Option<&FreeFnSig> {
           self.lookup_free_fn(name, arg_count)
               .or_else(|| self.internal_fns.iter()
                   .find(|(n, sig)| *n == name && sig.params.len() == arg_count)
                   .map(|(_, sig)| sig))
       }
       // free_fn_names() unchanged — does NOT include internal_fns, so LSP completion doesn't surface them.
   }
   ```
   `#[doc(hidden)]` + USAGE GUARD comment per Round 3 Concern. Register `__testFallibleAsync` via a new additive constructor helper `with_m2_internals(self) -> Self` (LOCKED per Round 4 Required Fix #7 — does NOT modify the existing `m1()` / `m2()` / ... builders at `crates/ynz-typeck/src/intrinsics.rs:140-152`; pure-additive method preserves M1's builder behavior and avoids touching call sites in production code).
5. **Function-contains-wait flag**: add `contains_wait: bool` to `FunctionSig` at `signatures.rs:14`. Populate during `collect_signatures` at `:48` via an AST-walk helper `body_contains_wait(body: &Block) -> bool` (recursive). Initialized to `false` for functions without `wait`.
6. **`wait_on_non_call_expression` error**: at `check.rs` `Expr::Wait(inner, span)` handling, when `inner` is not `Call`/`MethodCall`/`FreeFnCall`, emit the error per canonical text in `### Feature Registry Entries`. **Note on `wait background X()` (Round 3 Required Fix #7)**: per M1's parser, `Expr::Background` is a STATEMENT-position construct (verified at `crates/ynz-parser/src/parser.rs` — `background` parses as a statement via `parse_background_stmt`, NOT as an expression). Therefore `wait background X()` is a PARSER error (parsing breaks at the `background` keyword inside the `wait` expression — `wait` expects a call expression, gets a statement-position keyword). The `wait_on_non_call_expression` typeck error is unreachable for this case because parsing fails first. Test fixture `wait_of_background_rejected.ynz` triggers a parser error with the standard "unexpected token" diagnostic. The `inside_wait + inside_background` two-flag state space described in Step 8 has 4 corners, but the (true, true) corner is unreachable due to parser-level rejection — documented as "(true, true): unreachable per parser; not handled by typeck."
7. **`wait_on_non_may_block_warning` warning**: when `Expr::Wait(inner, span)` and `inner` IS a call but `is_may_block_callee(callee_name, sig.contains_wait)` returns false, emit the warning. Predicate spec per `### Performance` "M2 may-block predicate" subsection.
8. **`wait_required_on_state_machine_call` warning (per Round 2 reclassification)**: at `check.rs` `Expr::Call` handling, check if (a) calling function's `contains_wait == true` AND (b) callee's `contains_wait == true` (per the function-table lookup) AND (c) call is NOT inside an `Expr::Wait` parent AND (d) call is NOT the immediate inner of `Expr::Background` (per Round 2 Required Fix #2 — `background sm_fn()` from inside a state machine is a legal route-to-I/O-pool pattern). Emit the warning per canonical text. Parent-AST-node check: pass `inside_wait: bool` AND `inside_background: bool` parameters down the type-checker recursion (set to true on `Expr::Wait`/`Expr::Background` entry, false elsewhere). Reclassified Round 2: this is a TEACHING warning (guides users toward writing `wait` for perf), not a CORRECTNESS error — the runtime is panic-safe via `ynz_rt_call_state_machine_sync` (P1 Step 4) so users CAN ignore the warning and the program runs correctly (just suboptimally). M3's auto-`wait` insertion eliminates the warning entirely.
9. **`unawaited_sleep_async` warning**: when a `Call` expression with callee `"sleepAsync"` is NOT wrapped in `Expr::Wait`, emit the warning per canonical text.
10. **LSP visibility test** in `crates/ynz-lsp/tests/completion.rs`: open a buffer at a position where free-fn completions are offered, assert `sleepAsync` IS in the list, assert `__testFallibleAsync` is NOT in the list. Validates the `internal_fns` boundary.
11. **Carry-forward tests**: M1 tests for `wait` kernel-mode rejection, `background` lend-rejection, etc. — all should pass unchanged. Verified by running the existing test suite.
12. **New `sleepAsync` kernel-mode rejection**: M1 ships kernel-mode rejection of the `wait` keyword. Extend the check: if a `sleepAsync(ms)` call appears in `--kernel` mode (regardless of `wait`-wrapping), emit a kernel-mode rejection error (because no Tokio runtime in kernel mode). Test: `kernel_mode_rejects_sleep_async`.
13. **Transitive-no-wait fixture (per plan-reviewer Required Fix #6)**: add fixture `transitive_no_wait_does_not_trigger_warning.ynz` — function `foo()` calls `bar()` where `bar()` internally calls `sleepAsync(100)` without `wait`. The user-level call `wait foo()` should fire `wait_on_non_may_block_warning` (because `foo.contains_wait == false` and `foo` is not in `M2_MAY_BLOCK_INTRINSICS`). Asserts M2's local-predicate behavior — M3 will swap the predicate and this fixture's expected output flips, providing a tracking checkpoint for the M2→M3 transition.

**Acceptance criteria**:
- [x] `sleepAsync(int) -> nothing` registered in `registry/features.toml` as `[[primitive_intrinsic]]`; `cargo build -p ynz-registry` passes
  - Evidence: `registry/features.toml` new `[[primitive_intrinsic]]` (name=sleepAsync, kind=free_fn, param_types=["int"], return_type="nothing", since="v0.3-M2"); `cargo test --workspace` (106 pass) includes the registry build as prerequisite.
- [x] `sleepAsync` typeck dispatch arm added at `check.rs:1453` parallel to `sleepMs`
  - Evidence: `check.rs` `"sleepAsync"` arm in `match callee_name.as_str()` → kernel rejection + `check_sleep_async_call` + `unawaited_sleep_async`; `wait_sleep_async_is_clean` test passes.
- [x] `M2_MAY_BLOCK_INTRINSICS` const + `is_may_block_callee` helper in `intrinsics.rs`
  - Evidence: `intrinsics.rs` `pub const M2_MAY_BLOCK_INTRINSICS = &["sleepAsync","__testFallibleAsync"]` (`// CARVE-OUT:` annotated) + `pub fn is_may_block_callee`; wired at `check.rs` user-fn warning path.
- [x] `internal_fns: Vec<(&'static str, FreeFnSig)>` field added to `PrimitiveIntrinsicTable`; `lookup_free_fn_including_internal` helper added; `__testFallibleAsync(bool) -> int errors` registered via `internal_fns` (NOT in registry, NOT in `free_fn_names()`)
  - Evidence: `intrinsics.rs` `internal_fns` field + `with_m2_internals()` registering `__testFallibleAsync` (`int errors`) + `lookup_free_fn_including_internal` (`#[doc(hidden)]` + USAGE GUARD); `free_fn_names()` excludes it. **Production-callable** via `queries.rs:164` `m6().with_m2_internals()` (fix-round-2 — closed the round-1 WEAK); tests `wait_test_fallible_async_{true,false}_is_clean` + `_zero_args_gives_real_arity_error` pass.
- [x] `FunctionSig.contains_wait: bool` field added at `signatures.rs:14`; populated in `collect_signatures` at `:48`
  - Evidence: `signatures.rs` `pub contains_wait: bool` + `contains_wait: body_contains_wait(&f.body)`; recursive `body_contains_wait` walks all Stmt/Expr variants.
- [x] `wait_on_non_call_expression` error fires correctly; integration test (`wait_of_wait_rejected`, `wait_of_literal_rejected`)
  - Evidence: `check.rs` `is_call` guard at `Expr::Wait` → error "must be followed by a function call"; tests `wait_on_literal_is_an_error` + `wait_of_wait_rejected` pass.
- [x] `wait_on_non_may_block_warning` warning fires correctly; integration test for `wait print("x")`
  - Evidence: `check.rs` intrinsic fast-path (print/range/sleepMs/sensitive) + user-fn path via `is_may_block_callee`; `wait_on_non_may_block_print_warns` passes; fix-round-2 `wait_on_non_may_block_does_not_warn_on_nested_arg_call` (no false positive on args) mutation-verified.
- [x] `wait_required_on_state_machine_call` warning fires when state-machine fn calls state-machine fn without `wait` AND without `background`; integration test
  - Evidence: `check.rs` 4-condition gate (`!was_inside_wait && !was_inside_background && current_fn_contains_wait && callee_contains_wait`); test `state_machine_calling_state_machine_without_wait_warns` passes. (This is the warning DEFERRED from Phase 2 — landed here.)
- [x] `state_machine_can_background_state_machine_without_wait` test passes — `background sm_fn()` from inside another state machine compiles clean (no false positive)
  - Evidence: `inside_background` set on `Expr::Background` entry → exempts the immediate inner; test `state_machine_can_background_state_machine_without_wait` passes; fix-round-2 `background_arg_state_machine_call_still_warns` confirms the exemption does NOT leak to argument calls.
- [x] `unawaited_sleep_async` warning fires when `sleepAsync(100)` appears without `wait`; integration test
  - Evidence: `check.rs` `if !was_inside_wait` in the sleepAsync arm; tests `unawaited_sleep_async_warns` + fix-round-2 `unawaited_sleep_async_fires_on_arg_of_waited_call` pass.
- [x] LSP completion test in `crates/ynz-lsp/tests/completion.rs`: `sleepAsync` visible; `__testFallibleAsync` NOT visible
  - Evidence: `sleep_async_visible_test_fallible_async_not_visible` asserts both sides; passes (lsp 22/22). `with_m2_internals()` in the check table does NOT leak into completion (`free_fn_names()` excludes `internal_fns`).
- [x] M1 tests carry forward unchanged (kernel-mode, lend-rejection, etc.)
  - Evidence: typeck 185 pass / 0 fail (incl. `wait_in_kernel_mode_rejected`); workspace 106 pass / 5 environmental-only.
- [x] `sleepAsync` kernel-mode rejection fires; new test `kernel_mode_rejects_sleep_async`
  - Evidence: `check.rs` kernel-mode guard in the sleepAsync arm (WHAT/WHAT-INSTEAD/WHY, refs no-runtime-mode); test `kernel_mode_rejects_sleep_async` passes.
- [x] `transitive_no_wait_does_not_trigger_warning.ynz` fixture passes with the M2 local predicate (M3 transition checkpoint)
  - Evidence: inline test `transitive_no_wait_does_not_trigger_wait_required_warning` passes — `bar()` calls `sleepAsync` w/o wait so `bar.contains_wait==false`; `wait foo()` warns non-may-block but NOT wait_required (locks M2 LOCAL predicate; flips in M3). (Plan said `.ynz` fixture; inline test = equivalent coverage — accepted by plan-adherence.)
- [x] Banned-jargon audit passes (no async/await/coroutine/task/Future/Promise in new diagnostic text)
  - Evidence: `cargo test -p ynz-diagnostics --test jargon_audit` 4/4 pass; new diagnostic text uses plain Yinz vocabulary ("the awaited expression" is plain English, not the banned token).

**Quality gate**:
- [ ] All new diagnostics use WHAT/WHAT-INSTEAD/WHY format with contextual WHY
- [ ] `internal_fns: Vec<(&'static str, FreeFnSig)>` field on `PrimitiveIntrinsicTable` is excluded from `free_fn_names()` (LSP autocomplete filter); `lookup_free_fn_including_internal` is `#[doc(hidden)]` and gated by USAGE GUARD comment per Round 4 Required Fix #5
- [ ] Tests cover happy path AND unhappy path for each new check
- [ ] No `// TODO` comments left in the typeck additions
- [ ] Diagnostic text uses Yinz vocabulary per `.claude/rules/vocabulary.md` (no banned jargon)

**Verification**:
- `cargo test --workspace` passes
- Manual: write a `.ynz` file with `wait print("hi")` — confirm Tier 3 warning. Write `wait 42` — confirm error.

**Phase Review Gates** (filled at phase completion):
- [x] code-reviewer: PASS 2026-05-31T (review round 2, after fix-loop round 2) — round-1 BLOCK (Fix #1 arg-flag-leak + Fix #2 with_m2_internals) both resolved; 5 fresh adversarial inputs (wait-in-arg, 3-level nesting, call-itself non-regression, kernel early-return sibling) + 3 mutation-verified guard tests all pass. 2 non-blocking concerns noted for M3 (DRY-at-2; `M2_MAY_BLOCK_INTRINSICS` duplicated in intrinsics.rs + emit.rs — unify in M3 call-graph rewrite). NOTE: reviewer's stray `git checkout` reverted+reconstructed the uncommitted tree; coordinator verified the working tree is BYTE-IDENTICAL to the reviewed snapshot before commit.
- [x] rules-compliance-reviewer: PASS 2026-05-31T (review round 2, file-based diff) — 4 diagnostics WHAT/WHAT-INSTEAD/WHY; no banned jargon; `sleepAsync` registry entry + `M2_MAY_BLOCK_INTRINSICS` carve-out; no test-weakening. (Round-1 corrected/file-based after two cwd-footgun runs reviewed a phantom rollback.)
- [x] plan-adherence-verifier: PASS 2026-05-31T (review round 2b) — all 13 Steps MET; D1 (hover.rs) + queries.rs (Fix B) documented in-scope/necessary touches; D2 (struct-field flags) DEVIATED-WITH-REASON; all deviation rationales free of banned phrases.
- [x] acceptance-verifier: PASS 2026-05-31T (review round 2) — OVERALL PASS, all 15 ACs MET (round-1 WEAK AC4 resolved by `queries.rs:164` `with_m2_internals()`; 3 `__testFallibleAsync` tests pass); 106/106 non-environmental tests green.
- [x] deviation-judge #1 (approach: Checker struct-field flags vs recursion params): PASS 2026-05-31T (round 1) — traced all set/restore paths; no leak the param approach wouldn't also have; the arg-leak bug code-reviewer found was a logic bug (fixed in Fix A), NOT deviation-attributable.
- [x] Committed: 5b72521

**Findings Log** (filled during any fix loops):

**2026-05-31 — Phase 3 executor DONE (base = Phase 2 commit 5109b52), gate in flight.** Added (verified what Phase 2 already landed, didn't duplicate): `sleepAsync` registry `[[primitive_intrinsic]]`; `M2_MAY_BLOCK_INTRINSICS` + `is_may_block_callee`; `internal_fns` field + `with_m2_internals()` + `lookup_free_fn_including_internal` (#[doc(hidden)]) registering `__testFallibleAsync` (NOT in registry, NOT in free_fn_names); `FunctionSig.contains_wait` + `body_contains_wait` recursive walk; the 4 diagnostics (`wait_on_non_call_expression` error; `wait_on_non_may_block_warning`, `unawaited_sleep_async`, `wait_required_on_state_machine_call` warnings — the last deferred from Phase 2 lands here); LSP visibility test; kernel-mode `sleepAsync` rejection; transitive-no-wait fixture. Verified: typeck 179 pass, lsp 22 pass, jargon_audit 4/4, `cargo test --workspace` 106 pass / 5 environmental-only; manual diagnostics all fire correctly. Diagnostic text uses "the awaited expression" (plain English, plan canonical) — not a banned token.

**Deviations:** D1 (scope) — touched `crates/ynz-lsp/tests/hover.rs` (+3) for compile-required `contains_wait: false` on existing `FunctionSig` literals; mechanical. D2 (approach) — `Checker` struct fields (set-before-recurse/restore-after) instead of recursion params (~50 call sites; matches existing `kernel_mode` pattern); real flag-leak adversarial surface → deviation-judge #1 dispatched.

**Executor-flagged P5 concern:** `check_query` (production salsa path) uses `PrimitiveIntrinsicTable::m6()`, NOT `with_m2_internals()` — so `__testFallibleAsync` is TEST-callable but NOT production-callable yet. Per the registry-without-typeck-dispatch failure mode this is "registered but no production dispatch." Plan defers driver-fixture wiring to P5. **COORDINATOR TODO for P5:** wire `with_m2_internals()` into the query path (or the errors-cascade-through-state-machine integration test will find `__testFallibleAsync` "not defined").

**Gate dispatched (review round 1):** code-reviewer a48785d9, rules-compliance adb61e60, plan-adherence a5d8a041, acceptance ac50ab99, deviation-judge(D2) aee9af3a. All read materialized diff `/tmp/phase3_real.diff`.

**GATE ROUND 1 RESULT (2026-05-31): BLOCK.** rules-compliance PASS; plan-adherence PASS; deviation-judge(D2) PASS (struct-field-flag approach is sound — traced all set/restore paths, no leak the param approach wouldn't also have). code-reviewer BLOCK + acceptance BLOCK (convergent) → 2 fixes:
- **Fix A (code-reviewer Fix #1, CONFIRMED via production driver):** `inside_wait` (and symmetrically `inside_background`) leaks into the AWAITED CALL'S ARGUMENT sub-expressions. `wait inner(sleepMs(10))` → spurious `wait_on_non_may_block` warning on `sleepMs` ("remove the `wait`" — but there's no wait on it). Dual symptom (judge #4): `wait print(sleepAsync(100))` wrongly SUPPRESSES `unawaited_sleep_async` on the arg. Root: `Expr::Wait` sets `inside_wait=true`, then `check_call` recurses into `call.args` with it still set. NOT deviation-attributable (same under param-passing) — a logic bug in flag scope. FIX: in `check_call`, before recursing into `call.args`, save+clear BOTH `inside_wait` and `inside_background` (set false), restore after — neither keyword applies to argument calls. + tests for `wait inner(sleepMs(10))` (no false warning), `wait print(sleepAsync(100))` (correct unawaited warning), and `background foo(sm_bar())` (sm_bar still warns wait_required).
- **Fix B (code-reviewer Fix #2 == acceptance AC4 WEAK, CONFIRMED):** `crates/ynz-typeck/src/queries.rs:164` builds the production table via `PrimitiveIntrinsicTable::m6()` with NO `.with_m2_internals()` → `internal_fns` empty in `check_query` → `wait __testFallibleAsync(true)` hits the literal-name dispatch arm, `lookup_free_fn_including_internal` returns None, the `None⟹wrong-arity` else branch fires "takes 1 argument, got 1". Makes plan QR#8 ("internal_fns gives production typeck access") false as wired; P5's errors-cascade test would hit the same dead arm. FIX: chain `.with_m2_internals()` at the production table-construction site(s) (grep all `m6()` construction in production paths) + production-path test that `wait __testFallibleAsync(true)` resolves cleanly (int return, no arity error). Closes the COORDINATOR P5 TODO above.
- Cosmetic (fold in): drop the useless `format!` of a literal on the user-fn `wait_on_non_may_block` warning.

**FIX-LOOP ROUND 2 DONE (2026-05-31), gate re-running.** Executor applied Fix A (save+clear both flags around `check_call` arg recursion; warning decisions use saved pre-clear values), Fix B (`queries.rs:164` → `m6().with_m2_internals()`), Fix C (format!→plain string). Zero deviations. Coordinator-verified through the production compiler: `wait inner(sleepMs(10))` → 0 false `sleepMs` warnings (was the bug); `wait __testFallibleAsync(true)` resolves as `int errors` (no arity/not-defined error — the `exit=1` on an ad-hoc test was a correct "errors value needs .failed() first" teaching error, proving production-callability); `wait __testFallibleAsync()` → real arity error. typeck 185 pass (incl. 5 new fix tests: `wait_on_non_may_block_does_not_warn_on_nested_arg_call`, `unawaited_sleep_async_fires_on_arg_of_waited_call`, `background_arg_state_machine_call_still_warns`, `wait_test_fallible_async_{true,false}_is_clean`, `_zero_args_gives_real_arity_error`); lsp 22 pass (visibility test holds); workspace 106 pass / 5 environmental. **Gate round 2 dispatched:** code-reviewer af3de9f1, rules-compliance a6877e76, plan-adherence ab5cd8bb, acceptance ac6f3133 (no new deviations → no judge). Read `/tmp/phase3_real.diff` (1294 lines). Awaiting verdicts → on all-PASS: write AC ticks + gates, commit Phase 3 (stage only Phase-3 files), proceed to Phase 4.

**Exit Sequence**: per template.

---

### Phase 4: Teaching Surface — Registry, LSP, VSCode, Design-Doc Deferral Notes

**PR scope**: Ship all teaching surfaces required by the roadmap constraint: (1) registry `wait`/`background` hover updates (M1's forward-pointing text replaced with M2's real semantics); (2) `sleepAsync` primitive_intrinsic entry; (3) diagnostic_template entries for new errors/warnings; (4) LSP diagnostic flow validation; (5) VSCode extension bump + `wait-suspension.png` screenshot; (6) design-doc deferral notes in `filesystem.md` / `network.md` / `database.md` for the v0.5+ async I/O surface.
**Branch**: `feat/v0-3-m2-teaching-surface`
**Flag**: N/A
**Est. lines**: ~300 (mostly TOML + design-doc text + screenshot)
**Ships via**: `/pr`
**Objective**: Per the roadmap constraint "Full teaching surface ships in the same milestone as the feature — no exceptions." Without these, the M2 feature is invisible in the editor / docs until someone remembers to fix it later.
**Why this phase exists**: This is the load-bearing teaching commitment. Skipping any of these surfaces ships M2 as a hidden feature.
**Current-state anchors**:
- `registry/features.toml:166` (`wait` keyword), `:170` (`background` keyword) — hover doc update sites
- `registry/features.toml` — `[[diagnostic_template]]` section (M1 added 4; M2 adds 3)
- `registry/features.toml` — `[[primitive_intrinsic]]` section (M2 adds `sleepAsync`)
- `crates/ynz-lsp/src/diagnostics.rs` — diagnostic flow (existing infrastructure)
- `tooling/vscode-ynz/package.json` — version bump
- `design/stdlib/filesystem.md`, `network.md`, `database.md` — deferral note sites
**Files (expected scope)**:
- `registry/features.toml` — 2 keyword hover updates + 1 primitive_intrinsic entry + 3 diagnostic_template entries
- `crates/ynz-registry/build.rs` — (no change expected; schema already supports needed fields from M1)
- `crates/ynz-lsp/tests/diagnostics.rs` — new test for M2 diagnostic flow
- `tooling/vscode-ynz/package.json` — version → `0.3.0-m2`
- `tooling/vscode-ynz/CHANGELOG.md` — new entry
- `tooling/vscode-ynz/README.md` — mention v0.3-M2 capability
- `tooling/vscode-ynz/screenshots/wait-suspension.png` — new screenshot
- `design/stdlib/filesystem.md` — deferred-tooling-feature stub for `readFileAsync`/`writeFileAsync`
- `design/stdlib/network.md` — deferred stub for async network intrinsics
- `design/stdlib/database.md` — deferred stub for async DB intrinsics
**Steps**:
1. Update `registry/features.toml:166` (`[[keyword]] wait`): replace `hover_what`/`hover_what_instead`/`hover_why` with canonical M2 text from `### Feature Registry Entries`.
2. Update `registry/features.toml:170` (`[[keyword]] background`): add routing-distinction note to `hover_why` (canonical text in `### Feature Registry Entries`).
3. Add `[[primitive_intrinsic]]` entry for `sleepAsync` to `registry/features.toml`. Schema (verified against `registry/features.toml:625-636` `sleepMs` entry):
   ```toml
   # --- free_fn: sleepAsync (v0.3-M2) ---
   # Non-blocking sleep. `wait sleepAsync(ms)` suspends the calling function for ms milliseconds.
   # OS thread is freed during the suspension. Pairs with sleepMs (blocking sleep, v0.3-M1).
   # Lowers to ynz_rt_async_sleep_create + ynz_rt_async_sleep_poll in libynz_runtime.a.
   [[primitive_intrinsic]]
   name = "sleepAsync"
   kind = "free_fn"
   param_types = ["int"]
   return_type = "nothing"
   since = "v0.3-M2"
   ```
   NOTE: the registry schema does NOT have a `may_block` field; the may-block property lives in the in-code `M2_MAY_BLOCK_INTRINSICS` const set added in P3 Step 3 (registry schema change deferred to M3 when transitive analysis ships and the field becomes useful for more than 2 intrinsics).
4. Add 4 new `[[diagnostic_template]]` entries with canonical text from `### Feature Registry Entries`: `wait_on_non_may_block_warning`, `wait_on_non_call_expression`, `unawaited_sleep_async`, `wait_required_on_state_machine_call`.
4b. Add `[[deferred_tooling_feature]]` entry `async-io-stdlib-intrinsics-v0-5` to `registry/features.toml` with canonical text from `### Feature Registry Entries`. WHY/SUBSTITUTE/SHIPS_IN/DESIGN_DOC fields per the registry's existing schema for deferred features. Replaces the per-design-doc prose-stub approach with a real SSOT registry entry, per `.claude/rules/feature-registry.md`.
5. Run `cargo build -p ynz-registry` — confirm TOML parses + generated Rust constants compile + no schema-mismatch errors.
6. LSP diagnostic flow test: add a test case in `crates/ynz-lsp/tests/diagnostics.rs` that opens a buffer with `wait print("hi")` and asserts the LSP returns the `wait_on_non_may_block_warning` diagnostic with correct severity (Warning, not Error).
7. Hover lookups: add unit tests confirming `lsp_hover_for_token("wait")` returns the new M2 text (containing "Suspends the calling function") and `lsp_hover_for_token("background")` returns the updated routing-distinction text.
8. VSCode extension: bump version to `0.3.0-m2` in `package.json`. Update `CHANGELOG.md` with M2 summary (wait suspension shipped, sleepAsync added, two new warnings). Record a screenshot showing: hover over `wait` in VSCode → new hover text appears; trigger `wait_on_non_may_block_warning` in a `.ynz` file → see the squiggle. Save as `screenshots/wait-suspension.png`.
9. Add deferral cross-reference notes in design docs (NOT the SSOT — the SSOT is the `async-io-stdlib-intrinsics-v0-5` registry entry added in Step 4b):
   - `design/stdlib/filesystem.md` — add a "v0.5+ Async I/O Surface" section that points to the `async-io-stdlib-intrinsics-v0-5` deferred-tooling-feature registry entry for the canonical deferral spec; mentions: `readFileAsync(path) -> string errors`, `writeFileAsync(path, content) -> nothing errors`, etc.; states these ship with v0.5 file stdlib module; backed by `tokio::fs`; v0.3-M2 validated errors-through-state-machine via internal `__testFallibleAsync` so v0.5 inherits a working ABI.
   - `design/stdlib/network.md` — analogous cross-reference for async network primitives (v0.15+ http module per roadmap).
   - `design/stdlib/database.md` — analogous cross-reference for async DB primitives (v0.10 db module per roadmap).
10. Build a fresh `.vsix`: `cd tooling/vscode-ynz && vsce package`. Verify two files produced: `yinz-0.3.0-m2.vsix` AND `yinz-latest.vsix` (per project convention). NOTE: matches M1 phase 5 convention; vsce package may be deferred to release time if headless env doesn't have vsce; mark as such in acceptance criteria.

**Acceptance criteria**:
- [x] Registry: `wait` and `background` keyword hover docs updated with M2 text
  - Evidence: `registry/features.toml` `[[keyword]] wait` hover_what="Suspends the calling function..."; `background` hover_why gets the I/O-pool/blocking-pool routing-distinction note. LSP tests `hover_wait_keyword_returns_m2_suspension_text` (+ negative assert on old M1 text) + `hover_background_keyword_returns_routing_distinction_text` pass.
- [x] Registry: `sleepAsync` primitive_intrinsic entry added
  - Evidence: present from Phase 3 (verified not duplicated); `cargo build -p ynz-registry` green.
- [x] Registry: 4 new diagnostic_template entries added (wait_on_non_may_block_warning, wait_on_non_call_expression, unawaited_sleep_async, wait_required_on_state_machine_call)
  - Evidence: `registry/features.toml` `[[diagnostic_template]]` `WaitOnNonMayBlockWarning`/`WaitOnNonCallExpression`/`UnawaitedSleepAsync`/`WaitRequiredOnStateMachineCall`; `diagnostic_templates_have_all_three_parts` consistency test passes.
- [x] Registry: `async-io-stdlib-intrinsics-v0-5` deferred_tooling_feature entry added
  - Evidence: `registry/features.toml` `[[deferred_tooling_feature]] name="async-io-stdlib-intrinsics-v0-5"` with substitute/why/ships_in/design_doc/triggers; `deferred_tooling_features_have_required_fields` passes. (design_doc single-path filesystem.md per the registry `path.exists()` test constraint; network/db cross-refs in `triggers` — approved deviation.)
- [x] `cargo build -p ynz-registry` succeeds (TOML parses + Rust compiles)
  - Evidence: `Finished dev profile` clean; 26 ynz-registry tests pass.
- [x] LSP diagnostic test: `wait print("hi")` returns `wait_on_non_may_block_warning` at expected span
  - Evidence: `crates/ynz-lsp/tests/diagnostics.rs:wait_on_non_may_block_warning_flows_as_lsp_warning` — asserts Warning severity + WHY + `range.start.line==2` + `character>=9` (span assert added fix-round-2 per AC6; non-vacuous — fails on wrong-line/zero-range).
- [x] LSP hover test: `wait` and `background` hovers return updated M2 text
  - Evidence: the 2 hover tests above pass; hardened with `let HoverContents::Markup(mc) = ... else { panic!() }` (fix-round-2, kills the vacuity trap).
- [x] VSCode extension: `package.json` version bumped to `0.3.0-m2`; CHANGELOG updated; `wait-suspension.png` screenshot present (placeholder OK if vsce headless-unavailable — real screenshot at release time)
  - Evidence: `package.json` 0.3.0-m2; CHANGELOG `[0.3.0-m2]` section; README "What's new"; `screenshots/wait-suspension.png.PLACEHOLDER` (headless — AC-allowed).
- [x] `design/stdlib/filesystem.md` has v0.5+ async-I/O deferral section
  - Evidence: "## v0.5+ Async I/O Surface" section cross-referencing the registry SSOT entry (readFileAsync/writeFileAsync/readBytesAsync).
- [x] `design/stdlib/network.md` has analogous deferral section
  - Evidence: "## v0.15+ Async I/O Surface" (request.getAsync/postAsync).
- [x] `design/stdlib/database.md` has analogous deferral section
  - Evidence: "## v0.10+ Async I/O Surface" (db.queryAsync/findAsync).
- [x] No banned-jargon in any new text (audited by grep — zero async/await/coroutine/Future/Promise hits in user-facing text)
  - Evidence: jargon_audit 5/5 pass incl. NEW `no_banned_jargon_in_deferred_feature_user_facing_fields` (extended fix-round-2 to scan deferred-feature rendered fields — caught + fixed 6 PRE-EXISTING violations; non-vacuous, proven by injection). standalone `async`/`lifetime` removed from all user-facing field values; "borrow-scope" overshoot corrected to canonical "scope" + `` `lifetimes` `` domain-key reference (deviation-judge round-2 PASS). design/ titles ("Async I/O Surface") are contributor-facing (allowed).
- [x] [Deferred to release] VSCode extension `.vsix` files built (both versioned and `latest`) — vsce may not be available headless
  - Evidence: deferred-to-release per AC text (no vsce headless); placeholder + CHANGELOG note in place.

**Quality gate**:
- [ ] All new hover/diagnostic text passes WHAT/WHAT-INSTEAD/WHY format check
- [ ] No banned-jargon (async/await/coroutine/Future/Promise/Tokio brand names) in user-facing text
- [ ] All registry entries follow existing schema (verified by `cargo build -p ynz-registry`)
- [ ] Design-doc deferral notes follow the `[[deferred_tooling_feature]]` pattern's spirit (WHY / SUBSTITUTE / SHIPS_IN / DESIGN_DOC) even if they're prose rather than TOML
- [ ] No SQL/security concerns (pure docs+TOML+LSP wiring)

**Verification**:
- `cargo test --workspace` passes
- `cargo test -p ynz-registry` confirms TOML parses and generated Rust compiles
- `cargo test -p ynz-lsp diagnostics` runs new diagnostic flow test
- Manual install of `yinz-latest.vsix` in VSCode + hover over `wait` shows new text (deferred to release time)

**Phase Review Gates** (filled at phase completion):
- [x] code-reviewer: PASS 2026-05-31T (review round 2, after fix-loop round 2) — empirically proved the extended jargon audit non-vacuous (injected `async` → audit failed → restored); both round-1 vacuity traps (span / hover) closed; 6 pre-existing rewrites faithful. 2 non-blocking concerns (residual "lifetimes" in non-rendered fields — fixed in the judge-driven correction; pre-existing `site_count` unused-var — out of scope, follow-up).
- [x] rules-compliance-reviewer: PASS 2026-05-31T (review round 2) — re-scanned ALL rendered fields (closing its round-1 miss of the substitute `async`); the reworded async-io entry + 6 pre-existing rewrites jargon-clean; extended audit locks the gap.
- [x] plan-adherence-verifier: PASS 2026-05-31T (review round 2) — all 10 Steps MET; the 6-entry rewrite = audit-forced in-scope cleanup (not creep), minimal jargon-only swaps; Approach-D1 (single design_doc path) + Approach-D2 (screenshot placeholder) documented; rationales clean.
- [x] acceptance-verifier: PASS 2026-05-31T (review round 2) — OVERALL PASS, all 13 ACs MET; round-1 WEAK AC6 (span) + AC12 (jargon) both resolved; workspace 106/5-environmental, zero regression.
- [x] deviation-judge #1 (scope: lsp_adapter.rs M1→M2 hover test retarget): PASS 2026-05-31T (round 1) — clean retarget, all 4 structural asserts + `.expect()` preserved, WHY comment updated.
- [x] deviation-judge #2 (scope: 6 pre-existing deferred-feature entries reworded by the audit extension): BLOCK→PASS 2026-05-31T — round-2 BLOCK caught an overshoot (invented "borrow-scope" inconsistent w/ `domain="lifetimes"`; "aliased re-export" precision lost). Coordinator-applied judge-specified fix (canonical "scope" vocabulary + `` `lifetimes` `` key reference + restored "aliased re-export"); re-judge PASS — both corrections faithful, jargon-clean, cross-entry consistency restored.
- [ ] Committed: <commit SHA>

**Findings Log** (filled during any fix loops):

**2026-05-31 — Phase 4 executor DONE (base = Phase 3 commit 5b72521), gate in flight.** Mostly TOML + docs + 2 LSP test files (378-line diff). Added: `wait`/`background` keyword hover M2 updates; 4 `[[diagnostic_template]]` entries (wait_on_non_may_block_warning, wait_on_non_call_expression, unawaited_sleep_async, wait_required_on_state_machine_call); `async-io-stdlib-intrinsics-v0-5` `[[deferred_tooling_feature]]`; LSP diagnostic-flow + 2 hover tests; VSCode package.json→0.3.0-m2 + CHANGELOG + README; 3 design-doc async-I/O deferral sections (filesystem/network/database). Already-present (verified, not duplicated): `sleepAsync` `[[primitive_intrinsic]]` (Phase 3), `WaitInsideLoop`/`LocalCrossesWait` templates (Phase 2). Coordinator-verified: registry build green, lsp tests pass, lsp_adapter.rs test = clean M1→M2 retarget (kept all structure asserts, changed only the text line + WHY comment — NOT weakening, verified directly). workspace 106 pass / 5 environmental.

**Deviations:** Scope-D1 — `crates/ynz-registry/tests/lsp_adapter.rs` touched (compile-required: M2 hover broke the M1-text assertion) → deviation-judge dispatched (test-weakening lens). Approach-D1 — `async-io-stdlib-intrinsics-v0-5` `design_doc` uses single path `design/stdlib/filesystem.md` (not the plan's 3-path string) because `every_registry_entry_design_doc_exists` test does `path.exists()` on the whole value; network/database refs moved to `triggers` prose → plan-adherence assessing SSOT-completeness. Approach-D2 — screenshot `.PLACEHOLDER` text file (AC-allows headless placeholder; no PNG fabricated). vsix build [deferred to release].

**Gate dispatched (review round 1):** code-reviewer a99ac043, rules-compliance a23b52d8 (PASS, above), plan-adherence a6e14093, acceptance a7050b52, deviation-judge(Scope-D1) adf0576f. Read `/tmp/phase4_real.diff`.

**GATE ROUND 1 RESULT (2026-05-31): BLOCK (acceptance).** code-reviewer PASS, rules-compliance PASS, plan-adherence PASS, deviation-judge(Scope-D1) PASS (clean retarget). acceptance BLOCK — 11/13 ACs MET, 2 WEAK → fix round:
- **Fix #1 (AC6 span, WEAK):** `wait_on_non_may_block_warning_flows_as_lsp_warning` asserts the warning exists + Warning severity + WHY, but NOT the span — a zero-range/wrong-line diagnostic would pass. FIX: add a range assertion (the warning must anchor to the `wait`/`print` token on the fixture's known line/col).
- **Fix #2 (AC12 banned-jargon, WEAK — caught by acceptance, MISSED by rules-compliance):** the word `async` appears as STANDALONE PROSE ("async control flow", "async I/O") in the `substitute` field of the `async-io-stdlib-intrinsics-v0-5` deferred entry, which RENDERS in user-facing LSP hover (`**Substitute:** {substitute}` via lsp_adapter). `async` is a `[[banned_jargon]]` term → real user-facing violation. FIX: reword the substitute (+ any other hover-rendered deferred-feature field — why/triggers) to plain language, keeping `sleepAsync`/`readFileAsync` API names; AND extend `crates/ynz-diagnostics/tests/jargon_audit.rs` (or wherever the audit lives) to SCAN the user-facing-rendered fields of deferred_tooling_feature entries so this class is caught mechanically (the audit currently misses them — fix the detection hole, not just the instance). design/ section TITLES ("Async I/O Surface") are contributor-facing → keep. The entry slug `async-io-stdlib-intrinsics-v0-5` is an identifier → keep (assess if it renders as user-facing prose; slug ≠ prose).
- **Fix #3 (code-reviewer Concern #1, non-blocking but cheap — folded in):** the two new hover tests nest asserts in `if let Some/Markup` without else → latent vacuity trap (silently skip asserts if hover_response ever returns non-Markup). FIX: `let HoverContents::Markup(mc) = ... else { panic!(...) }` (strict strengthening, matches test-must-exercise-claimed-path).
- Concern #2 (`{args}` vs `(...)` template placeholder drift) — LEAVE: consistent with pre-existing M4 templates + no parity test binds them; changing this one creates inconsistency. Noted.

**FIX-LOOP ROUND 2 DONE (2026-05-31), gate re-running.** Executor applied Fix #1 (span assert in diagnostics test — `range.start.line==2`, `character>=9`, non-vacuous), Fix #2 (reworded `async-io-stdlib-intrinsics-v0-5` substitute+ships_in; EXTENDED `crates/ynz-diagnostics/tests/jargon_audit.rs` to scan deferred-feature user-facing fields — which exposed + fixed 6 PRE-EXISTING jargon violations in unrelated deferred entries, e.g. `--kernel`, lsp-inlay-hint-*, background-handle-form; "lifetime"→"borrow-scope"), Fix #3 (hover tests `let-else` hardening). Coordinator-verified: span assert non-vacuous; no standalone `async` left in user-facing field VALUES (remaining hits = `[[banned_jargon]]` defs + a TOML comment); extended audit passes + was non-vacuous (7 violations pre-fix); spot-checked `lsp-inlay-hint-lifetimes` rewrite faithful. lsp + jargon_audit green; workspace 106 / 5 environmental. Diff grew 378→518 lines (the 6 pre-existing rewrites). **Gate re-dispatched (round 2):** code-reviewer a29de5af, acceptance acad279e, rules-compliance a1ed002e (re-scanning ALL rendered fields after its round-1 miss of the substitute jargon), plan-adherence a72015da, deviation-judge(6-entry-rewrite faithfulness) ae2f0ed9. Read `/tmp/phase4_real.diff`.

**GATE ROUND 2 RESULT (2026-05-31): 4 reviewers PASS; deviation-judge BLOCK → fixed → re-judging.** code-reviewer PASS (empirically proved the audit non-vacuous — injected `async`, audit failed, restored; both vacuity traps dead; flagged residual "lifetimes" in ships_in/triggers as non-blocking Concern). rules-compliance PASS (re-scanned all rendered fields, closing its round-1 miss). plan-adherence PASS (6-entry rewrite = audit-forced in-scope cleanup, not creep). acceptance PASS (all 13 ACs MET; AC6 span + AC12 jargon resolved). **deviation-judge(6-entry) BLOCK** — the audit-forced cleanup OVERSHOT in 2 entries: (1) `lsp-inlay-hint-lifetimes` invented the term "borrow-scope" (appears nowhere else, ignores the registry's own canonical "lifetime"→"scope" replacement at features.toml:553, and contradicts `[[muted_hint_domain]] domain = "lifetimes"` at features.toml:1939); (2) `lsp-rename-aliased-re-export` changed "aliased re-export"→"re-exported name" in `why`, losing precision vs the `substitute` field (the actual banned word was only bare "alias"). **COORDINATOR-APPLIED FIX (judge-specified, 5-line text correction):** lifetimes entry → "scope"/"scope annotations"/"value scopes" (canonical replacement) + references the actual `` `lifetimes` `` domain key (reconciles with line 1939) + cleared residual plurals (also closes code-reviewer's Concern #1); rename entry → restored "aliased re-export" in `why`. Verified: "borrow-scope" 0 hits repo-wide; jargon_audit 5/5 pass; registry build green; diff 523 lines. **Re-judge dispatched: ad7c3943** (faithfulness re-check of the 2 corrected entries; 4 reviewers not re-run — fix is judge-specified text-only on 2 registry entries, doesn't touch their lanes). On re-judge PASS → write AC ticks + gates, commit Phase 4, proceed to Phase 5 (final).

**Exit Sequence**: per template.

---

### Phase 5: Demo + Error Gallery + Cross-Impl Harness Extension + Release Prep

**PR scope**: Extend `pirates-roster/entrypoint.ynz` with the v0.3-M2 concurrency-from-state-machines section. Create `primantis-orders/v0_3_m2_errors.ynz`. Extend the cross-impl consistency harness to cover M2 state-machine fixtures (with the timing-dependent exclusion list updated). Add the milestone wrap-up: Cargo.toml bump, CHANGELOG, tag.
**Branch**: `feat/v0-3-m2-demo-and-release`
**Flag**: N/A
**Est. lines**: ~300 (mostly demo Yinz code + CHANGELOG + state.md entry)
**Ships via**: `/pr` for the PR + `/release` for the `v0.3.0-m2` tag (per VSCode extension release convention: attach both `yinz-0.3.0-m2.vsix` and `yinz-latest.vsix`)
**Objective**: Verify the milestone holistically — state-machine concurrency works in real demo code; error gallery covers all new diagnostic classes; cross-impl harness passes; cut the milestone tag.
**Why this phase exists**: The earlier phases each focus on a layer. P5 is the cross-layer verification + release gate AND the hands-on UX validation per `.claude/rules/plan-invariants.md`.
**Current-state anchors**:
- `examples/pirates-roster/entrypoint.ynz:226` (M8 modules section end — insertion point for M1's v0.3-M1 section, just after which M2's section appends)
- `examples/primantis-orders/v0_3_m1_errors.ynz` — model for the v0_3_m2_errors.ynz file
- `crates/ynz-driver/tests/cross_impl_consistency.rs` (M1) — extension site for new state-machine fixtures
- `Cargo.toml` (workspace) — version field for bump
**Files (expected scope)**:
- `examples/pirates-roster/entrypoint.ynz` — append v0.3-M2 state-machine section
- `examples/primantis-orders/v0_3_m2_errors.ynz` — new error gallery file
- `examples/primantis-orders/README.md` — link new file
- `crates/ynz-driver/tests/cross_impl_consistency.rs` — extend timing-dependent fixture allowlist; add M2 fixtures
- `crates/ynz-driver/tests/m2_state_machine_integration.rs` — new file with full integration tests
- `Cargo.toml` (workspace) — version `0.3.0-m1` → `0.3.0-m2`
- `CHANGELOG.md` — new section
- `.claude/state.md` — append milestone-ship entry
**Steps**:
1. **Extend `examples/pirates-roster/entrypoint.ynz`** with a v0.3-M2 section after the existing v0.3-M1 section. **8 pirates** (per Round 2 Concern #5 — 8 demonstrates thread-sharing visibly on multi-core CI; 4 doesn't):
   ```yinz
   // ────── v0.3-M2: wait actually suspends ──────
   //
   // Below: 8 background tasks each `wait sleepAsync(100)`. M1's blocking
   // sleepMs would total 800ms wall-clock; M2's state-machine suspension
   // makes them share threads — total ~100ms even on a 4-core machine.
   //
   // Hover the `wait` keyword in VSCode to see the v0.3-M2 hover doc.
   print(``)
   print(`v0.3-M2 — wait suspends without holding a thread:`)
   background pausePirate(1)
   background pausePirate(2)
   background pausePirate(3)
   background pausePirate(4)
   background pausePirate(5)
   background pausePirate(6)
   background pausePirate(7)
   background pausePirate(8)
   print(`scheduled 8 pirates`)
   // Per Round 3 Required Fix #8: ynz_rt_shutdown is configured with
   // a 5s drain timeout (per M1's runtime.rs), so backgrounds will complete
   // before exit even without an explicit wait here. But the demo's print
   // ordering is more dramatic with an explicit pause — main visibly waits
   // for the pirates to finish.
   wait sleepAsync(150)
   print(`all pirates accounted for`)
   ```
   Plus a new helper function at the bottom of the file:
   ```yinz
   function pausePirate(n: int) -> nothing {
     // wait sleepAsync — non-blocking; the OS thread is freed while the timer runs.
     // M1's sleepMs (used in the v0.3-M1 section above) would tie up the thread instead.
     wait sleepAsync(100)
     print(`pirate ` + n.toString() + ` done`)
   }
   ```
2. **Create `examples/primantis-orders/v0_3_m2_errors.ynz`** with intentional triggers for every new M2 compile error/warning class:
   - `wait_on_non_may_block_warning` (a `wait print("hi")` — print is CPU-bound)
   - `wait_on_non_call_expression` (a `wait 42` — primitive)
   - `unawaited_sleep_async` (a `sleepAsync(100)` without `wait`)
   - Each block has a `// WHY:` comment naming the diagnostic class
3. Add to `examples/primantis-orders/README.md`: a line linking the new v0_3_m2_errors.ynz file.
4. **Extend cross-impl consistency harness** (`crates/ynz-driver/tests/cross_impl_consistency.rs`):
   - Add the new M2 fixtures to the `TIMING_DEPENDENT_FIXTURES` const allowlist: `v0_3_m2_concurrent_waits_proof.ynz`, etc.
   - Verify the harness still passes on the rest of the corpus (existing M1 + M2 wait-free fixtures)
5. **Create `crates/ynz-driver/tests/m2_state_machine_integration.rs`** with full integration tests:
   - `eight_background_waits_concurrency_proof` — uses the fixture from P2 Step 10; asserts all 8 `START N` lines appear BEFORE any `DONE N` line (core-count-independent concurrency proof); total wall-clock in band `[80ms, 200ms]`
   - `errors_cascade_through_state_machine` — call a fn that does `wait __testFallibleAsync(true)`, assert returned error has `Frame.line` equal to the EXPECTED line number computed from reading the fixture file (NOT just "non-zero"). Code: `let expected_line = fixture_text.lines().position(|l| l.contains("wait __testFallibleAsync(true)")).unwrap() + 1; assert_eq!(error.trace[0].line, expected_line)`. Also assert `error.source.file` equals the fixture's `.ynz` file basename. Validates end-to-end Yinz-span propagation through state-machine suspension.
   - `block_on_bridge_blocks_main` — main (non-state-machine) calls a state-machine fn WITHOUT `wait`, assert main blocks for the full duration in band `[80ms, 200ms]` (documented suboptimal-but-correct M2 behavior)
   - `state_machine_function_with_panic_does_not_crash_main` — state-machine fn that panics; assert main reaches its marker; program exits 0
   - `wait_inside_if_branches_correctly` — fn with `wait` inside an `if`; both branches tested
   - **`wait_inside_loop_completes_concurrently` (per round-1 adversarial case 1)** — fn with `for (i in range(0, 10)) { wait sleepAsync(10) }` inside a `background` task. Total wall-clock for the function: band `[80ms, 150ms]` — proves the 10 sequential waits inside the loop work correctly.
   - **`recursive_state_machine_fn` (per round-1 adversarial case 3)** — implements `fibA(8) -> int errors` returning 21; asserts no shared-state corruption between recursive frames.
   - **`background_state_machine_with_error_discards_error` (per round-1 adversarial case 5)** — `background fnReturnsError()` where fnReturnsError is a state-machine fn that ALWAYS returns an error. Asserts main reaches its final marker AND the program exits 0 (errors from background are discarded same as panics).
   - **`heap_string_local_survives_wait_boundary` (per round-1 adversarial case 7)** — fn with a `string` local declared before `wait sleepAsync(50)` and used after. Asserts the post-suspension string has correct length, bytes, and SSO/heap discriminant.
   - **`non_sm_fn_called_from_sm_fn_with_inner_sm_call_does_not_nest_block_on` (per round-2 Required Fix #1)** — three-fn chain: state-machine `main()` → regular `mid()` (no wait) → state-machine `inner()`. `mid()` does NOT contain `wait`, so its `contains_wait == false`. From inside `mid()`, the call to `inner()` triggers the codegen path that emits `ynz_rt_call_state_machine_sync` Shape B. Asserts program completes WITHOUT panic + with correct stdout (the `Handle::block_on` worker-thread path activates; scheduler pressure but correctness preserved).
   - **`state_machine_can_background_state_machine_without_wait` (per round-2 Required Fix #2)** — fn with `wait sleepAsync(50)` (so it's a state machine) calls `background other()` where `other()` is also a state machine. Asserts: compiles clean (no `wait_required_on_state_machine_call` false-positive); runs to completion with `other()` running concurrently.
   - **`background_regular_fn_that_internally_calls_state_machine_does_not_crash` (per round-2 Concern #1)** — `background mid()` where `mid()` is a regular fn (no wait) that internally calls a state-machine fn. Routes to `ynz_rt_spawn_blocking` (per P2 Step 5 local check); inside the blocking pool worker, the inner state-machine call uses `ynz_rt_call_state_machine_sync` Shape B (`Handle::block_on` — works on blocking-pool threads; no `block_in_place` panic). Asserts no panic + correct output.
   - **`mutual_recursion_state_machines` (per round-3 adversarial case 1)** — `function a(n) { if n > 0 { wait b(n-1) } }` + `function b(n) { if n > 0 { wait a(n-1) } }`. Each call alternates between two state-machine frames. Asserts: `a(8)` completes without deadlock; codegen handles mutual recursion (likely via forward declarations).
   - **`state_machine_errors_before_first_wait` (per round-3 adversarial case 2)** — `function f() -> int errors { if shouldFail { return error("early") }; wait sleepAsync(50); return 0 }`. Frame is allocated; early return must free it. Asserts: when `shouldFail` is true, no frame-leak (`ynz_alloc/ynz_free` counters return to baseline) AND error has populated trace.
   - **`sleepAsync_boundary_values` (per round-3 adversarial case 3)** — `wait sleepAsync(0)` should yield immediately (Tokio `Duration::ZERO` semantics; completes in < 5ms). `wait sleepAsync(-1)` must be rejected at typeck OR clamped to 0 at runtime. **Decision (locked)**: typeck does NOT validate int range for `sleepAsync`'s arg (Yinz allows any `int` literal); runtime CLAMPS negative values to 0 via `let ms = ms.max(0) as u64;` inside `ynz_rt_async_sleep_create`. Documented as a runtime safety net; no compile-time error to keep the intrinsic signature simple.
6. **Bump workspace `Cargo.toml`** version from `0.3.0-m1` to `0.3.0-m2`.
7. **Add CHANGELOG entry**. Required sections (per plan-reviewer Concern):
   - **Features**: state machine codegen for `wait` suspension; `sleepAsync(int) -> nothing` public intrinsic; `block_on` bridge for non-state-machine callers; 8-concurrent-wait demonstration in pirates-roster.
   - **Improvements**: no-coloring promise preserved (caller signature unchanged whether callee is state-machine or not); M3-ready ABI in place for transitive may-block analysis.
   - **Known limitations** (M2→M3 transition surface): (a) state-machine fn calling state-machine fn without `wait` triggers a Tier 3 warning (M3 auto-insertion eliminates the warning); (b) non-state-machine caller of state-machine fn uses `ynz_rt_call_state_machine_sync` Shape B (`Handle::block_on` everywhere — runtime-safe on all three thread contexts; cost: when called from a worker thread, scheduler pressure during the wait. M3 may-block analysis eliminates most call sites). (c) Concurrent sync-bridge calls from many threads use `Handle::block_on` (NOT `block_in_place`) — no Tokio panic; the cost is scheduler pressure, not correctness.
   - **Migration notes**: any M1 program using `wait` continues to compile and runs IDENTICALLY (M1's `wait` was identity-passthrough; M2's `wait` actually suspends, which is the documented design — observable difference is timing, not correctness).
   - **Internal-only**: test-only `__testFallibleAsync(bool) -> int errors` intrinsic for errors+wait validation; new `internal_fns` mechanism in `intrinsics.rs`; deletes when v0.5 ships real fallible async I/O.
   - Cross-link to merged PRs from each phase.
8. **Update `.claude/state.md`** with a new entry under Active Decisions documenting v0.3.0-m2 ship: key architectural decisions made (block_on bridge, sleepAsync naming, internal test intrinsic).
9. **Run `/release`** to cut the `v0.3.0-m2` tag. The release skill: bumps Cargo.toml (already done in step 6), commits, tags, pushes. Verify the GitHub release has both `yinz-0.3.0-m2.vsix` and `yinz-latest.vsix` attached.

**Acceptance criteria**:
- [ ] `examples/pirates-roster/entrypoint.ynz` has v0.3-M2 section with 8 pirates; snapshot test asserts all 8 `pirate N done` lines + `scheduled 8 pirates` marker + `all pirates accounted for` marker appear; total wall-clock for the section in band [80ms, 300ms] (includes the 150ms sleepAsync that keeps main alive)
  - Evidence: (filled at phase completion)
- [ ] `examples/primantis-orders/v0_3_m2_errors.ynz` exists with all 3 new error/warning triggers + `// WHY:` comments; snapshot test asserts each diagnostic fires
  - Evidence: (filled at phase completion)
- [ ] Cross-impl consistency harness passes on the non-timing-dependent corpus (existing M1 fixtures + new wait-free M2 fixtures)
  - Evidence: (filled at phase completion)
- [ ] `eight_background_waits_concurrency_proof` integration test passes: all 8 `START N` lines appear before any `DONE N` line (core-count-independent proof); total wall-clock in band [80ms, 200ms]
  - Evidence: (filled at phase completion)
- [ ] `errors_cascade_through_state_machine` test passes: error has populated Frame.line + SourceLoc
  - Evidence: (filled at phase completion)
- [ ] `block_on_bridge_blocks_main` test passes: main blocks for full duration; output correct
  - Evidence: (filled at phase completion)
- [ ] `state_machine_function_with_panic_does_not_crash_main` test passes
  - Evidence: (filled at phase completion)
- [ ] `wait_inside_if_branches_correctly` test passes (both branches)
  - Evidence: (filled at phase completion)
- [ ] `Cargo.toml` workspace version is `0.3.0-m2`
  - Evidence: (filled at phase completion)
- [ ] CHANGELOG section added with detailed M1→M2 upgrade narrative
  - Evidence: (filled at phase completion)
- [ ] `.claude/state.md` updated with v0.3.0-m2 ship entry with WHY-level context
  - Evidence: (filled at phase completion)
- [ ] [Deferred to /release step] GitHub release tagged `v0.3.0-m2`; both VSCode extension `.vsix` files attached (needs user confirmation + GitHub push)
  - Evidence: (filled at phase completion)
- [ ] `cargo test --workspace` passes (1220+ existing + new M2 integration tests; ~30+ new tests across P0-P5)
  - Evidence: (filled at phase completion)

**Quality gate**:
- [ ] Demo extension uses real Yinz operations from current scope (no invented APIs) per `.claude/rules/dot-postfix.md`
- [ ] Error gallery uses real Yinz operations (no invented APIs)
- [ ] Timing test tolerances are 1.5× ideal to account for CI scheduler noise (150ms ceiling for ~100ms ideal)
- [ ] CHANGELOG entry detailed enough for v0.3.0-m1 → v0.3.0-m2 upgrade understanding (named architectural decisions + behavior changes)
- [ ] No release-blocking warnings in `cargo build --release` (verified before /release)
- [ ] `.claude/state.md` Active Decisions entry includes WHY-level context for each architectural choice (block_on bridge, sleepAsync naming, internal test intrinsic)
- [ ] No SQL/security concerns

**Verification**:
- `cargo test --workspace` passes
- `cargo test cross_impl_consistency` and `cargo test m2_state_machine_integration` named test groups pass
- Manual: `./target/debug/ynz run examples/pirates-roster/` shows all 8 pirate-done lines arriving in close succession + total time < 300ms (includes 150ms sleepAsync at end)
- `/release` cuts a clean tag with both `.vsix` files

**Phase Review Gates** (filled at phase completion):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log** (filled during any fix loops):
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS (final phase):**

1. **Persist plan state.** Tick all remaining checkboxes across all phases. Verify the milestone-level Quality Checklist below has all boxes checked or N/A. Bump `last_updated:` to today.
2. **Invoke 4-agent review fan-out with cumulative scope** (Step 10f). Diff command: `git diff <plan-base-commit>..HEAD` covering all 6 phases. Brief: "End-of-plan review. Audit cumulative diff against ALL phases' acceptance criteria, Quality Gate items, the plan's overall Quality Checklist, the invariants block, and rules. Critical focus: (a) state-machine codegen doesn't leak Rust/Tokio types into user surface, (b) all teaching surfaces present (registry, LSP, VSCode, demo, gallery, design-doc deferral notes), (c) cross-impl harness covers the corpus correctly, (d) `__testFallibleAsync` is genuinely internal (not in registry, not in LSP completions), (e) `block_on` bridge correctness verified."
3. **Handle the verdict.** BLOCK → fix or push back with evidence (max 3 rounds). PASS → continue.
4. **Flip status.** Edit front-matter `status: active` → `status: done`. The radar will auto-move this file to `plans/done/` on next SessionStart.
5. **Cut the release.** Run `/release` to tag `v0.3.0-m2`. Confirm with user before pushing.
6. **Prompt the user.** Tell them: "v0.3-M2 done. Review fan-out: PASS. Cumulative tests: [count]. Tag v0.3.0-m2 ready to push. Roadmap rollup: 2 of 4 v0.3 milestones remain (M3 may-block + auto-parallel; M4 channels + SoA + v0.3.0 tag)."
7. **Roadmap rollup**: per /plan Step 11a, the roadmap `v0-3-concurrency-perf` has 2 milestones remaining; don't auto-mark done. Suggest M3 as the next planning target.

---

## Quality Checklist (verify at completion)

- [ ] All inputs validated — no new user input surface (compiler internal change); intrinsic registration uses `M2_MAY_BLOCK_INTRINSICS` in-code const set per Question Resolution #7 (no schema field added); transitive may-block analysis deferred to M3 per `### Performance` "M2 may-block predicate" subsection
- [ ] Auth/authz — N/A (compiler)
- [ ] Error handling: every new diagnostic uses WHAT/WHAT-INSTEAD/WHY (verified by per-phase reviews); no leaked Rust panic strings to user (catch_unwind boundaries in all new runtime shims)
- [ ] No SQL injection, XSS, path traversal, or secret exposure — N/A
- [ ] Performance: state-machine path used only when needed; codegen+typeck wall-clock ≤ 10% slower than M1 baseline on pirates-roster (measured); sync-bridge overhead ≤ 1000µs per call = 1% of 100ms wait (measured; single threshold per Round 3 reconciliation)
- [ ] Tests: state-machine integration + errors-cascade + block_on bridge + panic-through-state-machine + multi-wait + wait-in-if + cross-impl harness + LSP completion visibility + LSP diagnostic flow + registry parse all pass
- [ ] Existing tests still pass (1220+ existing + ~30+ new M2 tests)
- [ ] Types are complete (no `as any` equivalent; widening casts limited to C-ABI boundary with SAFETY comments)
- [ ] Follows existing codebase conventions: runtime extern fn pattern, codegen runtime_decls registration, typeck intrinsic registration, registry TOML schema, demo/error-gallery file patterns
- [ ] Every phase received a 4-agent review-fanout PASS before committing (P0–P5)
- [ ] Final cumulative review fan-out passed (all 6 phases PASS)
- [ ] Plan-file acceptance-criteria checkboxes accurate across all phases
- [ ] `__testFallibleAsync` confirmed NOT in registry, NOT in LSP completions (P3 test)
- [ ] Cross-impl consistency harness allowlist updated with timing-dependent M2 fixtures

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: Each of the 6 phases is one PR. P2 is the largest (state-machine codegen ~600 lines) but stays focused — state-machine IR generation is tightly coupled and shouldn't split. Sub-helpers (state_machine.rs module) extracted within P2 for clarity, not as a separate PR.
- **Shadow main branches**: All phases target `main` directly via PR. No long-lived integration branch.
- **Building the engine before shipping value**: P0-P1 are foundation (no user-visible value), but P2 ships actual value (wait actually suspends). P4 ships full teaching surface in the same milestone — not a follow-up.
- **Hotfix that isn't**: N/A — no hotfix-style work in M2.
- **Abandoned branches**: Each phase's branch merges to main at phase boundary; no branches survive past their phase PR. The P0 spike branch either becomes P1's baseline (rebase) OR gets explicitly closed if findings reject the state-machine design.
- **Flag graveyards**: `--no-auto-parallel` (M1 plumbing, no-op until M3) carries forward unchanged. M2 adds no new flags. Per `~/.claude/memory/branching.md` §Feature Flags 30-day cleanup, the M1 flag has a defined activation date (M3 ships) — NOT graveyard material.

---

## Reviewer Disputes

**Round 1 (2026-05-30)** — BLOCK verdict, 11 Required Fixes + 6 Concerns + 7 Suggested Adversarial Cases. All accepted; no disputes. Tier classification: A (correctness-critical) — accepted.

Required Fixes applied:

1. **`may_block`/`is_internal` "locate at execution" duct-tape** — fixed via Question Resolution #7 + #8 (added to `## Questions` section). Decision: in-code `M2_MAY_BLOCK_INTRINSICS: &[&str]` const set (no schema change) + new `internal_fns: Vec<(...)>` field on `PrimitiveIntrinsicTable` (NOT cfg(test), production typeck access). P3 Step 3 + 4 + acceptance criteria updated to specify exact mechanism.

2. **LSP completion test file "locate at execution"** — fixed. File confirmed to exist at `crates/ynz-lsp/tests/completion.rs`. P3 Step 10 + acceptance criteria reference the actual path.

3. **`contains_wait` struct "locate at execution"** — fixed. `FunctionSig` confirmed at `crates/ynz-typeck/src/signatures.rs:14`; population site at `:48` in `collect_signatures`. P3 Step 5 + Current-state Anchors updated.

4. **`sleepMs` registry pattern false claim** — partially disputed. The reviewer's grep missed the entry at `registry/features.toml:625-636` — `sleepMs` IS in the registry. However, the reviewer's deeper point stands: typeck dispatch (`check.rs:1452-1453`) and codegen dispatch (`emit.rs:2490`) are hardcoded string-match arms parallel to the registry entry. P2 Step 7 + P3 Step 2 updated to explicitly add the `sleepAsync` triple: registry entry + typeck check.rs:1453 arm + codegen emit.rs:2490 arm.

5. **Silent-bug risk: timing-based one-sided assertions** — fixed across all timing assertions in plan. Every wall-clock test now has a `[lower, upper]` band (lower catches broken sleep returning instantly; upper catches sequential execution). Applied to P0 Contracts #1-4, #7, P2 Step 10 concurrency proof, P5 integration tests.

6. **`wait_on_non_may_block_warning` semantic gap** — fixed. Added explicit "M2 may-block predicate (precise spec)" subsection inside `### Performance` with the exact predicate code. New P3 Step 13 adds the `transitive_no_wait_does_not_trigger_warning.ynz` fixture that validates M2's local-only behavior and serves as an M2→M3 transition checkpoint.

7. **`Frame.line` populated-but-not-correct silent bug** — fixed. P5 Step 5's `errors_cascade_through_state_machine` test now reads the fixture file to compute the EXPECTED line number, then `assert_eq!` against it. Spike Contract #5 clarified as struct-shape validation (uses literal `"__spike_fixture__"`) — end-to-end span propagation goes to P5 integration test.

8. **8-task timing test core-count fragility** — fixed by switching from wall-clock-only assertion to interleaved-print-order proof. Renamed test fixture `v0_3_m2_8_concurrent_waits.ynz` → `v0_3_m2_concurrent_waits_proof.ynz`. Adds `START N` / `DONE N` prints; asserts all 8 STARTs appear before any DONE — core-count-independent. Wall-clock band assertion kept as secondary signal (catches gross perf regression).

9. **`wait_on_non_call_expression` forward-pointing "awaitable value"** — fixed. Removed "or an awaitable value" clause from the canonical WHAT INSTEAD text. M2 / M3 have no source-typeable awaitable values; the diagnostic is now simply "must be followed by a function call." Applied to `### Teaching` + `### Feature Registry Entries` canonical text.

10. **State-machine-calls-state-machine-without-wait algorithm gap** — fixed by Question Resolution #6 (compile error via new `wait_required_on_state_machine_call` diagnostic). P2 Step 6 has explicit 4-case decision table. P3 Step 8 implements the typeck check. Lock-in: prevents the Tokio nested-block_on panic AT TYPECK, not at runtime. M3 lifts via auto-`wait` insertion with zero source change.

11. **Spike SourceLoc.file = "spike.rs" Yinz-source confusion** — fixed. P0 Contract #5 explicitly scopes itself to "validates the SourceLoc STRUCT SHAPE and writability — that the struct's bytes survive across the `Pending → Ready(Err)` boundary intact" — using a Rust-literal value `"__spike_fixture__"`. End-to-end Yinz-span propagation moved to P5 integration test (Required Fix #7's `errors_cascade_through_state_machine`).

Concerns addressed:

- **P2 scope creep guard at ~800 lines** — noted; P2's exit sequence will flag if line count exceeds 800.
- **`block_on` thread-starvation stress test at 64 concurrent calls** — explicitly out of M2 scope; the new `wait_required_on_state_machine_call` error catches the worst case (state-machine-to-state-machine without wait → Tokio panic). Stress testing of `block_on` from many non-state-machine threads deferred to M3 when auto-`wait` lifts the limitation.
- **Design-doc deferrals not in registry** — fixed. New `[[deferred_tooling_feature]]` `async-io-stdlib-intrinsics-v0-5` entry in `### Feature Registry Entries`; P4 Step 4b adds it; design-doc notes become cross-references to the SSOT registry entry.
- **`unawaited_sleep_async` not in registry-entries** — fixed. Added as 3rd diagnostic_template in `### Feature Registry Entries` + P4 Step 4.
- **`block_on` overhead 100µs no provenance** — fixed. P0 Step 8 now uses relative threshold (< 1% of 100ms wait) AND keeps absolute hard fail (> 5000µs).
- **CHANGELOG vague** — fixed. P5 Step 7 now lists required sections (Features / Improvements / Known limitations / Migration notes / Internal-only) with content guidance per section.

Adversarial cases added (per reviewer's "Suggested Adversarial Cases"):

- **Case 1 (wait-in-loop)** → P0 Spike Contract #7 + P5 integration test `wait_inside_loop_completes_concurrently`
- **Case 2 (wait-in-if-condition)** → P0 Spike Contract #8
- **Case 3 (recursive state machine)** → P0 Spike Contract #10 + P5 integration test `recursive_state_machine_fn`
- **Case 4 (wait of wait)** — handled via existing `wait_on_non_call_expression` error (nested `Expr::Wait` is not a `Call`); Risks table row added; test `wait_of_wait_rejected` in P3
- **Case 5 (background of state-machine returning errors)** → Risks table row added + P5 integration test `background_state_machine_with_error_discards_error`
- **Case 6 (concurrent block_on from two threads, Tokio panic)** → Risks table row added; the new `wait_required_on_state_machine_call` typeck error blocks the worst case (state-machine inside state-machine); documented in CHANGELOG known-limitations
- **Case 7 (heap-string survives suspension)** → P0 Spike Contract #9 (locks per-type slot-sizing ABI) + Risks table row + P5 integration test `heap_string_local_survives_wait_boundary`

**Round 2 (2026-05-30)** — BLOCK verdict, 3 Required Fixes + 4 Concerns + 3 new Adversarial Cases. All accepted; no disputes.

Round 2 Required Fixes applied:

1. **Transitive nested-`block_on` runtime panic class reachable from local-only typeck check** — fixed via Question Resolution #9 (new runtime-aware `ynz_rt_call_state_machine_sync` shim, replacing the originally-planned `ynz_rt_block_on`). Body uses `tokio::runtime::Handle::try_current` + `tokio::task::block_in_place` when inside a Tokio context; falls back to `RUNTIME.block_on` when outside. **Picked Option B** from reviewer's suggested fix options — preserves the no-coloring promise without requiring transitive analysis. The typeck `wait_required_on_state_machine_call` becomes a TEACHING warning (guides users toward writing `wait` for perf), not a CORRECTNESS gate; runtime path is panic-safe even if the warning is ignored. P1 Step 4, P2 Step 6 decision table, P2 Step 8 acceptance criteria, Risks table all updated. New P5 integration test `non_sm_fn_called_from_sm_fn_with_inner_sm_call_does_not_nest_block_on` validates the transitive case end-to-end.

2. **`wait_required_on_state_machine_call` false-positive on `background sm_fn()` from inside state machines** — fixed via P3 Step 8 background-exemption: pass `inside_background: bool` down typeck recursion; when the call is the immediate inner of `Expr::Background`, the check is exempted. New P5 integration test `state_machine_can_background_state_machine_without_wait` validates compile-clean + concurrent execution. P2 Step 6 decision table notes the exemption.

3. **P2 Step 6 (No, Yes) row mislabeled as "typeck rejects"** — fixed. Row updated to "Unreachable — writing `wait` upgrades caller's `contains_wait` to true at typeck time (per P3 Step 5 AST walk), so this row never fires. Listed for completeness; no codegen emission." No more phantom check reference.

Round 2 Concerns addressed:

- **Background routing transitive-case test** → added P5 integration test `background_regular_fn_that_internally_calls_state_machine_does_not_crash` per Concern #1.
- **`wait_required_on_state_machine_call` WHY references M3 work that doesn't exist yet** → noted; tracked in `.claude/todos.md` (will add cross-workstream TODO during plan-execution kickoff): "v0.3-M3 plan must verify auto-`wait` insertion lifts `wait_required_on_state_machine_call` or update the WHY text."
- **`internal_fns` API exposure to production typeck without guard** → P3 Step 4 doc comment will include `#[doc(hidden)]` attribute + `// USAGE GUARD: only ever called from M2 state-machine test fixtures or registered in M2_MAY_BLOCK_INTRINSICS. Production code paths should call lookup_free_fn, not lookup_free_fn_including_internal.` Belt-and-braces enforcement.
- **P5 demo with only 4 pirates doesn't visibly distinguish thread-sharing** → bumped to 8 pirates per Concern #5. P5 Step 1 demo snippet updated.

Round 2 Adversarial cases added:

- **Case 1 (state-machine `main` → regular fn → state-machine inner)** → P5 integration test `non_sm_fn_called_from_sm_fn_with_inner_sm_call_does_not_nest_block_on` (already added via Required Fix #1)
- **Case 2 (background of state-machine fn from inside state machine)** → P5 integration test `state_machine_can_background_state_machine_without_wait` (already added via Required Fix #2)
- **Case 3 (nested branching with wait in some branches but not others)** — covered by existing P0 Contract #3 (wait-in-if) + P5 `wait_inside_if_branches_correctly`. The "nested branching" case adds no new ABI surface beyond what Contract #3 validates (each branch is its own state in the resume-point switch; nesting just adds more states). Documented as "covered by Contract #3 + the integration test; nested branching adds no new ABI surface, only more states in the same switch."

**Round 3 (2026-05-30)** — BLOCK verdict, 8 Required Fixes + 5 Concerns + 4 new Adversarial Cases. All accepted; no disputes — Round 2's Option B shim was technically wrong (the canonical `block_in_place`-from-`spawn_blocking` panic-trap memory `feedback_tokio_block_in_place_thread_context.md` flagged the exact bug). Round 3 catches it before any code is written. Round 4 revisions applied:

Round 3 Required Fixes applied:

1. **`block_in_place` from `spawn_blocking`-pool panic-trap** — fixed by switching to **Shape B** of the sync bridge (`Handle::block_on` everywhere inside Tokio, NO `block_in_place`). Tradeoff: when called from worker thread, ties up that worker for the wait duration. Cost is real but bounded; M3's auto-`wait` insertion eliminates most call sites. P1 Step 4 body fully rewritten; Risks table line 109 updated; Question Resolution #9 rewritten; CHANGELOG known-limitations bullet (c) corrected. Validated by NEW P0 Spike Contracts #4a (worker), #4b (spawn_blocking-pool), #4c (no-runtime), #4d (state-machine-in-state-machine).

2. **State-machine-inside-state-machine sync bridge claim undefended** — fixed by P0 Contract #4d which specifically exercises outer SM A's poll() invoking the shim driving inner SM B (where B itself has `wait`). Validates that Tokio's reactor still drives B's wakers while A's worker is parked.

3. **`waker_ctx` ABI undefined → silent hang risk** — fixed by adding **Waker ABI LOCKED** spec to P2 Step 7: `waker_ctx: *mut u8` is a type-erased pointer to `std::task::Context<'_>`; codegen MUST forward the exact pointer (no fabricated Wakers) to all inner polls. Validated by NEW P0 Contract #11.

4. **`RUNTIME.get().unwrap()` panic with no teaching** — fixed by **codegen-invariant proof**: `ynz_rt_init` is always the first instruction in `main`'s entry block whenever any compilation unit fn contains `wait`/`background`. Invariant validated by NEW P0 Contract #12 (IR snapshot of main's entry shows the call as first non-allocation instruction). The `.expect()` panic becomes unreachable in correct codegen; serves as defense-in-depth against future compiler bugs.

5. **`wait_required_on_state_machine_call` warning/error classification inconsistency** — fixed by **locking it as WARNING** (Tier 3) consistently across all 6 places: `### Teaching` (line 245), `### Feature Registry Entries` canonical text, P2 Step 6 decision table row (Yes, No), P2 Step 8 acceptance criterion, P3 Step 8 description, P3 acceptance criterion. Test fixture's stderr expectation now specifies `warning:` substring + exit code 0 explicitly.

6. **Frame cleanup on cancellation with non-trivial locals** — fixed by expanding P0 Contract #6 to include `state_machine_no_leak_with_string_locals`: 1000 state machines with `string` local crossing wait boundary, half cancelled mid-wait; both `ynz_alloc/ynz_free` AND `ynz_string_free` counters must return to baseline. Frame layout decision locked: state machine struct implements `Drop` with per-slot `live_mask: u64` bitfield + per-type destructor calls. Spike validates the layout.

7. **`wait background X()` parser/typeck handling unspecified** — fixed by clarifying P3 Step 6: `Expr::Background` is statement-position in M1 (verified at `crates/ynz-parser/src/parser.rs`), so `wait background X()` is a PARSER error. The (true, true) corner of the two-flag state space is unreachable; documented. Test fixture `wait_of_background_rejected.ynz` triggers the parser error.

8. **Demo shutdown race** — fixed by adding `wait sleepAsync(150)` after `print("scheduled 8 pirates")` in the pirates-roster demo. Keeps main alive past all background pirates' waits; output ordering is deterministic + dramatic.

Round 3 Concerns addressed:

- **Overhead threshold reconciliation** (100µs vs 1000µs vs 5000µs) → single threshold locked: **≤ 1000µs absolute = ≤ 1% of 100ms wait**. Applied to `### Performance` invariant, P0 Step 8, Quality Checklist.
- **`M2_MAY_BLOCK_INTRINSICS` inline rationale** → added Tier 3 doc comment block in P3 Step 3 explaining the legitimate-deferral shape (M3 transition lifts it).
- **CHANGELOG known-limitations bullet (c)** → updated from "compile error" to "Tier 3 warning" + accurate description of the sync bridge cost.
- **`__testFallibleAsync` non-test fixture rejection test** → P3 Step 10 LSP visibility test sufficient; if a user-authored `.ynz` references `__testFallibleAsync`, it falls through to the standard "undefined function" error path (no special handling needed).
- **`internal_fns` API guard** → `#[doc(hidden)]` + USAGE GUARD comment added inline in P3 Step 4 code spec.

Round 3 Adversarial cases added (in P5 integration tests):

- **Case 1 (mutual recursion between two state machines)** → `mutual_recursion_state_machines`
- **Case 2 (errors before first wait)** → `state_machine_errors_before_first_wait`
- **Case 3 (sleepAsync boundary values)** → `sleepAsync_boundary_values` — decision locked: typeck doesn't range-check; runtime clamps negative ms to 0.
- **Case 4 (SIGTERM mid-wait)** → covered by Contract #6's expanded `state_machine_no_leak_with_string_locals` which uses `runtime.shutdown_timeout(Duration::ZERO)` — that simulates a SIGTERM by aborting outstanding tasks. No separate test needed.
