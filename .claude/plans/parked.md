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
    exception explicitly. TRIGGER: Phase 2's design. **Status: ABSORBED by Phase 2's design (pending sign-off, `m8-p2-20260903-a1`)** — a literal built from named heap values is `Provenance::Reaches`, never transferable; literal elements as consuming sinks is deferred with the container-store class (`IMP-ownership.md` "Transfer", sinks). Probe confirmed `[a]` given away leaves `a` readable (`1 3`).
13. **`.copy()` admission must be ordered after `ynz_map_clone`.** WHAT: the send-arm admission
    treats `.copy()` as always fresh, but `map.copy()` today returns the receiver's own pointer
    through codegen's `_ => Ok(recv_val)` catch-all (`emit.rs:19360`). If the admission lands before
    Phase 4 step 3a, `wire.send(table.copy())` compiles and sends an alias — the diagnostic's own
    advice becoming the bug. WHY deferred: it is an ordering obligation, not a defect. COST: none if
    ordered; a real aliasing hole if not. TRIGGER: Phase 4 step 3a, which must land before any
    `.copy()` admission ships. **Status: CARRIED into Phase 2's design** — `.copy()` is `Fresh` only where `copy_is_independent(type)` holds (parity-tested against the codegen arms), so a `map.copy()` before `ynz_map_clone` lands is `Unknown` and refused rather than admitted as an alias. The ordering obligation on Phase 4 step 3a stands.
14. **`dynamic Contract` dispatch has no ownership checking at all.** WHAT: `check.rs:5391-5421` and
    `shapes.rs:24-29` — `ContractSigDef` carries no ownership modifiers, so a contract method with a
    non-`self` `give array<int>` parameter bypasses every ownership rule. A shape `self` cannot be a
    channel payload, so there is no direct send hole today. WHY deferred: it is the fourth instance
    of the class FRAGO 008 re-homed; Phase 2's whole-program answer should cover it by construction
    rather than as another enumerated site. COST: unknown until Phase 2's shape is decided. TRIGGER:
    Phase 2's design must state whether contract dispatch is covered or explicitly excluded. **Status: COVERED BY CONSTRUCTION in Phase 2's design (pending sign-off)** — `ContractSigDef` carries the AST's modifiers, the dispatch site threads the one `check_transfer`, `follows` checks modifier parity (`IMP-ownership.md` "`dynamic Contract` dispatch — covered by construction"). Probe: typeck accepts today, codegen ICEs `dynamic dispatch call sites not yet lowered in M4 P4` — zero runtime exposure.
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
    `bg_inferred` — Phase 5's `auto_arc` hint work is the likely first reader. **Status: ABSORBED by Phase 2's design** — the three spawn-arg recording sites share ONE recording function that derives the label (`Give`/`Copy`/`Channel`/`Arc`) from the truth, so the handle form no longer records `Copy` for a given binding (`IMP-ownership.md` "What typeck records and what codegen reads"). **DONE in Phase 5 (`m8-p5-20260904-a1`)**: `record_spawn_arg_ownership` in `check.rs` is the one function; the handle-form pre-record now records `Give` for a position the callee's signature declares `give` (and `Arc` for a group member), `Copy` otherwise.
17. **A real blast-radius instance the design's "zero instances" claim missed.** WHAT:
    `examples/primantis-orders/m6_errors.ynz:112-115` passes bare parameter `fig` as a receiver into
    `haulCircle(give self: Circle)` — today's silent consume at `check.rs:4617`. Any `give`-tightening
    rule converts it into a second diagnostic on that line. `error_galleries.rs:100` allows 7–14
    diagnostics so the count assertion probably absorbs it, but the `// WHY:` comment needs updating.
    WHY deferred: Phase 2 owns the rule that decides whether it fires at all. COST: trivial (one
    comment, possibly one count bound). TRIGGER: Phase 2's rule shipping. **Status: CARRIED into Phase 2's design** — named as the one known corpus instance in `IMP-concurrency.md` "How far the `give` obligation transits"; Phase 4 updates the `// WHY:` and the count bound.
18. **Fresh forms that are conservatively refused.** WHAT: `array<int>()` / `map<..>()` constructor
    calls and builtin-returned fresh arrays (`.sort()` and friends) are call results, so the admitted
    -form set refuses them even though they are genuinely fresh. WHY deferred: conservative and safe;
    widening is Phase 2's call, not a defect. COST: small. TRIGGER: Phase 2's admitted-form decision;
    record the widening or the deliberate omission. **Status: DECIDED in Phase 2 — deliberate conservative omission with a widening point** — constructor calls (`array<int>()`, `map<..>()`, `channel<T>()`) and `receive()` are `Fresh`; builtin method results default to `Reaches(receiver root)` via ONE `builtins` table (`builtin_method_returns_fresh`), widened per method with evidence, never by default. A user function's result is `Fresh` iff its `returns_fresh` fixpoint fact says so.

### Phase 2 round 2 — text-accuracy findings on the signed-off-pending design (added 2026-09-03, round close)

Round 2 closed CLEAN (0 blockers, two seats). Everything below is `should-fix`/`minor` against
`docs/internal/implementation/IMP-ownership.md` ("Transfer" / "Auto-Arc" sections) and
`IMP-concurrency.md`'s channel-close section. None changes a decision; all are wording that would
mislead a Phase 4 implementer. **TRIGGER for every entry: the round that records Patrick's Phase 2
sign-off applies them alongside the owed downstream plan edits** — the design must not reach Phase
4 carrying a known-false claim. COST: one docs-only executor round, no code. Source plan-id
`2026-07-04-v0-3-m8-concurrency-completion`.

19. **`bg_inferred` is NOT read only by the inlay-hint pass.** `IMP-ownership.md:263-264` (sink 3)
    and the round-2 audit entry say so; codegen's `is_heap_arg` gate at `emit.rs:16810-16828` reads
    `background_arg_inferred_ownership.contains_key(span)` — PRESENCE gates the heap upgrade, the
    variant is ignored (`fr23_uaf_planned_red.rs:27` says the same). "Declines to `Copy` silently"
    must state that a `Copy` entry is still RECORDED for every `Whole(name)` spawn arg; an
    implementer who records nothing hands the task a spawner-stack pointer (the fr23 class).
    Raised independently by both round-2 seats. **Status: APPLIED in m8-p2-signoff-20260903** —
    `IMP-ownership.md` sink 3 paragraph rewritten to state the PRESENCE-gates-not-variant reading and
    name `emit.rs:16810–16828`/`fr23_uaf_planned_red.rs:27` directly.
20. **The restated `background` inference omits the `BgOwnership::Channel` branch** that runs before
    liveness (`check.rs:1461-1469`). `IMP-ownership.md:263` as written infers `Give` on a
    `channel<T>()` binding not read after the spawn (constructor → `Fresh` → `Owned`), where the
    real rule shares it (`emit.rs:16752-16761`, `ynz_channel_share`). **Status: APPLIED in
    m8-p2-signoff-20260903** — sink 3 paragraph now states the `BgOwnership::Channel` branch claims
    channel-typed arguments first, before this rule sees them.
21. **Swapped labels, again.** `IMP-concurrency.md:925` calls `check.rs:1511` the user-function
    `give` path and `:4618` the `background` spawn path; it is the reverse (`:1511` is inside the
    `Expr::Background` liveness block `:1443-1515`; `:4618` is inside `check_arg_ownership`). Same
    class as parked item 1 — a third instance means Phase 4 should cite by FUNCTION NAME, not line.
    **Status: APPLIED in m8-p2-signoff-20260903** — "The hole" paragraph corrected and now cites
    `check_arg_ownership` by name, with a note that this is the third swap of this pair.
22. **`IMP-concurrency.md:958` still says origin/alias class are "set once at creation."** That is
    the round-1 sentence the blocker retired; the same paragraph cites the binding-event rule that
    recomputes both at every `Let`/`Assign`. Internal contradiction across the two IMP docs.
    **Status: APPLIED in m8-p2-signoff-20260903** — corrected to "(re)computed at every binding
    event," cross-referenced to `IMP-ownership.md` "Binding events, origin and alias classes."
23. **Function-type annotations do not exist.** `IMP-ownership.md:116` states the OPTIONAL-modifier
    rule over `let f: function(give Data) -> nothing`; `ynz-ast`'s `Type` enum has no function
    type and `parse_type_with_depth` (`parser.rs:900`) has no `function` arm. Drop the form.
    **Status: APPLIED in m8-p2-signoff-20260903** — the function-type-annotation clause dropped from
    "Signature-Level Declaration"; the OPTIONAL sentence now states there is no third position.
