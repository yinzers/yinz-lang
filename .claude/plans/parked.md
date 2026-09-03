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
   wording. **Status: ABSORBED in Phase 1 fix round 2 (`m8-p1-fix2-20260903`)** — labels swapped
   in the design's "The rule." paragraph and in Phase 4 step 3b.
2. **The `maybe<T>` constructor the design names is dead code, and building it at `conduit_post`
   puts an alloca inside the consumer loop.** WHAT: `build_maybe_some` is `#[allow(dead_code)]`
   (`emit.rs:2382-2383`) with no live producer; both maybe-builders `build_alloca` at the current
   insertion point, and `conduit_post` (`emit.rs:12773`, `:12950`) sits inside the `while` body of
   the canonical consumer loop — so every iteration grows the resume function's stack by 16 bytes.
   `alloca_in_entry_llvm` (`emit.rs:2270`) exists for exactly this. WHY deferred: it is an
   implementation mandate, not a design decision. COST: small — mandate entry-block alloca in the
   design, honor it in Phase 4. TRIGGER: Phase 4's `maybe<T>` lowering. **Status: ABSORBED in fix
   round 2** — `alloca_in_entry_llvm` mandated in the design's runtime-guidance bullet and Phase 4
   step 4b.
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
   TRIGGER: Phase 4 step 2. **Status: ABSORBED in fix round 2** — the enum shape is the design's
   stated predicate ("Which element types") and the compile-time link named in "Two mechanisms,
   one rule"; `check_channel_construction` derivation/parity is Phase 4 step 3b.
4. **"Reuse the two refusal diagnostics with one wording change" is not a reuse as written.** WHAT:
   both are inline `format!`s inside `check_arg_ownership` (`check.rs:4602-4607`, `:4611-4616`) keyed
   on `fn_name`; swapping the WHAT-INSTEAD means extracting a helper with a caller-supplied
   instead-slot. WHY deferred: wording. COST: trivial — say "extract". TRIGGER: Phase 4 step 2.
   **Status: ABSORBED in fix round 2** — the design and Phase 4 step 3b now say "extracted".
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
   the bool. COST: small. TRIGGER: Phase 4 step 2. **Status: ABSORBED in fix round 2** — the design
   specifies `consumed: Option<ConsumedBy>` (`Given | Sent { channel }`), the `Scope::consume`
   parameter, the seven constructors by line, and both guards.
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
   correct, imprecisely scoped. COST: trivial. TRIGGER: with item 1's label fix. **Status: ABSORBED
   in Phase 1 fix round 2 (`m8-p1-fix2-20260903`) — the design's "The rule." paragraph names the
   helper as the shared user-fn/UFCS path with its three call sites (`check.rs:4799`, `:5115`,
   `:5445`); the status line was omitted then and added in fix round 3 (`m8-p1-fix3-20260903`).**
10. **Verbatim quote misattributed** (raised round 1, fixed in fix round 1 — recorded closed for the
    record): "extend THIS site to union it in explicitly" was attributed to
    `CHANNEL_SUSPENDING_METHODS`'s own doc comment; it lives at `check.rs:3988-3992`. **Status:
    FIXED in fix round 1 (`m8-p1-fix1-20260903`)** — an earlier revision of this entry said "round
    2"; corrected in fix round 3.

### Phase 1 round 3 — findings re-homed or still owed (added 2026-09-03, at Phase 1 sign-off)

Phase 1's fix loop was closed by re-diagnosis (FRAGO 008): the ownership question moved to Phase 2.
The three round-3 BLOCKERS are recorded in Phase 2's own plan block as failing programs its answer
must defeat — they are NOT parked as accepted residual. What follows is everything else round 3
surfaced that has no other home.

12. **Nested freshness is only one level deep.** WHAT: the design admits array/map literals and
    `.copy()` as "fresh by construction", but `wire.send([a, b])` on a `channel<array<array<int>>>`
    allocates a fresh OUTER container whose cells still alias the `a` and `b` bindings live in the
    sender (`emit.rs:16914-16937` — pointer-cell arrays clone as pointers). No free-side hazard
    today because `ynz_array_drop` is shallow (`lib.rs:1354-1360`), but it is exactly the cross-task
    aliasing the rule exists to forbid. WHY deferred: it belongs to Phase 2's ownership answer, and a
    syntactic patch here is the very producer FRAGO 008 named. COST: small once Phase 2's rule
    exists — either restrict owned-heap channel elements with pointer-cell inner types, or state the
    exception explicitly. TRIGGER: Phase 2's design.
