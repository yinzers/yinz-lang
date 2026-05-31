# v0.3-M2 Re-Spike Findings — State-Machine Codegen Divergence Map

**Date**: 2026-05-31
**Method**: real `.ynz` compiled+run through `./target/debug/ynz` (binary == committed Phases 0–4, rebuilt clean exit 0). NOT hand-written Rust — that's what gave the P0 spike its false ACCEPT.
**Purpose**: RE-ENTRY CONTRACT step 1 — map the full divergence before `/plan`ing the codegen fix.

Scratch fixtures: `/tmp/ynz-respike/*.ynz` (not committed).

---

## Verified works/breaks table (real compiler output)

| Case | Source shape | Result | Exit |
|---|---|---|---|
| control_value_no_wait | `f() -> int { return 5 }` (no wait → not an SM) | prints `5` | 0 ✅ |
| works_toplevel_nothing | top-level `wait sleepAsync` in `-> nothing`, called from non-SM entrypoint | `before`/`after` | 0 ✅ |
| works_wait_in_if | `wait sleepAsync` inside `if` body | correct both branches | 0 ✅ |
| works_concurrency_proof | 8× `background pause()` from a **non-SM** entrypoint | all 8 START + 8 DONE + MAIN | 0 ✅ |
| **bug_a_value_int** | `f() -> int { wait sleepAsync(10); return 5 }` | **LLVM verify failure** | 1 ❌ |
| **bug_a_value_errors** | `f() -> string errors { wait …; return … }` (error handled via `.or()`) | **codegen panic** (different from int!) | 2 ❌ |
| **bug_b_nested_sm** | entrypoint→`wait outer()`→`wait inner()`→`wait sleepAsync` | **runtime nounwind ABORT** | 1 ❌ |
| **bug_c_background_from_sm** | SM entrypoint (own wait) + `background worker()` | **HANG** (8s timeout) | 124 ❌ |

---

## Bug A — value-returning state machines. TWO distinct failure modes by return type.

### A1 — `-> int` (and any non-pointer scalar): LLVM verify failure
```
LLVM module verify failed: Function return type does not match operand type of return inst!
  ret i64 5
 i32Function return type does not match operand type of return inst!
  ret void
 i64
```
Two mismatches, two root causes:
1. **`ret i64 5` in the i32 resume fn** — the user's `return 5` in the SM body is lowered as a raw `ret <expr>` directly inside `ynz_sm_<name>_resume` (typed `i32 (ptr,ptr)`). It should instead store the value into a frame return slot and `ret i32 <discriminant>`. `lower_sm_body`'s return handling never routes through `store_return_value`.
2. **`ret void` in the i64 wrapper** — `lower_function_with_waits` emits `builder.build_return(None)` for **every** non-`main` wrapper (`emit.rs:1419-1424`, "Non-main wrapper: returns nothing (void)"), even when the function's declared LLVM return type is `i64` (`-> int`). The wrapper never reads/returns the actual value.

### A2 — `-> string` / `-> string errors` (pointer / `{i64,i64}` return): codegen panic
```
panic: Found IntValue(... "%sm_sync = call i32 @ynz_rt_call_state_machine_sync(...)") but expected PointerValue variant
location: crates/ynz-codegen/src/emit.rs:6835
```
The wrapper / call-site feeds the **i32** sync-bridge result where the continuation expects a pointer (`string`) or the `{i64,i64}` errors struct. The entire value-return path is hardcoded i32-shaped (exit-code shaped). `errors` ABI (`{i64 error_ptr, i64 success}`) cannot survive a single i32 at all.

### Root cause (unifying A1+A2)
`state_machine::store_return_value` (`state_machine.rs:159`) **truncates to i32 and writes into frame slot 0** (the `resume_point` slot). The sync bridge returns a single `i32`. This was only ever exercised by `entrypoint`/`main` (exit code is genuinely i32) and `-> nothing` fns (no value). **There is no properly-typed return slot in the frame and no typed return path through the wrapper.**

### Fix surface (A)
- Add a properly-sized, properly-typed **return-value slot** to the frame (must hold i64, pointer, AND the `{i64,i64}` errors struct → 16 bytes, separate from `resume_point`).
- `lower_sm_body` return handling: store the value to the return slot, then `ret i32 <Ready discriminant>` — never raw `ret <expr>`.
- Wrapper (`lower_function_with_waits` Part 2): load the typed value from the return slot and `ret <typed value>` matching the declared return type (not `ret void`). For `errors`, reconstruct the `{i64,i64}` struct.
- "Poll<T>" framing from the contract = the resume fn signals Ready/Pending **and** carries the value in the frame's typed return slot; the driver reads it on Ready.