24. **`IMP-ownership.md:52`/`:116`/`:276` — the "REQUIRED on contract signatures" line was rewritten
    to OPTIONAL before a Patrick ruling** (packet item (j)), and `:276` narrates the rewrite.
    Either mark provisional pending the ruling or, once ruled, state current truth with a one-clause
    anchor and delete the narrative. **Status: APPLIED in m8-p2-signoff-20260903** — packet item (j)
    is ruled; the "dynamic Contract" section's narrative of the rewrite is trimmed to current state
    plus a one-clause anchor (`audit.md`'s SIGN-OFF record).
25. **`For` destructure names are desugared `Let`s, not `Stmt::For` bindings.** `IMP-ownership.md:169`
    (`For` row) and `:224` (`stmt_rebinds`): the parser prepends `Stmt::Let { value: __shape.field }`
    per destructured name (`parser.rs:2252-2274`; map form `:2417`), so they are `Let` events with
    `Reaches(__shape)` provenance and — unlike the loop variable — `Assign` to them is admitted
    (`check.rs:2436-2465`). Outcome stays sound (the `Let` row covers them); the AST claim and the
    predicate's scope are wrong. **Status: APPLIED in m8-p2-signoff-20260903** — the origin/alias
    table's `For` row split into a loop-variable row and a separate destructure-name row correctly
    described as a desugared `Let` event.
26. **Dead `--grep=m8-p1` pointer.** `IMP-concurrency.md:843`/`:1122` claim `git log --grep=m8-p1`
    finds `de631bf`; only `cd71f7f` carries the token. The bare SHA resolves; drop the grep half or
    cite the SHA alone. **Status: APPLIED in m8-p2-signoff-20260903** — both sites now cite `de631bf`
    directly (dropping the false grep claim) and `cd71f7f` via `git log --grep=m8-p1`.
27. **Minors, one line each:** six `ScopeEntry` constructors, all in `check.rs`, not seven
    (`IMP-concurrency.md:963`); "nine found live" at `IMP-ownership.md:130`/`:302` is not what
    `corpses.md` says (eight ran; the `dynamic` probe ICEs in codegen); `root_binding_name` has a
    pre-existing twin at `check.rs:11864` beside `effective_ownership.rs:658` — Phase 4 collapses it
    or the "defined once" claim rests on a twin; dated "probe, 2026-09-03" prose at
    `IMP-ownership.md:167,168,191,224` → "probe" + grep pointer; the cold-resume banner
    (`plan.md:17`) still names the round-1 executor; the roadmap row (`roadmap.md:456`) and a plan
    Future-Requirements row still cite the nonexistent `SCRATCH-audit-2026-07-11-memory-safety.md`.
    **Status: APPLIED in m8-p2-signoff-20260903** — all six minors fixed: the six-constructor count
    corrected; the "nine found live" claim rewritten to "eight probes... seven found a live hole";
    the `root_binding_name` twin flagged with a Phase 4 obligation; the dated probe prose replaced
    with a `git log --grep=m8-p2` pointer; the cold-resume banner rewritten (no longer names the
    round-1 executor alone); the roadmap's two duplicate rows and the plan's FR item 7(2) both gained
    a citation correction pointing at the code-direct premise instead of the uncommitted SCRATCH file.
28. **A second dead `git log --grep` pointer, minted by the fix that closed the first.** WHAT:
    `IMP-concurrency.md:1006` (sign-off fix round, `m8-p2-signoff-fix1-20260903`) cites
    `git log --grep=FRAGO-009`; no commit carried that token at write time — FRAGO 009 existed
    only in `audit.md` prose. Same class as item 26. WHY deferred: not deferred — closed at the
    producer instead of the instance: the Phase 2 phase-boundary commit carries `FRAGO-009`,
    `FRAGO-010` and `m8-p2-signoff` in its body so every pointer this phase minted resolves, and
    `.claude/corpses.md` gained the entry "Minting a `git log --grep` token no commit carries."
    COST: none further. TRIGGER: n/a. **Status: CLOSED by the Phase 2 boundary commit (conductor,
    2026-09-03) — verify with `git log --grep=FRAGO-009` after it lands.**

### Phase 3 — Loom substrate (round 2 close, 2026-09-04)

29. **The round-2 audit entry overclaims the deterministic test's failure mode.** WHAT:
    `audit.md` entry `m8-p3-fix1-20260904`, "Deviation, stated plainly" — says
    `ladder_holding_last_reference_purges_parked_send_before_channel_teardown` "cannot in any
    build" fail by assertion on the kind-2 order swap. `test-quality` ran the swap six times: 5
    SIGABRT (misaligned-pointer UB check), **1 failed by the test's own sequence assertion**
    (`[FILLER, PARKED]` vs `[PARKED, FILLER]`). The test catches the regression every run; the
    record's certainty about the mechanism is wrong and would mislead a reader judging whether the
    sanitizer lane is load-bearing. WHY deferred: `audit.md` is append-only; the correction is
    recorded here and in the Phase 3 boundary commit body rather than by editing the entry. COST:
    none further. TRIGGER: n/a — closed by this record. Source plan-id
    `2026-07-04-v0-3-m8-concurrency-completion`.
30. **`GLUE_SEQUENCE` has no guard against payload-pattern collision.** WHAT: the new
    `m6_pending_send_aba` test in `crates/ynz-runtime/src/lib.rs` filters a process-global
    `Mutex<Vec<i64>>` by two literal payloads (`0x5EED_F111`, `0x5EED_DEAD`) it asserts nothing
    else in the crate mints; a future test reusing either pattern corrupts this test's sequence
    with no diagnosable message. Stable across 15 × 16-thread runs today. WHY deferred: not a
    defect now; a hygiene guard. COST: small — a crate-level `const` registry of test payload
    tags with a uniqueness test, or per-test tagging as `loom_tests.rs` already does. TRIGGER: the
    next test that mints a glue payload literal in `ynz-runtime`. Source plan-id
    `2026-07-04-v0-3-m8-concurrency-completion`.

### Phase 4 — environment debt surfaced by green-check (2026-09-04)

31. **Workspace `cargo clippy --all-targets -- -D warnings` is red on pre-existing test-target
    lints CI never runs.** WHAT: `ynz-lsp/tests/{code_action.rs:172,176,202,206,completion.rs:807,
    regression.rs:180,rename.rs:178,inlay_hint_array_to_fixed_edit.rs:234}`,
    `ynz-numerics/{src/decimal128/ops.rs:695,tests/differential.rs:12,126}`,
    `ynz-diagnostics/tests/jargon_audit.rs:{69,97,236,563}`, `ynz-watch/tests/long_session.rs:117`,
    `ynz-registry/tests/consistency.rs:6`, and (found by the round-2 gate once clippy got past the
    earlier-aborting crates) `ynz-typeck/tests/{strings_typeck.rs ×15 incl. 14 non-snake-case test
    fns, builtins.rs:11, inlay_hint_passes.rs:6, iterables_typeck.rs:13, maps.rs:11,
    generics_typeck.rs:410, check.rs:1027}` + `ynz-typeck/src/independence.rs:{981,995,1009,1026,
    1117}` (unused `susp` ×5; all last changed at `3b7e6e9`, pre-M8), and (Phase 5's gate)
    `ynz-lsp/src/{code_action.rs:365,semantic_tokens.rs:345}` (unused imports inside `#[cfg(test)]`
    modules, blame 2026-05-21) — `manual_contains`, `unused_imports`, `len_zero`,
    `unnecessary_map_or`, `implicit_saturating_sub`, `while_let_on_iterator`, `single_match`,
    `dead_code`. None of these files are in Phase 4's diff (`git diff --name-only HEAD` confirms);
    clippy aborts at a different first-failing crate per run, so two green-check passes reported
    disjoint "NEW" sets — both wrong. `.github/workflows/ci.yml:78` runs `cargo clippy --workspace
    -- -D warnings` WITHOUT `--all-targets`, so CI has never seen a test-target lint. WHY deferred:
    out of this milestone's charter (test hygiene across four unrelated crates); fixing it inside a
    concurrency phase's diff would bury a 15-site mechanical sweep in a memory-safety commit. COST:
    one mechanical `executor-low` round (~15 one-line fixes) plus adding `--all-targets` to the CI
    clippy step — and the CI edit is Patrick's hard-to-reverse class, so it rides a confirm gate.
    TRIGGER: Phase 9 close-out's full-workspace gate, at the latest; earlier if any phase's
    green-check needs `--all-targets` clean to distinguish its own lints from the debt. Source
    plan-id `2026-07-04-v0-3-m8-concurrency-completion`.
    **Status: CLEARED at Phase 9 close-out (2026-09-04, `m8-p9-20260904-b1`).** `cargo clippy
    --workspace --all-targets -- -D warnings` is GREEN. Every listed site was fixed with a real
    one-line-to-few-line correction, never an `#[allow]`: `ynz-watch/tests/long_session.rs`
    (`match` → `if let`); `ynz-lsp/tests/{code_action.rs,rename.rs,completion.rs,regression.rs,
    inlay_hint_array_to_fixed_edit.rs}` (`manual_contains`/`len_zero`/unused import/
    `double_ended_iterator_last`→`filter_next`→`rfind`/`unnecessary_map_or`);
    `ynz-lsp/src/{code_action.rs,semantic_tokens.rs}` (dead `use super::*` / dead `LineTable`
    import in `#[cfg(test)]` modules — confirmed dead by reading every call site first, not
    assumed); `ynz-numerics/src/decimal128/ops.rs:695` (`neg_and_abs` was missing `#[test]` — a
    REAL bug, not lint noise: the test never ran; now runs and passes); `ynz-numerics/tests/
    {differential.rs,conformance/mod.rs,deterministic_vectors.rs}` (unused imports/vars, all
    confirmed genuinely dead by grep before removal); `ynz-diagnostics/tests/jargon_audit.rs`
    (redundant `use ynz_registry;`, `unnecessary_map_or`, `while_let_on_iterator`→`for`,
    `implicit_saturating_sub`×3→`saturating_sub`, and a genuinely-dead `site_count` variable
    superseded by `site_strings.len()` everywhere it mattered — removed, not renamed to
    `_site_count`); `ynz-registry/tests/consistency.rs` + `ynz-parser/tests/keyword_sync.rs`
    (redundant `use ynz_registry;`); `ynz-parser/tests/trivia.rs` (unused `Comment` import);
    `ynz-parser/tests/lex.rs` (`manual_contains`×8, `byte_char_slices`); `ynz-parser/tests/
    {error_recovery.rs,parse.rs}` (`useless_conversion`×3, `len_zero`×8); `ynz-typeck/tests/
    {strings_typeck.rs,builtins.rs,iterables_typeck.rs,maps.rs,inlay_hint_passes.rs,
    generics_typeck.rs,check.rs}` (unused `Type`/`SourceFileRegistry` imports, `len_zero`×2, and
    14 non-snake-case test function renames — e.g. `m7_string_toUpperCase_returns_string` →
    `m7_string_to_upper_case_returns_string` — identifier only, test bodies untouched);
    `ynz-typeck/src/independence.rs` (5 genuinely-dead `let susp = suspend_set(...)` bindings —
    `stmts_are_independent` never took a suspend-set parameter — confirmed by reading the
    function signature before deleting, and by NOT touching the other 9 `let susp = ...` lines
    in the same file whose `susp` binding IS consumed downstream); `ynz-fmt/tests/
    {proptest_idempotency.rs,semantic_roundtrip.rs}` (`useless_conversion`×5, `String::into::
    <String>()` no-ops). `.github/workflows/ci.yml` was NOT touched — `--all-targets` still rides
    its own confirm gate per this entry's original COST field.

### Phase 4 — pre-existing correctness bug surfaced by fix round 3 (2026-09-04)

32. **Repeated `.failed()` checks on one binding INSIDE an errors-capable function both evaluate
    true regardless of the actual failure state.** WHAT: reproduced by the round-3 executor
    (`m8-p4-fix3-20260904`) on the pre-round base `d0c46b3` via `git stash`, so it is independent
    of every Phase 4 change; surfaced while wiring the `.message`-after-`.failed()` guard because
    `resolve_ident`'s auto-propagation strips `ErrorsCapable` on first use inside an `errors`
    function (that half is fixed this round by the shared `restore_ec_receiver_ty` helper); the
    remaining half is a real logic bug in `errors` handling — a second `if (x.failed())` in the
    same function body takes the true branch on a value that did not fail. No existing fixture
    exercises `.failed()` from inside an errors-capable function, which is why it never showed.
    WHY deferred: outside Phase 4's charter (channel close / transfer rule); a fix inside this
    round's diff would bury an `errors`-semantics change in a concurrency commit. It is NOT
    memory-unsafe (wrong branch, not wrong pointer). COST: root-cause in `check.rs`'s errors
    propagation / `emit.rs`'s ok-word handling (likely a stale `ErrorsCapable` flag after the
    first check) — one focused round with a RED fixture first. TRIGGER: **Patrick's call at the
    Phase 4 boundary — hotfix on its own branch (the FRAGO 004 precedent) or a Phase 9 close-out
    item.** Whichever he picks, a RED fixture pinning the wrong branch is authored FIRST so the bug
    can't hide again. Source plan-id `2026-07-04-v0-3-m8-concurrency-completion`.
    **Status (2026-09-04): ROUTED by Patrick to the `errors`-surface hotfix branch, with items 33
    and 34 — after M8 closes (FRAGO 011).**
    **Status (2026-09-07, hardening Phase 3 step 3.5): LIVE, confirmed by re-running the exact
    round-3 shape on HEAD (`loadConfig() -> nothing errors` calling an always-succeeding
    `errors` function, then `if (raw.failed()) { print("marker1") }` followed by
    `if (!raw.failed()) { print("marker2") }`) — reproduced first on base `d0c46b3` (both
    markers print, matching the round-3 finding exactly, confirming the repro is faithful),
    then on HEAD (`2c624ce` + this session's C3/S1 fixes): STILL both markers print. C2's
    landing (`6be6773`) and this session's C3/S1 fixes touch none of the machinery below.
    **NOT C3's sibling — a distinct, deeper producer, named here so it is not silently folded
    into a fix that would not touch it.** C3 (`errors_failed_true_branch`/`errors_consumed`/
    `errors_success_narrowed` keyed by name instead of `alias_class`) is about WHETHER typeck
    ADMITS a read; this defect is about the RUNTIME VALUE `.failed()` computes on a SECOND read
    of the SAME binding, and reproduces with no shadowing anywhere in the program. Root cause,
    confirmed by reading (not inferred): `resolve_ident` (`check.rs`) auto-narrows an
    `ErrorsCapable` binding to its inner type on ANY read inside an `errors`-capable function —
    including the read that IS itself a `.failed()`/`.message` receiver — already self-
    documented at `EC_MEMBER_NAMES`'s doc comment ("resolve_ident's auto-propagation may
    already have narrowed a bare-ident receiver to its success type on THIS exact read").
    `restore_ec_receiver_ty` patches typeck's OWN dispatch (so `.failed()` isn't rejected as
    "unknown method on string") but does not stop `cg.expr_type` from recording the narrowed
    type for that specific `Ident` AST node. Codegen's `Expr::Ident` arm
    (`crates/ynz-codegen/src/emit.rs`, the `errors_capable_locals` block) reads that recorded
    type: on the FIRST `.failed()`'s receiver it takes the "already narrowed" branch (no error
    check at all — pulls field-1 bits directly) and REPLACES `cg.locals[name]` with the
    extracted success value, removing the binding from `errors_capable_locals`; the SECOND
    `.failed()`'s receiver then loads that success-typed value directly and hands it to
    `lower_errors_capable_method`'s `"failed"` arm, which unconditionally calls
    `.into_pointer_value()` and dereferences offset 0 as if it were the `{i64,i64}` result
    struct — reading the STRING's own bytes as a bogus error pointer. Two independent
    almost-correct pieces (typeck's dispatch-only restore; codegen's per-read narrowing cache)
    each assume the other closes the gap; neither does, for the SAME reason
    `authoritative-derivation.md` names: no single producer decides "is this specific read the
    checking read or a later one." COST: this is NOT a one-function fix like C3/S1 — it needs
    either (a) `resolve_ident` to skip auto-narrowing when the current read is itself the
    receiver of an EC-member dispatch (a lookahead `check_stmt_if`/`check_errors_capable_method`
    do not currently have), or (b) codegen's `errors_capable_locals` removal to preserve a
    pointer-shaped representation across repeat reads instead of an eagerly-extracted success
    value. Needs its own diagnosis-then-FRAGO cycle, not a fold into C3's fix. TRIGGER: the next
    `errors`-surface session; a RED fixture pinning the two-marker shape (given above) is that
    session's first commit, per this item's own established pattern.**