13. **`.copy()` admission must be ordered after `ynz_map_clone`.** WHAT: the send-arm admission
    treats `.copy()` as always fresh, but `map.copy()` today returns the receiver's own pointer
    through codegen's `_ => Ok(recv_val)` catch-all (`emit.rs:19360`). If the admission lands before
    Phase 4 step 3a, `wire.send(table.copy())` compiles and sends an alias — the diagnostic's own
    advice becoming the bug. WHY deferred: it is an ordering obligation, not a defect. COST: none if
    ordered; a real aliasing hole if not. TRIGGER: Phase 4 step 3a, which must land before any
    `.copy()` admission ships.
14. **`dynamic Contract` dispatch has no ownership checking at all.** WHAT: `check.rs:5391-5421` and
    `shapes.rs:24-29` — `ContractSigDef` carries no ownership modifiers, so a contract method with a
    non-`self` `give array<int>` parameter bypasses every ownership rule. A shape `self` cannot be a
    channel payload, so there is no direct send hole today. WHY deferred: it is the fourth instance
    of the class FRAGO 008 re-homed; Phase 2's whole-program answer should cover it by construction
    rather than as another enumerated site. COST: unknown until Phase 2's shape is decided. TRIGGER:
    Phase 2's design must state whether contract dispatch is covered or explicitly excluded.
15. **`refuse_closed` must drop the sender lock before calling glue.** WHAT: the design's new
    `None`-under-the-sender-lock arm calls drop glue; `channel.rs:496` holds a `Mutex<mpsc::Sender>`,
    and the existing convention at `:480-481` releases it first. The design does not say so. WHY
    deferred: implementation detail with a stated convention to follow. COST: trivial. TRIGGER:
    Phase 4 step 4.
16. **The handle form pre-records `Copy` unconditionally.** WHAT: `check.rs:2331` records `Copy` for
    ident spawn args even when a `give` consumed the binding; codegen clones either way
    (`emit.rs:16731-16738`), so it is not observable today — but any IDE hint reading `bg_inferred`
    would tell the user "copied" for a value that was given. WHY deferred: not observable until the
    muted-hint surface reads that field. COST: trivial. TRIGGER: whichever phase wires a hint over
    `bg_inferred` — Phase 5's `auto_arc` hint work is the likely first reader.
17. **A real blast-radius instance the design's "zero instances" claim missed.** WHAT:
    `examples/primantis-orders/m6_errors.ynz:112-115` passes bare parameter `fig` as a receiver into
    `haulCircle(give self: Circle)` — today's silent consume at `check.rs:4617`. Any `give`-tightening
    rule converts it into a second diagnostic on that line. `error_galleries.rs:100` allows 7–14
    diagnostics so the count assertion probably absorbs it, but the `// WHY:` comment needs updating.
    WHY deferred: Phase 2 owns the rule that decides whether it fires at all. COST: trivial (one
    comment, possibly one count bound). TRIGGER: Phase 2's rule shipping.
18. **Fresh forms that are conservatively refused.** WHAT: `array<int>()` / `map<..>()` constructor
    calls and builtin-returned fresh arrays (`.sort()` and friends) are call results, so the admitted
    -form set refuses them even though they are genuinely fresh. WHY deferred: conservative and safe;
    widening is Phase 2's call, not a defect. COST: small. TRIGGER: Phase 2's admitted-form decision;
    record the widening or the deliberate omission.

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
    not cheap. **Status: GUARD TAKEN in fix round 2** — as a compile ERROR
    (`HandleChannelArgNeedsBinding`, handle form only; a warning would leave the hang class live for
    one spawn shape while the fix costs one `let`), with three-slot text in the design's "Teaching
    text", a `[[diagnostic_template]]` entry, and a gallery trigger; Phase 4 step 3c ships it.
