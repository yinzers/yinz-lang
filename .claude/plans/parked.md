# Parked findings

Non-blocking findings (`should-fix` / `minor`) collected during plan execution. Each entry names
WHAT, WHY it was deferred, COST to fix later, TRIGGER that forces the fix, and the source plan-id.

Written by the `/execute-plan` conductor at round close, when line anchors are stable. Not a
backlog to groom — a record so a deferred finding is never a silent one.

---

## From `2026-07-04-v0-3-m8-concurrency-completion`

### Phase 1 — Channel Close design (rounds 1 and 2)

Every entry below was raised against the design text in
`docs/internal/implementation/IMP-concurrency.md`'s "Channel Close — End-of-Stream Semantics"
section. All are Phase 4's to absorb when it implements, unless stated otherwise.

1. **Two `scope.consume` sites are labelled backwards in the design.** WHAT: the design calls
   `check.rs:4596-4620` "the `background` spawn site" and `check.rs:1511` "the user-function
   `give`-parameter path"; they are the other way round — `check.rs:1498-1512` is the `background`
   statement-form give-inference path (no const/share refusals), and `check.rs:4591-4648`
   (`check_arg_ownership`) is the user-fn/UFCS parameter path where the const refusal (`:4602`) and
   share refusal (`:4611`) actually live. WHY deferred: the substance (mirror `check_arg_ownership`)
   is the correct choice; only the labels are wrong. COST: trivial, two label swaps. TRIGGER: before
   Phase 4 step 2 — an executor following the labels goes to the wrong site to find the refusal
   wording.
2. **The `maybe<T>` constructor the design names is dead code, and building it at `conduit_post`
   puts an alloca inside the consumer loop.** WHAT: `build_maybe_some` is `#[allow(dead_code)]`
   (`emit.rs:2382-2383`) with no live producer; both maybe-builders `build_alloca` at the current
   insertion point, and `conduit_post` (`emit.rs:12773`, `:12950`) sits inside the `while` body of
   the canonical consumer loop — so every iteration grows the resume function's stack by 16 bytes.
   `alloca_in_entry_llvm` (`emit.rs:2270`) exists for exactly this. WHY deferred: it is an
   implementation mandate, not a design decision. COST: small — mandate entry-block alloca in the
   design, honor it in Phase 4. TRIGGER: Phase 4's `maybe<T>` lowering.
3. **The "ONE owned-heap predicate" needs to be an enum, not a bool, and has no existing home to
   thread.** WHAT: neither candidate works — `is_trivially_copyable` (`types.rs:242`) mis-handles
   `string` for this purpose, and `is_mutable_heap_type` (`independence.rs:751`, private) returns
   `false` for `number`, which fr12 says must join the give set. A bool cannot feed codegen either,
   because `channel_drop_glue` needs the drop function (`emit.rs:15512-15513`). Reviewer's proposed
   shape: `pub enum ChannelElemDrop { None, Array, Map }` + `pub fn channel_elem_drop(&Type)` in
   `ynz_typeck::types`, typeck consuming `!= None` and codegen matching exhaustively — the
   compile-time link `authoritative-derivation.md` requires. Note `check_channel_construction`
   (`check.rs:4290-4295`) is then a THIRD listing of element kinds and must be derived from the same
   function or covered by a parity test. WHY deferred: authoring it is Phase 4's work. COST: small.
   TRIGGER: Phase 4 step 2.
4. **"Reuse the two refusal diagnostics with one wording change" is not a reuse as written.** WHAT:
   both are inline `format!`s inside `check_arg_ownership` (`check.rs:4602-4607`, `:4611-4616`) keyed
   on `fn_name`; swapping the WHAT-INSTEAD means extracting a helper with a caller-supplied
   instead-slot. WHY deferred: wording. COST: trivial — say "extract". TRIGGER: Phase 4 step 2.