### Phase 4 — closed by ceiling (round 3, 2026-09-04); two blockers re-homed to the `errors`-surface hotfix

Patrick's ruling (FRAGO 011): Phase 4's fix loop closed at the three-round cap with two open
blockers that share one ancestor with parked 32 — **the `errors`-value field surface was never
finished end-to-end** (typeck typed `.message`/`.suggestions`/`.trace`/`.source` unconditionally;
codegen lowered none of them until round 2 added `.message`). None is a channel-close defect;
Phase 4 tripped over it through a REF example. All three ride ONE branch, `fix/errors-fields`,
after M8's milestone PR is up — the FRAGO 004 precedent. Each gets a RED pin FIRST.

33. **The `.failed()` guard is keyed by name, so a shadowed rebinding inside the guarded block
    inherits checked status.** WHAT: `check.rs:~619` `errors_failed_true_branch: Vec<String>`;
    `if (x.failed()) { let x = computeB(); print(x.message) }` compiles (`code-reviewer-high`,
    round 3, direct build) — the inner `x` was never checked. Exposure: codegen's `br`/`phi`
    defense (round 3) means the read prints `""` rather than dereferencing null — a compile-time
    rule with a hole, not a crash. WHY deferred: closed-by-ceiling; the fix (key the set by scope
    entry / invalidate on re-declaration — the same push/pop point one level deeper) belongs with
    its siblings. COST: small. TRIGGER: the `fix/errors-fields` branch, RED pin first.
    **CLOSED (2026-09-07, hardening Phase 3 step 3.3 / FRAGO 002 cluster C3).** RED pinned first
    (`message_before_failed_check_fires_on_a_shadowed_rebinding_inside_the_guard`,
    `crates/ynz-typeck/tests/diagnostic_template_parity.rs` — confirmed failing against the
    pre-fix `check.rs` by reverting the file to `HEAD` and re-running, then restored). Fixed at
    the producer named above: `errors_failed_true_branch`/`errors_consumed`/
    `errors_success_narrowed` now key on `ScopeEntry::alias_class` (binding identity), not the
    bare name — the SAME identity signal `Scope`'s use-after-give tracking already mints per
    binding event (`binding_event_origin`), threaded rather than re-derived
    (`authoritative-derivation.md`). `check_stmt_if` (`check.rs`), `resolve_ident` (`check.rs`),
    and `check_errors_field_needs_failed_check` (`check.rs`, the one shared gate both
    `Expr::MethodCall` and `Expr::FieldAccess` dispatch consult) are the three touch points; one
    fix, both call forms. Verified for the `for`-destructure case the plan named (desugared
    `Stmt::Let`s go through the same `binding_event_origin` path, no special-casing needed) via
    `a_for_destructure_binding_shadows_by_identity_not_just_by_type_mismatch`. Gallery trigger
    added to `examples/primantis-orders/v0_3_hardening_errors.ynz`.
34. **`.trace` / `.suggestions` / `.source` inside a `.failed()` check ICE** — `emit.rs:~19281`
    "not lowered yet (only .message)". WHAT: typeck admits all four siblings post-check
    (`EC_FIELDS_REQUIRE_FAILED_CHECK`), `REF-errors.md` documents all four, codegen has an arm for
    `.message` only. Pre-existing (they ICEd via `field_gep` before M8) and LOUD ("This is a
    compiler bug"), not silent — not a regression. Round 3's corpus sweep claimed all four and
    did not name this gap. WHY deferred: same ancestor, same branch. COST: three arms mirroring
    the `.message` `br`/`phi` shape + fixtures. TRIGGER: `fix/errors-fields`, RED pin first.
    **CLOSED (2026-09-07, hardening Phase 3 step 3.4 / FRAGO 002 singleton S1) — by REFUSAL, not
    by lowering.** RED pinned first (`ec_field_not_yet_available_is_rendered_from_the_registry`
    + siblings, same file — confirmed failing against the pre-fix `check.rs`, then restored).
    Decided against `no-duct-tape.md`: lowering `.trace`/`.source` needs new `array<Frame>`/
    `SourceLoc` shape-value construction from the runtime's already-captured trace data
    (`ynz_error_trace_len`/`ynz_error_trace_frame` exist; nothing in codegen builds a Yinz value
    from them) and `.suggestions` has NO producer anywhere in the compiler (the runtime's
    `suggestions_ptr`/`_len` fields are permanently null) — a separate, feature-shaped piece of
    work, not a producer-alignment fix. Refusing is loud, actionable, and honest; an ICE
    reached by a correct program is none of those. Fixed at the producer: `EC_FIELDS_REQUIRE_
    FAILED_CHECK` (a hand-written list) and codegen's raw `field_name == "message"` string
    check (the second, unbound list) are both replaced by ONE shared table,
    `ynz_typeck::errors_fields::{EcField, ec_field_lowering}` — exhaustive over `EcField`, no
    `_` arm, so a fifth field is a build failure in `ec_field_lowering` itself. A new
    `EcFieldNotYetAvailable` `[[diagnostic_template]]` + `DiagnosticKind` variant carries the
    three-slot refusal text. `errors_field_parity_tests` in `crates/ynz-codegen/src/emit.rs`
    mirrors `copy_parity_tests`'s `every_plan_matches_the_type_shape_its_emitter_destructures`
    binding. Three pre-existing tests that asserted the WRONG (pre-fix) behavior — corrected,
    not left red or weakened: `ec_method_{suggestions,trace,source}_resolves_in_ec_fn`
    (`crates/ynz-typeck/tests/check.rs`) and `m7_suggestions_method_returns_array_of_string`
    (`crates/ynz-typeck/tests/errors_typeck.rs`). Gallery trigger added to
    `examples/primantis-orders/v0_3_hardening_errors.ynz`.

### Phase 4 — final `test-quality` grade (2026-09-04, phase close)

Five revert-proofs run by the seat itself all failed loud (release-before-glue via loom + runtime
unit test; close-vs-send linearization; the alias snapshot via `give_twice_is_use_after_give` +
`a_class_consumed_before_the_call_is_not_reported_twice`; `NumberCell` `transfers_source`; the
byte-span renderer via `byte_spans.rs` + the gallery caret check). 0 blockers. Everything below is
`should-fix`, source plan-id `2026-07-04-v0-3-m8-concurrency-completion`.

35. **The two open blockers (33, 34) have NO RED pin in the tree.** WHAT: the hotfix branch
    `fix/errors-fields` would start with no failing test. The seat reproduced both live: `if
    (x.failed()) { let x = <fresh EC call>; print(x.message) }` compiles and prints `""`;
    `x.trace` inside a correct guard ICEs "This is a compiler bug". WHY deferred: Patrick routed
    both defects to the hotfix branch (FRAGO 011); the pins are that branch's FIRST commit, not
    this milestone's. COST: two fixtures, each asserting today's wrong behavior so the fix flips
    them. TRIGGER: the first commit on `fix/errors-fields` — before any fix lands.
36. **`m8_p4_chan_array_send_after_close_frees_the_refused_payload_once` cannot see a
    `release_taken_value()` regression.** WHAT: `v03_m8_channel_close.rs:~244` — the fixture
    builds `rows` inside the background task, so no ladder slot is at stake; the runtime unit test
    and loom model catch the revert, the driver fixture passes unchanged. WHY deferred: coverage
    exists one layer down; the fixture's WHY comment overclaims. COST: one sibling fixture passing
    `rows` as a `give` bg-arg, or a corrected comment. TRIGGER: the `fix/errors-fields` branch's
    test sweep, or Phase 9's gallery/fixture pass — whichever comes first.
37. **The three `same_call_alias_*` fixtures' WHY comments claim to lock the pre-call snapshot;
    they lock alias DETECTION only.** WHAT: `v03_m8_channel_close.rs:~418-442` — forcing the
    snapshot empty leaves all three green; the snapshot's real regression class (duplicate
    diagnostics on a legitimate consume-then-reuse) is caught by `give_twice_is_use_after_give`
    (`typeck/tests/check.rs:1450`) and `a_class_consumed_before_the_call_is_not_reported_twice`.
    WHY deferred: coverage exists; attribution is wrong. COST: comment fix. TRIGGER: Phase 9.
38. **The `.message` IR-level test scans the whole module for `select`.** WHAT:
    `v03_m8_channel_close.rs:~498-502` `!ir.contains("select")` — any unrelated `select` in the
    fixture's output flips it red for the wrong reason; the same test's `br i1 %ec_msg_failed`
    check shows the name-scoped pattern to reuse. WHY deferred: brittle, not wrong today. COST:
    scope the assertion to the `ec_msg_*` block. TRIGGER: Phase 9, or the first false red.
39. **Close-wakes-every-receiver is tested with exactly one receiver.** WHAT:
    `channel.rs:~280-287` `wake_recv_waiters()` drains the whole Vec; the only fixture
    (`v0_3_m8_p4_close_wakes_parked_receiver.ynz`) parks one — a `.pop()`-instead-of-`.drain(..)`
    regression goes uncaught. WHY deferred: contract is implemented; the multi-waiter case has
    no fixture. COST: one fixture parking two receivers on one channel, or a loom model. TRIGGER:
    Phase 9's fixture pass; the loom model is the better home if Phase 5's Arc work touches
    `recv_waiters`.
    Also: the four `ec_method_*_resolves_in_ec_fn` tests assert only the guarded form compiles —
    the refusal half exists only for `.message` (driver-level). Rides item 35's branch.

### Phase 5 — Auto-Arc emission (2026-09-04, `m8-p5-20260904-a1`)

