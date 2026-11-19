---
name: Yinz Authoritative Derivation Protocol
description: "Applies when compiler or codegen code must agree with another pass on a derived answer: thread the one authoritative value, never re-derive it."
paths:
  - crates/ynz-codegen/src/**
  - crates/ynz-typeck/src/**
---

**When two or more code paths must agree on a derived answer, thread the same authoritative value into all of them. Never let a second surface derive its own "equivalent" answer.** Why: a copy that matches today diverges when one call site, value type or intrinsic changes and the twin does not. The failure is silent: wrong output or a wrong-sized frame, with no error at compile time.

Before computing an answer that another pass already computes (a suspend set, a frame size, an ABI predicate, a gate decision):

1. **Find the authoritative producer and consume its output.** Producers here include `base_suspends`, `crossing_local_names`, `locals_crossing_wait`, `flush_var_slot_to_frame` and the frame-size resolver. Why: one source cannot drift.
2. **If it is out of reach, plumb it there** (a parameter, a seeded memo, a query), never re-derive a local copy. Why: needing to plumb it is not a reason to fork it.
3. **If two predicates must stay separate, tie them together**: define one in terms of the other, or add a parity test that asserts they agree. Why: a later edit to one then fails loudly.

**Not this:** deriving an answer once, in the pass that owns it, is correct, and so is extending that one producer (a new arm in its dispatch). Forking a second producer is the bug.

**Mirror case:** a precise answer that is computed must be consumed. Otherwise record the coarse choice as a named, deliberate decision. Why: a consumer that quietly does the coarse unconditional thing loses the precision and no test fails.

**Specific cases:**

- **Answer ownership, escape, aliasing and liveness questions from `crates/ynz-typeck/src/effective_ownership.rs`, never by matching `Expr::` variants at call sites.** Before writing a whole-program predicate, grep `crates/ynz-typeck/src/` for an existing analysis. Why: a call-site match misses the cases the analysis handles and breaks when a variant is added.
- **In `check.rs`, look up a callee signature in `sig_table.fns`, then `generic_fn_table.fns` (`.or_else`), or reuse `bg_call_return_type_readonly` / `bg_ufcs_return_type`.** Why: a one-table lookup misses a generic callee, so its spawn argument is not admitted and its temporary is freed while the spawned task still uses it (a use-after-free).
- **Mirror an edit to the type match in `flush_var_slot_to_frame` in `reload_params_from_frame`, for every crossing local and loop variable, not just params.** Why: reload reads the slot in a layout the flush never wrote (a `Number` as a 2-slot i128 against an 8-byte i64), so values come back silently wrong after a `wait`.