5. **`param_ownership` needs `build.rs` validation or it drifts silently.** WHAT: the new registry
   schema field must be validated at build time — each value in `{share, lend, give}` (matching the
   AST `OwnershipModifier` names) and `len == param_types.len()`. Without both, a typo or a
   misalignment compiles and ships as hover text. WHY deferred: the field itself was ratified by
   Patrick (FRAGO 003); the validation is implementation. COST: small, two checks in
   `crates/ynz-registry/build.rs`. TRIGGER: whichever phase lands the `param_ownership` field.
6. **The consumption-cause change is larger than "the scope entry gains a cause".** WHAT:
   `is_consumed: bool` (`scope.rs:31,85`) becomes a cause carrying the channel binding name (to fill
   `{channel}` in the diagnostic); `Scope::consume(name)` gains a cause parameter; seven
   `ScopeEntry` constructors change; the two `!entry.is_consumed` guards (`check.rs:1510`, `:4617`)
   become "not already consumed by any cause". WHY deferred: feasible with no restructuring, but the
   design must list the signature change so Phase 4 does not bolt a parallel `Option<String>` beside
   the bool. COST: small. TRIGGER: Phase 4 step 2.
7. **The `Consumed` diagnostic template is DEAD DATA — the drift was never observable.** WHAT: the
   template (`features.toml:1599-1603`) and its supposed emitting code (`check.rs:3624-3629`)
   disagree in all three slots, but the emit site never attaches `DiagnosticKind::Consumed` at all —
   it builds a bare `Diagnostic::error`, and the checker's only `.with_kind` calls are `NotDefined`
   (`check.rs:5694`, `:11764`). Fixing this needs `.with_kind(Consumed)` at the site, a new
   `ConsumedBySend` variant with `kind_name`/`tag` arms
   (`crates/ynz-diagnostics/src/diagnostic.rs:36-58`, `:69`, `:88`), AND a parity test asserting
   every `[[diagnostic_template]]` `kind_name` names a real variant whose emit site renders the
   template — otherwise the next drift is silent again. WHY deferred: pre-existing, unrelated to
   this milestone's charter, discovered at the same site. COST: small-to-medium (the parity test is
   the real work). TRIGGER: Phase 4, which touches that exact site.
8. **`use-after-move` appears in the `Consumed` template's WHY** (`features.toml:1603`). WHAT:
   "move" is a banned NOT-term per `vocabulary.md`'s Quick Reference (`give` row). WHY deferred:
   one-word fix, rides item 7's reconciliation. COST: trivial. TRIGGER: same as item 7.
9. **`check.rs:4596-4620` is described as spawn-exclusive; it is a shared three-call-site helper.**
   WHAT: `check_arg_ownership` is called from `check_user_fn_call`, a monomorphized-generic call
   path, and the UFCS dot-call path (`check.rs:4799`, `:5115`, `:5445`). WHY deferred: directionally
   correct, imprecisely scoped. COST: trivial. TRIGGER: with item 1's label fix.
10. **Verbatim quote misattributed** (raised round 1, fixed round 2 — recorded closed for the
    record): "extend THIS site to union it in explicitly" was attributed to
    `CHANNEL_SUSPENDING_METHODS`'s own doc comment; it lives at `check.rs:3988-3992`. **Status:
    FIXED in round 2.**

### Open exposure carrying a cheap in-scope guard — flagged under `no-duct-tape.md`

11. **The `background-handle-close` deferral leaves a live window and names a cheap guard it does
    not take.** WHAT: a channel spawned from a non-binding expression
    (`background doubler(makeWire())` — confirmed admissible by probe, prints 42) leaves no spawner
    binding to close, so that task can never be told the stream ended — the exact hang class this
    milestone exists to remove, reintroduced for one spawn shape. `no-duct-tape.md` requires taking
    a cheap in-scope guard alongside a deferral when the deferred risk has live exposure before its
    trigger. Reviewer's proposed guard: one steering diagnostic next to `check.rs:2334-2344` when
    the first channel-typed argument of a handle spawn is not an ident — "bind the channel first —
    `let commands = makeWire()` — so you can call `commands.close()`". WHY deferred: the decision to
    omit `h.close()` itself is sound and stands; only the guard is outstanding. COST: small, one
    diagnostic arm. TRIGGER: Phase 4 — either take the guard, or the deferral must state why it is
    not cheap.