40. **A shape with a `number` field SIGSEGVs when passed to ANY `background` spawn — pre-existing,
    on the shipped copy path.** WHAT: `shape Scene { name: string, width: int, height: int,
    scale: number }` passed as `background render(scene, results)` (a suspending callee, ONE spawn,
    no Auto-Arc involvement — the program emits zero `ynz_arc_*` calls and the one-spawn IR is
    proven byte-identical to the pre-Phase-5 compiler) dies with signal 11; the identical program
    without the `number` field runs. Repro: the probe in the Phase 5 audit entry (a two-spawn
    twin was authored as `m8_arc_number_field_declines.ynz` and DELETED because the fixture
    corpus sweep runs every fixture and this one crashes). Likely producer: the shape heap-copy
    at `prepare_bg_arg_for_ctx` (`ynz_alloc` + a struct load/store) copies the 16-byte `i128`
    field by value, but the task-side field read of a decimal128 goes through the
    `sm_number_param_set` / heap-cell indirection (v0.3-M6 FRAGO 006/007) that expects a
    POINTER — a load through i128 bits as an address. Unverified beyond the symptom. WHY
    deferred: out of Phase 5's scope (the Arc floor excludes `number` fields precisely, so the
    emission never touches this path); a fix belongs with the M6 number-arg marshalling. COST:
    small-to-medium (one probe with `--emit-ir` to confirm the producer, one codegen arm).
    TRIGGER: the post-M8 hotfix branch (FRAGO 011) or the next milestone touching
    `prepare_bg_arg_for_ctx`'s Shape arm — the crash is a silent-until-run miscompile on a
    plausible user program.
    **Status (2026-09-04): ROUTED by Patrick to its OWN hotfix branch, `fix/bg-arg-number-field`,
    NOW (FRAGO 012, the FRAGO 004 precedent) — running in a parallel worktree, M8 does not pause.
    Different ancestor from `fix/errors-fields`; not bundled. RED pin first.**

41. **`background` argument evaluation snapshots argument 0 before a later argument runs — the
    copy path and the Arc path agree with each other and disagree with a plain call.** WHAT:
    `f(scene, bump(scene))` with `bump(lend scene)`: a plain call reads the post-`bump` value
    through the pointer; a `background` spawn (explicit `scene.copy()` OR the inferred copy) hands
    the task the PRE-`bump` bytes because `prepare_bg_arg_for_ctx` materializes argument 0 before
    argument 1 evaluates. Found by the Phase 5 round-2 executor while pinning the Arc-range
    blocker; not an Arc defect — Phase 5's fix DECLINES sharing for that shape, and the copy path
    behaves the same as before M8. WHY deferred: a left-to-right evaluation-order question for the
    language design (`IMP-concurrency.md` "Ownership with Background Tasks" says nothing about
    when spawn arguments are evaluated relative to each other); not this milestone's charter.
    COST: a design ruling (evaluate all arguments, THEN copy/share; or document the snapshot
    order) plus one codegen reorder and a fixture. TRIGGER: the next design pass over `background`
    argument semantics, or a user report of the pre-`bump` value — whichever first; a gallery
    trigger would be premature before the ruling. Source plan-id
    `2026-07-04-v0-3-m8-concurrency-completion`.
42. **The give/copy inlay-hint walker renders `Expr::Call` arguments only; its Arc sibling also
    handles UFCS receivers.** WHAT: `inlay_hint_passes.rs::collect_background_ownership_hints_block`
    (`~:799`) enumerates Call args; `collect_auto_arc_hints_block` (`~:978-984`, same file)
    enumerates Call + MethodCall(receiver, args) over the same recorded-position map. Typeck's
    `background_spawn_call_form` records a UFCS receiver as position 0, so `background
    scene.render(r)` records a `Copy`/`Give` no hint renders. Pre-existing narrowness; Phase 5
    round 2 edited exactly this arm (added the handle form) and left the second position
    enumeration beside the first. WHY deferred: a hint gap, not a compiler defect; no user-visible
    wrong text, only missing text. COST: small — lift one shared position-enumeration helper both
    walkers call. TRIGGER: Phase 9's teaching-surface pass, or the next edit to either walker.
    Also minor: `queries.rs::find_transitive_share_violations` still takes `&declared_writes` as a
    parameter although the same map rides on the report — two handles, one map; collapse when
    touched. Source plan-id `2026-07-04-v0-3-m8-concurrency-completion`.

### Phase 5 — final `test-quality` grade (2026-09-04, phase close)

All five revert-proofs the seat ran itself failed loud (a dropped clone crashed the compiled
program before the IR count could fail; a skipped transient release showed as `alloc 6 != free
5`; (h) flipped failed the per-kind parity test; the old range re-admitted the stale-read group;
`number` admitted failed the unit pin). 0 blockers. Source plan-id
`2026-07-04-v0-3-m8-concurrency-completion`.

43. **No END-TO-END fixture pins the `number`-field exclusion from Arc sharing.** WHAT: revert 5
    (`arc_shareable` admits `Number`) fails only the unit pin `types.rs::every_excluded_field_kind_
    declines`; the driver suite stays 12/12. The two-spawn twin was deleted in Phase 5 because it
    crashes today (parked 40). WHY deferred: until `fix/bg-arg-number-field` merges back, the
    fixture can only pin a crash. COST: one `m8_arc_number_field_declines.ynz` through
    `assert_declined_fixture` (correct output, 0 `ynz_arc_*`). TRIGGER: the merge-back of the
    hotfix into this branch — author it in the same commit that merges, so the parked-40 crash
    becomes a named GREEN. **Status: DONE (dispatch `m8-tail-20260904-a1`)** — the hotfix
    (parked item 40) merged back into this branch; `m8_arc_number_field_declines.ynz` (two-spawn
    twin of `m8_arc_two_spawn_group.ynz` with an added `ratio: number` field) plus
    `number_field_excludes_the_shape_from_arc_sharing_and_declines_correctly` in
    `v03_m8_auto_arc.rs` pin the decline end-to-end: `assert_declined_fixture` asserts zero
    `ynz_arc_*` calls in the IR, exit 0, expected stdout, alloc==free. Proved non-vacuous by
    temporarily widening `arc_shareable`'s match arm to admit `Type::Number` — the fixture then
    failed loud with IR counts `(1, 2, 1)` instead of `(0, 0, 0)` — before reverting.
44. **No fixture exercises a caller write AFTER the group.** WHAT: condition 3 covers
    `first..=last`; a write after the last spawn is allowed and must not reach the tasks' block
    (a snapshot minted at `first`). If a future change makes the transient alias the caller's
    storage instead of copying (`emit.rs` Arc arm), the leak is invisible to every current
    fixture. COST: two spawns, then `scene.width = 100`; tasks report the ORIGINAL product,
    the caller prints the new value, IR `(1,2,1)`. TRIGGER: the next edit to the Arc arm or
    Phase 9's fixture pass.
    Minors (one line each): `ynz_run_counted` parses the counter file before checking the exit
    code, so a crashing child reports as a missing counter line; `emit_ir_no_optimize` has no
    watchdog; the hammer's `bridge.depth * 0` / `bridge.name` reads contribute nothing to the
    asserted total; the LSP label tests assert presence, not position; `m8_arc_rebind_boundary`
    covers `Stmt::Assign` only (no `let`-shadow / `for` arm fixture).

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

### Plan-level deferrals with no durable home outside the plan (added at Phase 9 close-out, 2026-09-04)

Five of the plan's eleven Future Requirements have no durable record once
`2026-07-04-v0-3-m8-concurrency-completion` moves to `.claude/planning/done/` at close-out — the
other six are homed in the roadmap Capability Ledger and/or `registry/features.toml`
`[[deferred_language_feature]]` entries and are NOT duplicated here (authoritative-derivation.md).
Source plan-id `2026-07-04-v0-3-m8-concurrency-completion`, `## Future Requirements / Revisit`.

45. **Fuzzing corpus backlog — CLOSED-BY, not a residual deferral (plan FR #6).** WHAT was deferred:
    "interesting failing/regression cases the structured fuzzer surfaces need a durable home (a
    saved corpus for replay, not discarded after each CI run)". Phase 8 shipped the mechanism,
    documented in `crates/ynz-driver/tests/fuzz_grammar/README.md`: every finding is reproducible
    from its seed alone (`YNZ_FUZZ_SEED=<seed> cargo test ... print_generated_program`, seed + full
    generated source embedded in the failure text), and a finding confirmed as a genuine miscompile
    is copied verbatim into `crates/ynz-driver/tests/fixtures/` under "Where an interesting case is
    promoted" — at which point the existing hand-written sweeps cover it forever, independent of any
    later generator revision. That closes the FR's own ask. Residual, tracked under FR #11/parked
    entry 49, not here: the two genuine defects Phase 8 found have NOT yet been promoted into
    `fixtures/` as RED pins, because their fix is itself deferred off-plan — promoting an
    unfixed-but-known bug as a fixture is a decision for whoever picks up FR #11, not a gap in the
    backlog mechanism itself.