---

## Bug B — nested `wait` SM-from-SM: runtime nounwind ABORT (not a clean error)

```
thread '<unnamed>' panicked at crates/ynz-runtime/src/runtime.rs:612:24:
Cannot start a runtime from within a runtime. ...
thread '<unnamed>' panicked at .../panicking.rs:225:5:
panic in a function that cannot unwind        ← becomes an ABORT across extern "C"
```

### Root cause
- `lower_sm_body` only emits inline poll-and-yield for `wait sleepAsync(...)`. A `wait outer()` where `outer` is a **user** state machine is NOT intercepted — it falls through to `lower_expr`'s call-site dispatch.
- That dispatch (`emit.rs:3582-3616`) emits the **sync bridge** `ynz_rt_call_state_machine_sync` for *any* may-block callee with a resume fn — the in-code comment claims `(caller-SM, wait) → inline poll-and-yield (handled by lower_sm_body)` but **that path was never implemented**. Only `sleepAsync` got inline poll-and-yield.
- The sync bridge (`runtime.rs:611`) calls `handle.block_on(future)` when inside Tokio. `Handle::block_on` panics "Cannot start a runtime from within a runtime" when called from a thread already driving the runtime — i.e. exactly the SM-from-SM case. (Round-3's "Shape B = Handle::block_on everywhere" avoided the *block_in_place* panic but NOT this one.) Because the panic crosses the `extern "C"` resume-fn boundary, it's a nounwind **abort**, not a catchable error — `catch_unwind` in the bridge can't save it.

### Fix surface (B)
- `lower_sm_body`: when a `wait <userSM>(args)` appears inside a state machine, emit **genuine inline poll-and-yield**: alloc the inner frame, store args, then at the continuation state call `ynz_sm_<inner>_resume(inner_frame, waker_ctx)` **directly** (forwarding the SAME `waker_ctx` — never fabricate a waker, per the ABI lock), branch Ready→read inner return slot + continue / Pending→`ret i32 1` up to the outer driver. The inner SM becomes additional states in the outer resume fn's switch.
- **Delete the sync-bridge fallback for the in-async-context SM-from-SM path.** The bridge stays only for the genuine non-SM→SM top-level entry (main, or a non-SM fn calling an SM) where `block_on` from outside any runtime is legitimate.

---

## Bug C — `background` from inside a state machine: HANG

SM entrypoint (own `wait`) + `background worker()` → the worker future is spawned but never driven; the calling thread is parked in `block_on` driving the entrypoint SM, and the spawn isn't progressed → 8s timeout.

### Fix surface (C)
- Once Bug B's inline poll-and-yield removes the nested-`block_on` parking, `background` from an SM must spawn-and-drive on the runtime such that the parent SM yielding (Pending) lets the runtime progress the backgrounded task. Validate the spawned task runs concurrently with the parent SM's remaining waits. Likely shares the inline-poll rework: the parent SM must actually *suspend to the runtime* (yield Pending) rather than block, so the executor can schedule the backgrounded task.

---

## Reconciliation note (RE-ENTRY CONTRACT step 3)
Phase 2/3's Option-B typeck guards (`WaitInsideLoop`, `LocalCrossesWait`) are SEPARATE from these three bugs — they descope wait-in-loop and let-crossing-wait to M3. The rework here is value-return + nested-SM + background-from-SM, none of which the Option-B guards touch. After the rework, re-decide whether any Option-B guard narrows or lifts (e.g. if frame-backed locals get done as part of the typed return slot work, `LocalCrossesWait` may lift). Wait-in-loop almost certainly STAYS M3 (needs the full loop-state transform). Decide explicitly at plan time — don't lift silently.

## What the P0 spike got wrong (graveyard seed)
P0 hand-wrote Rust mimicking the codegen and validated THAT (incl. Contract #4d SM-in-SM, #5 errors-cascade). The real emitted codegen diverges: no typed return slot, sync-bridge-everywhere instead of inline poll-and-yield. Lesson: spikes MUST compile real source through the real compiler; milestone success criteria MUST have an end-to-end test through `./target/debug/ynz` before any phase claims them met.