46. **Panic-payload log asymmetry — `panic_payload_msg` private to `channel.rs` (plan FR #8, item
    (1)).** WHAT: the handle-side panic path logs a payload-less message while the channel-side logs
    the panic payload string, because the formatting helper is private to `channel.rs`. **Correction
    to the plan's own text:** FR #8 claims both its residuals "already carry fielded deferrals in the
    roadmap's own `audit.md`" — false for this item. A grep of
    `.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md` for `panic_payload_msg` and
    "panic-payload" returns zero hits; the only surviving record of item (1) is inside the
    already-archived `.claude/planning/done/2026-07-04-v0-3-m6-concurrency-hotfix/plan.md` (line
    ~1775) and its `audit.md` (line ~3259), neither of which is a durable home once this plan itself
    archives. (Item (2) — the duplicated `recv_waiters`/`record_recv_waiter`/`wake_recv_waiters`
    registry — IS correctly homed: `.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md`
    "2026-07-11 — Deferral: shared RecvWaiterRegistry extraction", confirmed by direct read; it is
    NOT re-parked here.) WHY deferred: cosmetic, log text only on a now-theoretical panic path; does
    not narrow the hang-closing guarantee; this plan writes no code on that path. COST: trivial
    (widen the helper's visibility, one call site). TRIGGER: the next milestone that touches
    `handle.rs`'s panic-reporting path, or any real panic there needing diagnosis.
47. **`background` arg escape door #4 — a ladder-owned array clone stored into an ALIASED outer
    container (plan FR #9).** WHAT is deferred: `stash(bucket: array<array<int>>, rows: array<int>) {
    bucket.add(rows) }` spawned with `background` — `rows` is heap-cloned and owned by the task's
    drop ladder, but `bucket` is an `array<pointer-elem>` bg arg passed through un-cloned by
    `prepare_bg_arg_for_ctx`'s fall-through arms, aliasing the parent's container; the task's clone is
    pushed into the parent's bucket and freed at task retire, leaving `bucket[0]` dangling (observed
    garbage counts and SIGSEGV across 5 runs). RED-pinned by
    `bg_arg_alias_container_add_is_a_known_uaf_red_pin` in `crates/ynz-driver/tests/integration.rs`
    against fixture `bg_arg_alias_container_add_red.ynz`, with two companion guards in
    `crates/ynz-driver/tests/cross_impl_consistency.rs`. WHY: the escape exists because the container
    is aliased rather than cloned or given a defined ownership — a design decision about what a
    `background` argument IS, not a runtime patch; hooking `ynz_array_push` to walk the ladder would
    put an O(descriptors) scan on a hot synchronous path to treat a symptom. The plan's own `give`
    guard (`ParamNeedsGive`) closes the channel-send instance of this door but explicitly does not
    reach the non-channel container-aliasing instance. COST to fix later: ~1 session — extend
    `prepare_bg_arg_for_ctx`'s per-type table with defined deep-copy (or share) semantics for
    pointer-cell element arrays and maps, matching `BgArgFreeKind` free arm, one fixture per element
    class, RED pin flipped to its correct-world assertions. TRIGGER: v0.3-M8's general ownership rule
    for `background` arguments (owned by whichever future milestone lands the scope-drop /
    drop-insertion design) deciding, once and for every heap type, whether a bg arg is cloned,
    shared, or given — or earlier if a user hits the dangling-container read in the wild.
48. **`.copy()` codegen catch-all silently aliases outside `Shape`/`array`/`map` (plan FR #10).**
    WHAT is deferred: `PostfixOpKind::Copy` lowering (`crates/ynz-codegen/src/emit.rs`) ends in
    `_ => Ok(recv_val)` — the receiver's own pointer — for `maybe<T>`, union, `fixed<T>`, `dynamic`,
    the same alias-no-op stub class already closed for `array` and (Phase 4) `map`. WHY not absorbed
    in this plan: a `.copy()` audit across the remaining types is a decision about what `.copy()`
    MEANS for each type (one-level? deep? refused?), belonging with the ownership/drop story, not
    channel close. **Narrowing already shipped (FRAGO 010, signed 2026-09-03) — this plan's residual
    scope is smaller than the original finding:** `provenance(expr).copy()` classifies `Unknown`
    unless `copy_is_independent(type)` holds (`IMP-ownership.md` "Classification" table), so a
    `.copy()` on one of these types is refused as a transfer (`TransferNeedsCopy`) rather than
    silently admitted as an alias for any value this plan's transfer rule reaches — the FR's
    remaining scope is auditing what `.copy()` on those types SHOULD mean, not whether transferring
    one of today's alias-no-op copies is caught. COST to fix later: small-to-medium — per-type
    decision + arm (or a typeck refusal with a three-slot diagnostic for types that must not be
    copied), one independence fixture per admitted type. TRIGGER: the first diagnostic or spec
    example that recommends `.copy()` on one of those types, or the future `background`-argument
    ownership rule (which must decide what a copy of each heap type is).
49. **Two genuine pre-existing runtime defects the Phase 8 owned-heap-channel fuzz widening
    surfaced (plan FR #11) — the ONLY durable record; NOT fixed inline per this plan's CCIR item 5 /
    risk R5.** Both are PRE-EXISTING on `main` (`ec014d8`), independently reproduced twice in clean
    worktrees, and are NOT M8 regressions. Neither is RED-pinned in the tree — no test fails today
    on account of either — but the evidence is NOT confined to planning text: committed
    doc-comment narrative in `crates/ynz-driver/tests/fuzz_grammar/mod.rs` describes both defects
    at the generator guards that suppress them (the `Builder::suspension_seen` reuse guard for (a),
    the `send_count`-versus-capacity floor for (b)), including the corrected symptom rates. What
    lives ONLY in this entry and the FR text is the minimal reproducing `.ynz` shape and the
    routing decision.
    **STATUS: CLOSED 2026-09-06 by plan `2026-09-04-v0-3-concurrency-hardening` Phase 3 step 3.1.**
    Both defects were ONE producer — `collect_crossings_in_stmts` never scanned the operands of a
    statement that itself suspends, so a pre-suspension local read only there got no frame slot and
    was read from an uninitialised alloca on resume (FRAGO 002, and FRAGO 003 for the fix). Both
    are now live regression locks in `crates/ynz-driver/tests/frago002_c1_c2_planned_red.rs` (pins
    A/D/G/J), and two 256-seed fuzz sweeps at different seed bases return zero findings with the
    generator's suppression guards deleted. **The pointer this entry used to carry — "read
    `mod.rs`'s guards first, they are the in-tree record" — is dead: those guards no longer exist,
    because the defect they hid is fixed.** Patrick's separate-chat routing is moot; it was fixed
    here instead.
    - **(a) A crossing-local heap-channel-send corruption.** WHAT: an `array<int>`/`map<string,int>`
      LOCAL declared BEFORE any suspension point in the same function and later `.send()`-ed into a
      channel AFTER that suspension reads back corrupted on receive — `RUNTIME ERROR: killed by
      signal 6 (SIGABRT)`, a null/misaligned pointer dereference inside `ynz_map_count`/
      `ynz_array_count`. Fires in the DEFAULT optimized mode. The SIGABRT is the MINORITY symptom: an
      `YNZ_FUZZ_PROGRAMS=256` sweep with the generator's protective guard removed produced 35
      findings, of which 28 were silent `MODE-DIVERGENT` wrong output at exit 0 (a heap address
      printed where a count belonged) rather than a crash — the silent divergence is the majority and
      the more dangerous half since nothing signals a failure. WHY not fixed here: this is the same
      frame-crossing/suspension-boundary hazard family the M3a/M3d/M3e/M3g twin-derivation corpses
      warned about (`authoritative-derivation.md`); diagnosing which choke point needs the fix (the
      channel-send transfer lowering, or the crossing-local frame-slot machinery itself) is real
      engineering, not a fuzzing-harness-round task. COST to fix later: a diagnosis session (read
      `crossing_local_names`/the channel-send lowering against the plan's exact repro) plus a fix of
      likely-small size once the choke point is identified. TRIGGER: the next milestone touching
      channel-send lowering or the crossing-local/suspension-frame machinery, or a real workload
      building an `array`/`map` before an I/O call and sending it afterward.
    - **(b) A capacity-forced-blocking channel send reads back garbage.** WHAT: NOT `number`-specific
      (the `int` variant shows it too), NOT deterministic (~17–30% bad-run rate across runs), NOT an
      arithmetic shortfall (bad runs print a heap address, i.e. an uninitialized-or-freed read, not a
      lost addend). Fires ONLY when a `background` producer is forced to actually BLOCK on a full
      channel buffer, and ONLY under `--no-optimize`/`--no-auto-parallel` (`-O0`) — the DEFAULT
      optimized mode measured 36/36 correct on the same shape. WHY not fixed here: this is the
      channel's blocked-send path under `-O0` (`crates/ynz-runtime/src/channel.rs`'s blocked-send
      retry/wake logic) — NOT `fr12`'s `number_to_heap_cell` marshalling, which is not on the `int`
      path at all — a runtime diagnosis, not a fuzzing-harness-round task. COST to fix later: a
      diagnosis session (the blocked-send retry/wake path in `channel.rs` under `-O0`, for both `int`
      and `number`) plus a fix of unknown size until diagnosed. TRIGGER: the next milestone touching
      the channel send/backpressure path, or a real workload whose producer blocks on a full channel
      under `-O0`.
    - **Open question, unresolved:** are (a) and (b) the same producer (a general "value crossing a
      suspension/blocking-send boundary" bug with two symptoms) or two independent ones? Bisection
      did not settle it. Per `root-cause.md`'s "cluster findings before fixing any," whoever picks
      this up should check that first before fixing either.
    - **Routing, per Patrick's ruling 2026-09-04:** fix in a SEPARATE chat, on its own branch off
      `main` — not owed by this plan and not a trigger this plan carries; recorded here as the
      routing decision, not as a deferred task with its own trigger.

### Phase 9 — cumulative-gate RED, pre-existing test infrastructure (2026-09-04, `m8-p9-20260904-b1`)

50. **`timed_out_program_leaves_no_descendant_process_running` fails under full-workspace
    contention and passes in isolation.** WHAT:
    `bounded_run_kills_the_whole_tree::timed_out_program_leaves_no_descendant_process_running`
    (`crates/ynz-driver/tests/cross_impl_consistency.rs`) failed 3 of 3 full
    `cargo test --workspace` runs during Phase 9's cumulative gate — the descendant process was
    still in `D` state (uninterruptible sleep, i.e. blocked in the kernel on I/O) 3 seconds after
    `killpg` — and passed cleanly every time the file was run on its own. The file is UNTOUCHED by
    Phase 9's diff (confirmed: it does not appear in the round's `git status`), and by the M8 branch
    generally; this is pre-existing infrastructure debt surfaced by the gate, not a regression.
    WHY not fixed here: it is a textbook instance of the corpse class
    [`.claude/rules/test-parallelism.md`](../rules/test-parallelism.md) already documents as OPEN —
    *a wall-clock budget calibrated on an idle machine* — where the same producer (a fixed poll
    margin, here 3s, that holds on an idle box and not under 16-way contention) has already killed
    four other tests in this repo. A process in `D` state cannot respond to a signal until its I/O
    completes, so the margin, not the kill, is what failed. That rule's companion obligation is
    explicit: liveness budgets in a parallel lane stay generous (an order of magnitude over the
    observed run) and performance assertions do not belong in a parallel lane. The Phase 9 executor
    was right to refuse to widen the margin without diagnosis — widening a timing budget to make a
    red go away, with no measurement behind the new number, is how this class keeps recurring.
    COST to fix later: small — one measurement of the observed teardown latency under real
    contention, then either a generous liveness budget derived from it (order-of-magnitude, per the
    rule) or a poll-until-gone loop with a generous ceiling instead of a fixed sleep; ~half a
    session including re-running the suite enough times to show the flake is actually gone rather
    than merely quieter. TRIGGER: the `cargo nextest` migration this repo already wants (the same
    contention that produced this red is what that migration makes permanent), or the next
    full-workspace gate that has to distinguish a real red from this one — whichever comes first.
    Source plan: `2026-07-04-v0-3-m8-concurrency-completion`, Phase 9 cumulative gate.

### Close-out gate — two detection gaps the cumulative green-check exposed (2026-09-04)

51. **The registry's affected-test lane names two of its THREE consumers — `ynz-tmgrammar` was
    never in it.** WHAT: `registry/features.toml` is the SSOT for user-facing feature
    inventories, and a diff touching it has, since FRAGO 014, carried a lane rule naming
    `jargon_audit` and `ynz-registry`. It has a third consumer: `crates/ynz-tmgrammar`
    regenerates `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` from the registry, and
    `grammar_snapshot.rs::committed_grammar_matches_generator_output` fails when the committed
    file drifts. M8 Phase 4 (`2be2244`) added three `[[deferred_language_feature]]` entries —
    `channel-auto-close-on-last-producer`, `background-handle-close`, `maybe-move-out` — and
    never regenerated the grammar, which had last been regenerated back in M6 (`c409557`).
    The drift then survived five phases and every affected-lane gate in between, because no
    lane ever named the crate that would catch it. Fixed at close-out (regenerated, snapshot
    test green), but the LANE RULE is the producer and it is still wrong. WHY not fixed here:
    the rule lives in this project's `.claude/` docs and the FRAGO-014 record, and rewriting a
    process rule at a milestone's close-out gate is the wrong moment — it wants doing
    deliberately, with all three consumers derived from what actually reads the registry rather
    than from memory. COST: small — enumerate the registry's real consumers once (grep for what
    reads `ynz_registry` and what a `build.rs` generates from it), then correct the lane rule in
    one place. TRIGGER: the next plan that edits `registry/features.toml`, or the next FRAGO
    that restates the lane. Source plan: `2026-07-04-v0-3-m8-concurrency-completion`, close-out.

52. **`cargo test --workspace` fail-fasts across test binaries, so the planned-RED file masks
    every target after it.** WHAT: this repo has a deliberate planned-RED test file
    (`crates/ynz-driver/tests/fr23_uaf_planned_red.rs`, 3 failures that are SUPPOSED to fail —
    they lock known defects and flip when fixed). Cargo stops at the first failing test target
    unless `--no-fail-fast` is passed, so a plain `cargo test --workspace` on this branch stops
    there and never runs the targets ordered after it. Directly observed at close-out: the first
    run stopped early and a second run with `--no-fail-fast` was needed to see the rest of the
    workspace — which is how the `ynz-tmgrammar` drift in item 51 surfaced, five phases late.
    Every gate here that shells `cargo test --workspace` without `--no-fail-fast` has therefore
    been reporting on a PREFIX of the suite while reading like a full-suite verdict. WHY not
    fixed here: the fix is one flag, but it belongs everywhere a gate is defined — the
    green-check agent's own command set, the `/execute-plan` per-phase lane, and `ci.yml` — and
    changing CI is separately confirm-gated. Correcting one and not the others would leave the
    same false-green live where it is least visible. COST: small — add `--no-fail-fast` to each
    gate definition, then confirm on this branch that the run reports MORE targets than it did
    before (proof the flag actually changed what was observed, not just what was typed).
    TRIGGER: before the next milestone's first cumulative gate; or immediately, if a green gate
    is ever again contradicted by a red found later on the same tree. Source plan:
    `2026-07-04-v0-3-m8-concurrency-completion`, close-out.
    - **Related, recorded here so it is not lost:** the close-out `plan-adherence` pass hit a
      SECOND pre-existing contention flake beyond entry 50's —
      `v0_3_m4_p3_cross_give_generic_not_over_rejected` — which passes in isolation and is
      already tracked in the roadmap's own `audit.md` and in M7's. Entry 50 names one flake;
      the real flake surface on this branch is at least two. Same class, same producer
      (`.claude/rules/test-parallelism.md`'s wall-clock-budget corpse), same trigger.

### Recovered from the deleted `.claude/todos.md` — M2-M7 concurrency deferrals with no surviving home (added 2026-09-05, plan 2026-09-04-v0-3-concurrency-hardening Phase 1)

`.claude/todos.md` was deleted in `1b83fcb` ("chore: remove state.md/todos.md — superseded by
the .claude/planning system") — 403 deletions, zero insertions, no migration. Six M2-M7 plans
still cite that file as the recorded home for a deferral; the home no longer exists. Recovered
from `git show 1b83fcb^:.claude/todos.md`. Every WHAT/WHY/COST/TRIGGER below is sourced from
that recovered text (never paraphrased from a summary), cross-checked against the current
tree where the item touches live code — two of the thirteen turned out to be already closed by
later, unrelated work (entries 59 and 64 below); that verification is recorded inline rather
than silently carried forward as still-live.

53. **`param_read_after_join` still declines any post-join param read on a CPU spike-host.**
    WHAT: a CPU spike-host that reads a parameter in a statement AFTER its group's join still
    declines to sequential lowering. Confirmed still live and unfixed: `param_read_after_join`
    (`crates/ynz-typeck/src/cpu_admission.rs`), consumed by `admitted_cpu_group` /
    `spike_cpu_candidates` (`crates/ynz-codegen/src/emit.rs`), and that codegen function's own
    doc comment still points at ".claude/todos.md" for the deferred fix — the exact dead
    pointer this section exists to close. The wrapper writes param slot 0 at the byte offset
    where the first CPU handle begins (`ynz_abi::SPIKE_HANDLE_BASE_OFFSET`), so the spawn's
    handle store clobbers the param's frame slot; a post-join param read reloads the handle
    pointer's bytes instead of the param's value — a silent wrong answer. A param used only in
    spawn args round-trips safely (loaded into a stack alloca before the spawn runs), so that
    narrower subset fires; any post-join read declines. WHY not fixed here: needs param-slot
    reservation past the CPU-handle region — the wrapper's param-store must write past the
    spike reserve, and the resume path's param-load must offset to match — real frame-layout
    machinery, not a gate flip. COST: param-slot reservation in the wrapper param-store + an
    offsetting resume param-load, per the original finding's own estimate — real but modest,
    scoped to this one interaction. TRIGGER: when param-read-post-join host parallelism is
    wanted. Locked by (still in tree): `v0_3_m3d_param_host_declines.ynz` (the read-after-join
    DECLINE) vs `v0_3_m3d_param_host_spawn_args_only.ynz` (the spawn-args-only FIRE). Source:
    plan `2026-06-11-v0-3-m3d-cpu-parallelization` (sub-slice 4e, 2026-06-15) +
    `.claude/todos.md` at `1b83fcb^`.

54. **Loop-body CPU groups decline to sequential at every placement.** WHAT: a CPU group
    inside ANY `for`/`while` loop body declines to sequential lowering, at every placement
    (top-level loop, loop-under-branch, inner-of-nested-loop) — `spike_nested_blocks`
    (`crates/ynz-codegen/src/emit.rs`) excludes `For`/`While` bodies from candidacy entirely,
    so a loop-body CPU group is never detected and never fires; the program runs
    byte-identical to `--no-auto-parallel`. WHY not fixed: a loop-body CPU group needs the
    loop index AND every loop-carried local frame-backed across the group's join (the state
    machine resumes inside the body without re-running the loop header), plus multi-level
    synthetic-index reservation for nested placements — none of which the spike path provides
    today. M3d's own sub-slice 4b PROVED two of three attempted partial-admit shapes either
    silently miscompile or hard-abort codegen (a group in a for-body nested under an `if`; a
    group in the inner of two nested for-loops), so ALL loop-body placements were declined
    rather than shipping a partial, hand-listed envelope. COST: a dedicated
    loop-placement-matrix slice — multi-level synthetic loop-index slots (outer + inner),
    loop-carried-local frame-backing, threaded through the spike spawn/poll lowering, and
    re-including loop bodies in `spike_nested_blocks` only once every placement (top-level
    loop, loop-under-branch, inner-of-nested-loop) is proven byte-identical + abort-free.
    TRIGGER: when loop-body CPU parallelism is wanted. Locked by (still in tree):
    `v03_m3d_nested_for_body_group_declines_byte_identical`,
    `v03_m3d_for_under_if_group_declines_byte_identical`,
    `v03_m3d_inner_nested_for_group_declines_byte_identical` — all 0 spawns / byte-identical /
    abort-free. Source: plan `2026-06-11-v0-3-m3d-cpu-parallelization` (sub-slice 4b,
    2026-06-14) + `.claude/todos.md` at `1b83fcb^`. **Note**: the roadmap at
    `.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md` (M3d section) had
    implied this was "owned elsewhere per the Capability Ledger" — no such ledger row exists;
    the roadmap text is corrected as part of this same Phase 1 pass to point here instead.

55. **A function with 2+ CPU groups declines ALL spiking, not just the extras.** WHAT: a
    function with two-or-more CPU groups (top-level plus nested, or two nested, in ANY source
    order) declines ALL spiking to sequential lowering — `admitted_cpu_group` /
    `spike_cpu_candidates` require exactly one CPU group across all depths
    (`count_cpu_groups_all_depths == 1`); the frame reserves exactly one group-0 handle/result
    slot region, so firing two groups would alias the first group's handles. The decline is
    order-independent (a nested-group-before-top-level function declines identically to
    top-level-before-nested) — this order-independence is itself the fix for an earlier
    corrupt-binary regression the order-DEPENDENT version produced. WHY not fixed: partial
    hosting (fire one group, decline the rest) or full per-group hosting needs handle/result
    slots keyed per (group-index, member-index), threaded through the spike spawn/poll
    lowering, plus a per-group fired-set replacing the current single-fire guard — real
    frame-layout machinery, not a gate flip. COST: per-group slot keying + fired-set, per the
    original finding's own estimate. TRIGGER: when multi-group CPU/mixed parallelism is
    wanted, or when the spike frame-slot reservation is next reworked. Locked by (still in
    tree): `v03_m3d_nested_multi_group_declines_byte_identical` (top-first) +
    `v03_m3d_nested_group_before_top_declines_byte_identical` (nested-first) — both 0 spawns /
    byte-identical. Source: plan `2026-06-11-v0-3-m3d-cpu-parallelization` (sub-slice 4b fix
    round, 2026-06-14) + `.claude/todos.md` at `1b83fcb^`. **Note**: same roadmap
    misattribution as entry 54 above — corrected in the same Phase 1 pass.

56. **Wide-value-EC (`number errors` / `Shape errors`) CPU-parallel join-bind dereferences a
    dead worker-thread stack pointer — a genuine UAF, dormant only because the return class is
    excluded from CPU admission.** WHAT: a non-SM `number errors` (or any wide-value-EC)
    callee, if ever admitted to a CPU group, returns its ok-word as a pointer to a
    worker-thread stack i128 (`is_number_errors_callee` / the Number arm of `to_i64_bits`,
    `crates/ynz-codegen/src/emit.rs`); the parent's join-bind (`bind_sm_result_and_flush`)
    would dereference that pointer AFTER the worker frame is dead. Confirmed still dormant,
    not live: `ec_inner_fits_cpu_result_abi` (emit.rs) explicitly excludes `Number` from the
    set of `T errors` inner types safe to carry in the CPU-result ABI's ok-word — its own doc
    comment states the exact reason ("number → dead-worker-stack pointer... that pointer
    dangles the instant the worker frame dies"), so no program can admit this shape to a CPU
    group today. WHY not fixed: the fix is heap-stabilizing the wide-EC ok-word (malloc the
    i128, store a heap pointer in the ok-word, free at bind, alloc==free accounting) —
    mirroring bare-`number`'s SM-wrapper heap-stabilization — or a wider CPU-result ABI; same
    staging-slot family as the `ec-wrapper-collect-on-completion` lift already SHIPPED for the
    `background`-handle path in v0.3-M4 Phase 2 (`registry/features.toml`), but NOT yet done
    for this CPU-parallel join-bind path. COST: 1-2 sessions, shared with any future wide-EC
    CPU-parallelism work — per M3g's own Future Requirements row E7. TRIGGER: when CPU
    parallelism for wide-value-EC returns is wanted. Source: plan
    `2026-07-01-v0-3-m3g-mixed-cpu-io-overlap` (Future Requirements row E7) +
    `.claude/todos.md` at `1b83fcb^` (M3d P3 sub-slice-4d gate, 2026-06-14).

57. **`spike_host_subset` probe vs. emit-time `suspends_with_promotions` disagree on
    host-calls-host chains — benign over-allocation, tradeoff genuinely unquantified.** WHAT:
    `spike_host_subset` (`crates/ynz-codegen/src/emit.rs`) probes each candidate against the
    EFFECTIVE suspend set, computed BEFORE spike-host names are unioned in; the emit-time
    re-probe in `lower_function_with_waits` instead reads `suspends_with_promotions` (effective
    ∪ spike-hosts). For a host whose post-pair statement calls ANOTHER host, the probe admits
    (that callee isn't yet a known-suspending name) while the emit-time gate declines (the
    callee is now in the host union) — the probe then reserves the 48-byte handle/result
    region for a host the emitter runs sequentially: a dead reserve. Confirmed still live: the
    function's own current doc comment names this "Probe/emit-time asymmetry (benign
    over-allocation; tracked residual)" in the exact same terms. WHY not fixed, stated plainly
    rather than left thin per the audit's own instruction: the over-allocation itself is
    confirmed benign (the embedded child sub-frame still fits; no aliasing, no corruption;
    alloc==free verified on a probe host chain), but the cost of leaving it unreconciled is
    genuinely UNQUANTIFIED — nobody has measured how common host-calls-host chains are in real
    Yinz programs, so "dead-reserve waste" has no known magnitude. Reconciling the two sets
    needs a fixpoint (removing a host can re-admit a function that declined only because that
    host was suspending, which can in turn re-decline a third function) — a single
    reconciliation pass is not enough, and a naive full-promoted-union probe risks
    reintroducing the M3d slice-1 union-poisoning class, so this is not an opportunistic fix.
    COST: a fixpoint reconciliation pass + adversarial fixtures re-proving no under-allocation
    is introduced — unscoped in sessions because the fixpoint's convergence behavior on a real
    host-chain corpus has never been characterized. TRIGGER: when the dead-reserve waste
    matters on a real large host-chain program, or when the spike host-set computation is next
    reworked. Source: plan `2026-07-01-v0-3-m3g-mixed-cpu-io-overlap` (referenced by the
    current `spike_host_subset` doc comment as a tracked residual) + `.claude/todos.md` at
    `1b83fcb^` (M3d P3 sub-slice-4a, 2026-06-14).

58. **`background` + CPU-group shutdown can panic a worker mid-join — benign, pre-existing,
    timing-dependent.** WHAT: `ynz_rt_shutdown` (`crates/ynz-runtime/src/runtime.rs`)
    force-aborts in-flight tasks at program shutdown; when a `background`-spawned task owns a
    still-mid-join CPU child, the abort can surface as a Tokio-worker panic (confirmed still
    present: `panic!("ynz runtime: CPU child task was aborted before it could produce a
    result")` in the non-panic JoinError arm of `ynz_rt_join_poll`). Confirmed pre-existing
    (reproduces identically at the base commit and after the M3d sub-slice-4d change originally
    suspected of introducing it), timing-dependent (~25% of runs in the original measurement),
    and benign — the program has already produced its correct output and exits 0 before the
    panic fires on the detached worker thread. WHY not fixed: no stable byte-identical Yinz-
    source fixture can be constructed for a timing-dependent shutdown race (tested via
    deterministic runtime unit tests instead of an integration fixture); the fix — graceful
    drain of in-flight background-task CPU joins on `ynz_rt_shutdown`, or a silent detach
    instead of an abort-arm panic when the program is already terminating — is runtime work
    crossing the `background` (M1/M3e) and CPU-group (M3d) boundaries, outside any single
    milestone's charter to date. COST: not sized in the original finding — a runtime-only
    change to `ynz_rt_shutdown`'s abort path. TRIGGER: when `background` + CPU-group
    composition is hardened (a dedicated background-CPU slice). Source: plan
    `2026-07-01-v0-3-m3g-mixed-cpu-io-overlap` (Future Requirements table row) +
    `.claude/todos.md` at `1b83fcb^` (M3d P3 sub-slice-4d verify-first, 2026-06-14).

59. **State-machine frame-header offsets, dual-defined with only a prose comment — CLOSED-BY,
    not a live deferral.** WHAT the audit flagged: general SM frame-header offsets
    (`sleep_handle`@8, `return_slot`@16) allegedly dual-defined across
    `crates/ynz-codegen/src/state_machine.rs` and `crates/ynz-runtime/src/runtime.rs` with only
    a prose "must stay in sync" comment and no compile-time binding — the exact twin-derivation
    class [`.claude/rules/authoritative-derivation.md`](../rules/authoritative-derivation.md)
    exists to prevent. **VERIFIED CLOSED, not live**: `crates/ynz-abi/src/lib.rs` defines
    `FRAME_OFFSET_SLEEP_HANDLE`, `FRAME_OFFSET_RETURN_SLOT`, and `FRAME_HEADER_SIZE` EXACTLY
    ONCE, with a doc comment stating the move happened "v0.3-M3g Phase 1 ... the same drift
    class sub-slice 4d closed for the spike-frame offsets." Both `state_machine.rs` and
    `runtime.rs` import all three constants from `ynz_abi` (grep-confirmed:
    `use ynz_abi::{FRAME_HEADER_SIZE, FRAME_OFFSET_RETURN_SLOT, FRAME_OFFSET_SLEEP_HANDLE};` in
    `state_machine.rs`; the same three names imported in `runtime.rs`). A repo-wide grep for a
    second, locally-defined constant of the same byte values (`FRAME_SLEEP_HANDLE_OFFSET`,
    `FRAME_RETURN_SLOT_OFFSET`, or any other local `pub const` at 8/16 for these fields) returns
    zero hits outside `ynz-abi`. The finding was true when `.claude/todos.md` recorded it
    (2026-06-15) and is FALSE today — v0.3-M3g Phase 1 closed it as a byproduct of its own
    frame-ABI consolidation, before M3g's own plan or Future Requirements table ever named it
    as a residual. **Ruling on fold-vs-standalone** (the question this dispatch posed): moot —
    there is no live risk left to fold into the roadmap's "Authoritative-derivation write-time
    guard" unscoped-capability row (`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md`
    ledger row 438/439) or to park as a standalone live deferral. Recorded here as CLOSED-BY
    rather than silently dropped, so a future reader who finds the same stale
    `.claude/todos.md`-sourced description elsewhere does not re-open a non-issue.
    **Correcting the dispatching brief's own framing**: this is NOT this plan's blocker —
    Phase 2/4 will find this specific frame-slot layer already load-bearing-safe when they
    reach it. Source: plan `2026-07-01-v0-3-m3g-mixed-cpu-io-overlap` Phase 1 +
    `crates/ynz-abi/src/lib.rs` (read 2026-09-05) + `.claude/todos.md` at `1b83fcb^` (M3d P3
    sub-slice-4d gate round 4, 2026-06-15 — the original finding).

60. **`options` (i8 tag) crossing `wait` still works by coincidence, not by an explicit
    classified arm.** WHAT: confirmed still true — `flush_var_slot_to_frame`
    (`crates/ynz-codegen/src/emit.rs`) dispatches crossing-local flush behavior on
    `cg.sm_crossing_scalar_set` / `_bool_set` / `_float_set` / `_decimal128_set` /
    `_errors_capable_set` / `_shape_embed_set`; there is no `sm_crossing_options_set` anywhere
    in the file (grep-verified, zero hits). An `options` value therefore falls into whatever
    generic path handles everything not explicitly classified, and small i8 tag values happen
    to round-trip through the i64 frame slot. WHY not fixed: correct-by-coincidence, not by
    design — a future change widening `options` or altering slot conventions could silently
    break it with no test catching the assumption until it does. COST: add an explicit
    `Type::Options` branch to `flush_var_slot_to_frame` (i8 zext-on-write / trunc-on-read,
    mirroring the existing bool arm), OR add an explicit comment + guard documenting why the
    else-path is safe for i8 tags today. TRIGGER: any change to options representation OR the
    frame-slot classifier, or a v0.3 follow-up hardening pass. Source: plan
    `2026-06-01-v0-3-m3a-suspension-codegen` (cumulative review, 2026-06-04) +
    `.claude/todos.md` at `1b83fcb^`.

61. **A NESTED mixed (fused CPU+I/O) group stays declined to sequential.** WHAT:
    `admitted_fused_group` (`crates/ynz-typeck/src/cpu_admission.rs`) is TOP-LEVEL ONLY by
    construction (its own doc comment) — a mixed group inside an `if`/`while`/`for`/`match`
    body never fuses, even where the pure-CPU path's nested-branch machinery would admit a
    pure-CPU-only group at the same position. WHY not fixed, scoped at authoring time per
    M3g's own disciplined-initiative rule 4: the existing pure-CPU nested-group frame-slot
    reasoning (`nested_group_member_path`, the branch-arm block-path descent) was proven
    against a SINGLE-KIND (CPU-only) reserve; a nested FUSED group would ALSO need to embed
    I/O child sub-frames inside that same nested block's byte layout, which has not been
    derived or fixture-proven. COST: ~1 session — extend the nested-branch admission +
    frame-layout machinery to a fused (dual-kind) reserve at depth > 0, mirroring how
    `cpu_group_slots_and_reserve`'s fused branch already does it at depth 0, plus adversarial
    fixtures for the newly-admitted nested+fused shapes. TRIGGER: a real workload needs a
    mixed CPU+I/O group inside a branch arm (no current M3g fixture is blocked — every fixture
    exercising fusion is top-level). Locked by (still in tree):
    `v03_m3g_nested_mixed_group_declines_byte_identical`
    (`crates/ynz-driver/tests/integration.rs`) + fixture
    `v0_3_m3g_nested_mixed_group_declines.ynz` — 0 spawns, byte-identical to
    `--no-auto-parallel`. Source: plan `2026-07-01-v0-3-m3g-mixed-cpu-io-overlap`, Future
    Requirements / Revisit table.

62. **`admitted_fused_group`'s `!f.params.is_empty()` restriction is coarser than
    `param_read_after_join`'s precision.** WHAT: the pure-CPU top-level branch tolerates SOME
    params (via the narrower "no post-join READ" gate, entry 53 above); the fused-group gate
    (`admitted_fused_group`, `crates/ynz-typeck/src/cpu_admission.rs`) declines ALL params,
    zero exceptions. WHY not fixed, a real safety tradeoff not a shortcut: the CPU
    handle/result reserve AND the fused group's embedded I/O sub-frames both depend on the
    same byte-32-relative `own_base` computation a param-host's param slots also use —
    narrowing to `param_read_after_join`'s precision for the fused+I/O-embedding case has not
    been re-derived (the I/O sub-frame layout interaction with param slots is a materially
    different shape than the pure-CPU case `param_read_after_join` was proven against), so the
    conservative "no params at all" bar was chosen for this first fused-group codegen consumer
    rather than risk an unproven narrower gate. COST: ~1 session — re-derive the byte-offset
    interaction for a param-host embedding both a CPU reserve AND an I/O sub-frame, prove it
    under `param_read_after_join`'s discipline, add adversarial fixtures for the newly-admitted
    param+fused-group shapes. TRIGGER: a real workload needs a parameterized mixed-group host
    (every current M3g fixture has zero params; none is blocked today). Source: plan
    `2026-07-01-v0-3-m3g-mixed-cpu-io-overlap`, Future Requirements / Revisit table.

63. **CPU-heavy code in a plain non-state-machine function has no cooperative-yield mechanism
    at all — COST genuinely unscoped.** WHAT: the back-edge poll-yield mechanism
    (`ynz_rt_check_preempt`, `crates/ynz-runtime/src/runtime.rs`, called from
    `crates/ynz-codegen/src/emit.rs`) only exists for loops inside state-machine
    (`wait`-containing) functions. A plain synchronous function's hot loop can never
    cooperatively yield this way; its only protection is the existing CPU-admission routing to
    the blocking pool, which is a heuristic (admission), not a guarantee. WHY not fixed: no
    cooperative-yield mechanism exists for non-SM functions in this language's design (per
    `docs/internal/implementation/IMP-no-function-coloring.md` — plain functions are, by
    design, synchronous and non-suspending); closing this residual needs either a new
    function-coloring-adjacent mechanism (a real design change) or provably tightening
    admission's classification — neither was in the optimizer-pipeline milestone's charter.
    COST: UNSCOPED — the source plan states this explicitly ("unknown until scoped — likely a
    dedicated design session revisiting `cpu_admission.rs`'s classification boundary"); no
    number is invented here. TRIGGER: a real, reproduced starvation incident traced to a
    non-SM CPU-bound function that admission misclassified, or the next milestone revisiting
    `cpu_admission.rs`/scheduler design. Source: plan `2026-07-04-v0-3-m7-optimizer-pipeline`,
    Future Requirements / Revisit #8.

64. **`array<boolean>` literal ICE — CLOSED-BY, not a live deferral, and NOT a concurrency
    defect.** WHAT the audit flagged: `array<boolean> = [true, true, true]` allegedly ICEs at
    LLVM-verify time (a raw i1 passed to the i64 `ynz_array_push`/`ynz_array_set` ABI),
    reproducing with or without a `wait` — confirmed NOT a concurrency defect even at the time
    it was found (2026-06-04, M3a cumulative review; the no-wait control was the proof).
    **VERIFIED CLOSED, not live**: `crates/ynz-driver/tests/fixtures/m5_p2_byval_bool_literal.ynz`
    (`array<boolean> = [true, false, true]` plus `.add()`/`.set()`/index-assign) is a committed,
    currently-green integration test (`m5_p2_byval_bool_literal`,
    `crates/ynz-driver/tests/integration.rs`) whose own WHY comment names this exact bug by
    description — "RED-today cell #1: the pointer ABI miscompiles boolean arrays outright — raw
    i1 passed to the i64 push/set ABI; LLVM module verify fails" — and asserts `code == 0` plus
    exact correct stdout. The by-value array element rewrite (`array_elem_src_ptr` /
    `zext_bits64` in `crates/ynz-codegen/src/emit.rs`, which zero-extends any sub-64-bit int
    generically before marshalling) closed this as part of v0.3-M5 P2's general array-ABI
    work — the same milestone the original finding named as its own natural trigger ("OR the
    `m3c-array-by-value` work... fix it there since it touches the same lowering"). Labeled
    plainly per this section's own instruction: this was never a concurrency defect; recorded
    here only because it was equally homeless once `.claude/todos.md` was deleted, and would
    otherwise have vanished silently. Source: `.claude/todos.md` at `1b83fcb^` (M3a cumulative
    review, 2026-06-04 — the original finding) + `crates/ynz-driver/tests/fixtures/m5_p2_byval_bool_literal.ynz`
    and `crates/ynz-driver/tests/integration.rs::m5_p2_byval_bool_literal` (read 2026-09-05 —
    the closing evidence).

65. **The `number` return ABI's "caller copies immediately" contract had no compile-time
    enforcement — CLOSED-BY, not a live gap.** WHAT the audit flagged: the bare-`number`
    (decimal128) return ABI allegedly returned a pointer to a callee-owned stack alloca with no
    compile-time enforcement that the caller copies its value before any intervening call could
    clobber the dead frame — a structural fragility, not a live bug, per the original
    2026-06-19 finding. **VERIFIED ABSORBED, not a live gap**: `abi_return_type` and its
    module-level doc comment "The dangling-stack-return fix (R9)"
    (`crates/ynz-codegen/src/emit.rs`) show `number` (precision ≤ 34) now returns `i128` BY
    VALUE (register-based — "no memory at all," per the comment), never a pointer to
    callee-owned memory; `maybe<T>` returns `{i64,i64}` by value; `Shape` returns its LLVM
    struct by value. `wrap_abi_call_result` is the ONE authoritative reception wrapper every
    C-ABI call site routes through (per `.claude/rules/authoritative-derivation.md`): it
    allocas a CALLER-OWNED slot, stores the by-value result into it, and yields the slot
    pointer — there is no longer a window where a returned pointer aliases dying callee memory,
    because the callee never returns a pointer into its own frame for these classes at all.
    This is the R9/FRAGO 005 fix shipped in v0.3-M7 Phase 3 ("eliminate ret-of-own-alloca on
    `maybe<T>`/`number` returns... via a caller-provided out-slot/sret or a by-value return").
    **Ruling**: this closes the general class — return-by-value structurally removes the
    timing dependency for `number`/`maybe`/`Shape` — so the specific gap named in the original
    finding no longer exists. NOT re-opening `errors`-capable wide-value returns here — those
    still carry a pointer into worker/staging memory and are the separate, still-live entry 56
    above. Source: `.claude/todos.md` at `1b83fcb^` (M3d Phase-5 gate, 2026-06-19 — the
    original finding) + plan `2026-07-04-v0-3-m7-optimizer-pipeline` (R9/FRAGO 005) +
    `crates/ynz-codegen/src/emit.rs::abi_return_type` / `::wrap_abi_call_result` (read
    2026-09-05 — the closing evidence).

66. **`fr23_generic_maybe_payload_spawn_receiver_reads_live_values` — SUPERSEDED, coverage
    migrated, not silently dropped.** WHAT is deferred: the GENERIC-container B' fixture
    (`v0_3_m7_fr23_generic_maybe_payload_spawn_receiver.ynz`, `let first = identity(m)` with
    `identity<T>(give T) -> T` producing an un-annotated `maybe<Cargo>` binding) and the test
    that ran it, both deleted. WHY, a real tradeoff: the test's original purpose was proving the
    B' admission arm (`bg_arg_is_materialized_shape_temp`'s `FieldAccess` arm) read
    `binding_ty_narrowed` — the concrete instantiated scope type — rather than a function table,
    so it stayed generic-safe by construction (Phase 9 security fix-round, 2026-07-18). FRAGO
    022's default-deny redesign made that specific proof moot: `Expr::FieldAccess` now falls
    through `bg_arg_is_provably_safe`'s trailing wildcard regardless of how its type was
    derived, so there is no table lookup left for the deleted test to regress-guard. Its fixture
    also stopped compiling under v0.3-M8 Phase 4's transfer rule (`TransferNeedsCopy`) once that
    rule shipped, and three independent attempts to construct a legal replacement — an owned
    `maybe<Cargo>` reaching a generic `give` parameter — all failed for documented reasons: a
    direct return is a type error, a `give` accessor has `returns_fresh` correctly propagate
    `Reaches` (refusing the copy), and `channel<Cargo>.receive()`'s element type is unsupported.
    The tradeoff is real, not "acceptable for now": there is currently no legal Yinz program
    that reaches this exact shape, so keeping the test would mean maintaining dead, uncompilable
    fixture code. COST to restore later: small and concrete — one fixture rewritten around
    whatever accessor `maybe-move-out` ships, and one test re-added with the WHY reasoning
    below, migrated back out of the two surviving B' locks. TRIGGER: the `maybe-move-out`
    `[[deferred_language_feature]]` (`registry/features.toml`, `IMP-ownership.md` "Deferred
    (four fields, packet item (g))") landing — at that point a consuming accessor on
    `maybe<T>` exists and the generic-container B' shape becomes expressible again. The B' class
    itself is NOT unguarded in the meantime: `fr23_red_maybe_payload_spawn_receiver_reads_live_values`
    and `fr23_sm_arm_maybe_payload_spawn_receiver_reads_live_values`
    (`crates/ynz-driver/tests/fr23_uaf_planned_red.rs`) carry the migrated reasoning and stay
    green. Source: dispatch `hardening-fr23fix-20260906-a1`, FRAGO 003
    (`.claude/planning/done/2026-09-04-v0-3-concurrency-hardening/audit.md`).

### Step 3.2 fix round — findings recorded rather than fixed (2026-09-07, `hardening-p3.2-fix1-20260907-a1`)

Raised by the review seats on commit `6be6773` (the one authoritative owned-copy operation).
Entries 67–69 are the performance seat's, recorded on the dispatch's explicit instruction;
entry 70 is a residual the `number`-cell reconciliation surfaced and could not close in scope.

67. **A nested container handed to a `background` task now costs one allocation per leaf item,
    per spawn — at a call site the user never opted into.** WHAT is deferred: nothing about the
    deep-copy ruling changes; what is recorded is that the ruling now also lands on
    COMPILER-INFERRED spawn copies, not only on a typed `.copy()`. `background eat(rows)` where
    `rows: array<array<int>>` and `rows` is read after the spawn is labelled `Copy` by the
    liveness inference, and the copy walks every item (`emit_array_elem_deep_copy`), where
    before it passed one pointer. WHY, a real tradeoff with a concrete cost: the alternative is
    the alias that produced the use-after-free this cluster exists to remove, and the ruling
    (Patrick, 2026-09-06) already weighed depth against speed and chose depth — Golden Rule 2
    over Golden Rule 10. The NEW information is only that the cost appears without the user
    typing anything, so the "if deep copying proves too slow, add an explicit cheaper operation"
    escape hatch is reached through an inferred site, where there is no source text to attach a
    cheaper name to. Measured order of magnitude, from the seat: 1,000,000 small allocations
    cost more than moving 10× the data in one memcpy, and `background` inside a loop is a
    shipped shape (`examples/`, the fuzz corpus). COST to fix later: medium — either a spawn-side
    "the callee only reads it" analysis that can share instead of copy (the Auto-Arc admission
    is the existing precedent and already answers exactly this question for shapes), or the
    named cheaper operation the ruling anticipates plus a way to select it at an inferred site.
    TRIGGER: a measured workload where a `background` spawn of a nested container inside a loop
    is the bottleneck — or Auto-Arc's admission being widened past `Type::Shape`, which would
    make the sharing answer available to containers for free.

68. **The spawn-cloned `map` is still released by nobody, and its trigger may be further out
    than "next phase."** WHAT is deferred: `emit_owned_copy`'s `MapClone` arm returns
    `BgArgFreeKind::None` for a `CopyMode::SpawnArg` copy, so a `map` argument the task received
    a clone of is never freed — the four-field deferral written on that arm. WHY: unchanged from
    that arm's own text (releasing it is a `BG_ARG_KIND_*` WIRE-FORMAT change the runtime retire
    ladder, the channel-side release rewrite, and both of their kind-enumerating parity tests
    read; shipping it inside a correctness fix would invert the ordering constraint C2 was
    required to satisfy). COST to fix later: unchanged — one wire constant, one runtime retire
    arm, one emitter arm, two parity tests. TRIGGER — **this is the part that is new**: that arm
    names "the scope-exit release pass" (Phase 4 of
    `2026-09-04-v0-3-concurrency-hardening`), and Phase 4 is itself flagged as possibly
    splitting into its own milestone. If it splits, this deferral lives materially longer than
    "the next phase" implies, so the second half of that arm's trigger — "or the first program
    that spawns with a `map` argument from inside a loop" — is the one that should be watched.
    Note the fix round of 2026-09-07 REDUCED this leak's rate rather than widening it: a
    `background eat(m.copy())` used to mint two map clones and free neither (measured: 16 allocs
    / 1 free), and now mints one (11 allocs / 1 free, identical to the no-`.copy()` spawn).

69. **`emit_array_elem_deep_copy` recurses with no depth cap.** WHAT is deferred: the
    element-copy loop calls `emit_owned_copy` on each item, which re-enters this function for a
    nested container, and nothing bounds the nesting. WHY: the recursion is bounded by the
    SOURCE type's nesting depth, which a program spells out by hand (`array<array<array<int>>>`)
    — it is compile-time recursion over a finite written type, not a runtime walk over data, so
    there is no runtime pathology to hit and no attacker-controlled depth. The seat found none.
    COST to fix later: trivial — a depth counter threaded through `emit_owned_copy` plus a
    teaching refusal past the limit. TRIGGER: a type-level recursion becoming expressible
    (a self-referential shape, `SCRATCH-future-self-references.md`), which would turn a bounded
    walk into a potentially unbounded one.

70. **An `array<number>`'s cells carry pointers into the producing frame, and no per-cell
    re-homing exists for a spawn.** WHAT is deferred: `elem_copy` classifies a `number` element
    as `ElemCopy::Inline`, which is correct for INDEPENDENCE (the cell's 8 pointer bits address
    storage nothing can change, so two cells reading it can never disagree — `Cg::array_elem_size`
    sizes the cell at 8, and `Cg::to_i64_bits` is what puts a pointer there). What is NOT closed
    is LIFETIME: that storage belongs to the frame that produced the number
    (`value_to_stable_bits` has no `Number` arm, so nothing re-homes it), so an `array<number>`
    handed to a `background` task carries the same dangling exposure with or without a copy.
    WHY: a copy cannot fix it — this is a property of how an `array<number>` is built, one
    producer upstream of the copy table, and the fix is a `Number` arm in the ONE stable-bits
    choke point plus a matching release story for the per-cell decimal cells, which is the same
    wire-format/release work entry 68 names. Fixing it inside the copy table would mean the
    copy table minting cells nothing frees. COST to fix later: small-to-medium — one
    `value_to_stable_bits` arm reusing the existing `number_to_heap_cell`, plus the per-cell
    release story. TRIGGER: the scope-exit release pass (which owns per-cell release), or the
    first reproduction — the review seat could NOT turn this into a failing program, so it is
    recorded as a named residual on `elem_copy`'s own comment rather than as a confirmed defect.
    Source: dispatch `hardening-p3.2-fix1-20260907-a1`, should-fix 4.

71. **A default-deny `Give` of a REFUSED type still shares the spawner's value.** WHAT is
    deferred: `prepare_bg_arg_for_ctx`'s refusal arm passes a union-typed `background` argument
    through untouched when typeck labelled it `Give`. That label is reached by three routes and
    only the ident routes consume anything, so `background eat(b.choice)` (a `FieldAccess`,
    default-deny `Give`, FRAGO 022) hands the task a value `b` still reaches. WHY: there is no
    copy to make — the shared table REFUSES the type outright, so the only alternatives are the
    pass-through that shipped or a new compile-time refusal at the spawn, and designing that
    refusal is the same "what does `background` do with a union" question the union work owns,
    not a corollary of a copy fix. This is pre-existing and UNCHANGED by the 2026-09-07 fix
    round, which narrowed the same arm for `map` (where a copy does exist) and left this one
    named instead of silently inheriting the old comment's false soundness claim. COST to fix
    later: small — extend `spawn_arg_refusal` to fire under `Give` for `NoIndependentCopy` too,
    plus the fixture corpus re-run for any program that spawns with a union argument today.
    TRIGGER: the first program that spawns with a union/`dynamic` argument and reads it
    afterwards, or the union `background` design landing — whichever comes first.
