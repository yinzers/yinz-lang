---
name: "v0-3-m4-channels-arc-release"
plan-id: "2026-07-02-v0-3-m4-channels-arc-release"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["plan-producer-2026-07-02-m4", "plan-producer-2026-07-02-m4-r2", "plan-producer-2026-07-02-m4-r3", "plan-producer-2026-07-02-m4-r4", "executor-2026-07-02-m4-p0"]
created_at: "2026-07-02"
updated_at: "2026-07-02"
metadata:
  type: "plan"
---

# PLAN: v0.3-M4 — Channels + Auto-Arc + v0.3.0 Release

> **Correction pass (r2).** This version supersedes the first draft's body in place (same plan-id).
> Two defects fixed: (1) the composed-suspension risk (R5) the plan-reviewer flagged, now a
> build-blocking gate with its three confirmed trap doors named; (2) the may-block twin-derivation
> risk (R6) the second recon pass found — the fifth occurrence of the exact drift class
> [`authoritative-derivation.md`](../../rules/authoritative-derivation.md) exists to prevent — now
> the highest-priority P0 deliverable. Also corrected: the version bump (`0.3.0-m7` → `0.3.0`, NOT
> `0.2.x`), the CHANGELOG span (`m7..HEAD`), and every stale `check.rs` citation.
>
> **Correction pass (r3) — narrow, surgical.** One no-duct-tape violation fixed (Patrick's catch):
> the r2 draft deferred `ec-wrapper-collect-on-completion` behind a "communication-only handle"
> scope-narrowing. The design doc's own gating language (IMP-concurrency:475 — cost "landing WITH
> the `background-handle-form` feature"; :477 — trigger names `.send`/`.receive` as the collection
> mechanism) and the registry entry's own `ships_in = "v0.3-M4"` (`features.toml:1168`) make the
> copy-before-free fix required in-plan work. It is now risk **R8** and a build-blocking P2
> deliverable. Nothing else in the plan changed.
>
> **Correction pass (r4) — narrow, additive.** Two plan-reviewer third-pass findings fixed
> (verdict: sound + nearly complete, 0 blockers), nothing else touched: (1) the **R5×R8
> composition cell** — a collected `-> T errors` background child that suspends on a full-channel
> `send()` mid-execution (R5's scenario) AND is then collected via the copy-before-free path at
> completion (R8's scenario) — was covered by neither the R5 composed fixture nor the R8 matrix
> individually; it is now a dedicated build-blocking cell in P2's R8 matrix. (2) R8 had **no
> dormant override pre-positioned** (R6 and R5/R1/R2 each carry one); dormant override #3 now
> arms if R8's P2 proof REDs.

## 1. Situation

### Terrain (landscape) — recon-confirmed 2026-07-02 against the live codebase (second, deeper pass)

- **Runtime ABI is real and reusable, not a rebuild.** `crates/ynz-runtime/src/runtime.rs` ships the
  poll-based suspension substrate: `ynz_rt_spawn` (I/O pool — fire-and-forget; return value
  explicitly discarded at `runtime.rs:699`, so no handle-returning variant exists),
  `ynz_rt_spawn_blocking` / `ynz_rt_spawn_blocking_joinable` (M3d, `Box<CpuJoinHandle>`),
  `ynz_rt_join_poll` (real forwarded waker — but **genuinely one-shot**: drops its box on first
  Ready at `runtime.rs:1308`; a second poll is a dangling-pointer read), and
  `ynz_rt_async_sleep_create/_poll` (boxed Tokio `Sleep` + real endpoint future + forwarded waker —
  **the no-fabricated-waker discipline channel send/recv must mirror**).
- **The may-block set is DEFINED TWICE with no compile-time link** — `M2_MAY_BLOCK_INTRINSICS` at
  `crates/ynz-typeck/src/intrinsics.rs:24` AND independently at `crates/ynz-codegen/src/emit.rs:819`,
  with scattered consumers at `may_block.rs:916`, `cpu_admission.rs:806`, `check.rs:2595`/`8339`,
  `emit.rs:627`/`3418`/`6726`. This is the exact twin-computation-drift class that shipped silent
  miscompiles in M3a, M3d, M3e, and M3g (commit `bb2281d`, the M3g AAR). Channel send/recv must join
  the may-block set — making M4 the FIFTH milestone touching this duplicated list — and channel ops
  are **methods on a value, not named free-function intrinsics**, so the name-keyed model may not
  even express them. This is risk **R6**, resolved at P0 before anything builds on it.
- **`CpuJoinHandle` is structurally wrong for a communicating child.** Its contract is a
  synchronous, run-once-to-completion blocking-pool closure (`runtime.rs:1129-1133`). A suspending,
  channel-communicating child cannot be modeled on it (trap door 1a, risk R5).
- **The frame header has NO spare slot for a channel future.** The single `sleep_handle` slot
  (`crates/ynz-abi/src/lib.rs:18-39`) is type-punned as `*mut Pin<Box<Sleep>>` by BOTH cancellation
  drop paths (`runtime.rs:602`, `runtime.rs:663`) — reusing it for a channel future is a
  type-confused free (UB) on cancellation (trap door 1c). Growing the header ripples through the
  const-asserted `FRAME_HEADER_SIZE`/`SPIKE_HANDLE_BASE_OFFSET` pair (`ynz-abi/src/lib.rs:57-61`)
  and M3e's cross-module `FrameLayout` serialization.
- **Closest prior art for the composed problem:** M3d/M3g's fused-group composed-poll mechanism at
  `emit.rs:8827` — one shared continuation re-drives every live CPU handle and pending I/O sub-frame
  per resume, never a blocking join. It solved the M3g CPU+I/O deadlock class; it does NOT directly
  reuse for M4's shape (a handle child is long-lived and bidirectional, not run-to-completion), but
  it is the mandatory study reference for P0's spike design.
- **Channels are 100% new — and the Tokio `sync` feature is NOT enabled.**
  `crates/ynz-runtime/Cargo.toml:23` lacks the `sync` feature; enabling it is a required,
  easy-to-forget P0 step. No `tokio::mpsc` anywhere in the workspace (only `crossbeam-channel` in
  `ynz-lsp`, unrelated LSP transport).
- **The share/lend-across-`background` reject is real — corrected citations.** The
  `Expr::Background` analysis block starts at `check.rs:2253`; share-reject at `check.rs:2275-2280`
  (message line 2277); lend-reject at `check.rs:2287-2292` (message line 2289). The roadmap's
  `check.rs:1216` is STALE (that region is now the background give/copy *inference*,
  `check.rs:1183-1197`). **Pre-existing gap under the new boundary:** the reject only fires when the
  callee resolves via `sig_table.fns.get(name)` (`check.rs:2269-2270`) — non-ident callees and
  unresolvable names skip silently. The auto-Arc boundary (R3) must account for this gap, not just
  the resolvable-callee happy path.
- **The handle-form lift site is a locked compile error** at `check.rs:1331-1342` ("Background
  handles will land in v0.3") — P2 lifts and re-types it.
- **No kernel-mode gate exists for channel ops.** The existing `wait`/`background` kernel gates sit
  at `check.rs:2223-2233`; nothing analogous covers channel operations (risk R7 — P1 adds the gate).
- **No runtime telemetry exists** to detect a deadlock/hang in the field — a shipped composed-
  suspension bug is a silent, undiagnosable hang for users. Build-time hostile fixtures are the
  compensating control; the observability gap is parked in Future Requirements with a trigger.
- **Registry: net-new machinery.** Zero `[[lint_rule]]` entries repo-wide (no schema, no parser, no
  LSP wiring — the entry-KIND is fully absent, not protocol-only). `channel_capacity` and `auto_arc`
  muted-hint domains absent (11 existing `[[muted_hint_domain]]` entries, `features.toml:2053-2145`,
  none channel/Arc-related).
- **LSP inlay wiring is mature but tint-less.** `crates/ynz-lsp/src/inlay_hint.rs` fires 8 domains +
  2 protocol-only stubs via the established pattern (typeck hint-pass fn → LSP import →
  registry-sourced hover). Hints today carry label+tooltip only — **no red-tint/color-variation code
  path exists**, so `auto_arc`'s cautionary red-tint is very likely net-new LSP rendering work, not
  a style flag (see §3.1 recorded decision).
- **Cross-impl oracle** (`crates/ynz-driver/tests/cross_impl_consistency.rs`,
  `YNZ_NO_AUTO_PARALLEL=1`) is mature — but it has **never gated a layout transform**: false-sharing
  padding is the first (SoA would have been, now deferred to M5). P4 must prove the gating, not
  assume it.
- **Adversarial gate pattern established** (M3d/M3g): `*_declines.ynz` + hostile fixtures, the
  DECLINE→FIRE flip, pool-exhaustion stress, alloc=free via `YNZ_ALLOC_COUNTER_OUTPUT`.
- **Version truth:** workspace version is `0.3.0-m7` (`Cargo.toml:21`, the M3d tag) — NOT `0.2.x`.
  M3f and M3g are merged to `main`, un-tagged, no CHANGELOG entries. Demo + gallery: latest gallery
  is `v0_3_m3g_errors.ynz`; M4 adds `v0_3_m4_errors.ynz`.

### Weather (external constraints)

- **Open-ended time** — no external deadline; budget phases against the work, not a date.
- **Solo project, no external stakeholders** (confirmed); pre-v1.0 breaking-change latitude
  (ADR-versioning). `trading-v4` mounts `target/release` but is not a Yinz-source consumer.
- **New dependency surface:** the `sync` feature of the already-bundled Tokio
  (`ynz-runtime/Cargo.toml:23`) — a feature-flag addition, not a new toolchain; license posture
  unchanged (MIT, already shipped in `libynz_rt.a`).
- **`--kernel` mode:** no scheduler exists there; channel/handle/auto-Arc must compile-error (gate
  is NEW work — R7), matching the existing `wait`/`background` gates at `check.rs:2223-2233`.

### Friendly forces

- **Depends on (both shipped, merged to `main`):** v0.3-M3b (I/O inline-poll suspension model) and
  v0.3-M3d (joinable-handle ABI + the `emit.rs:8827` fused composed-poll mechanism — the mandatory
  study reference for R5, even though it doesn't directly reuse).
- **Folds in:** M3f + M3g, both merged un-tagged — Patrick's call: fold into the final `v0.3.0`
  tag; no standalone `-mN` tags.
- **Higher intent:** last milestone before `v0.3.0`. Auto-SoA is DELIBERATELY split to v0.3-M5
  (documented deferral); this plan must not let it creep back.

### Assumptions (each marked; verify before relying)

1. `verified` — Share/lend reject present at `check.rs:2275-2280`/`2287-2292` (block starts 2253);
   roadmap's `check.rs:1216` stale; silent-skip gap for unresolvable/non-ident callees at
   `check.rs:2269-2270`. Confirmed by second recon pass.
2. `verified` — `ynz_rt_join_poll` one-shot (`runtime.rs:1308`); `CpuJoinHandle` run-once contract
   (`runtime.rs:1129-1133`); `ynz_rt_spawn` fire-and-forget (`runtime.rs:699`). Handle-form is a
   genuinely new C-ABI surface.
3. `verified` — `sleep_handle` is the frame header's only handle slot (`ynz-abi/src/lib.rs:18-39`),
   type-punned by both cancellation drop paths (`runtime.rs:602`/`663`);
   `FRAME_HEADER_SIZE`/`SPIKE_HANDLE_BASE_OFFSET` const-asserted (`ynz-abi/src/lib.rs:57-61`).
4. `verified` — `M2_MAY_BLOCK_INTRINSICS` twin-defined (`intrinsics.rs:24`, `emit.rs:819`) with
   scattered consumers and no compile-time link.
5. `verified` — Tokio `sync` feature absent (`ynz-runtime/Cargo.toml:23`); `[[lint_rule]]` kind,
   `channel_capacity`, `auto_arc` all absent from the registry; no red-tint path in `ynz-lsp`; no
   kernel gate for channel ops; no runtime deadlock telemetry.
6. `verified` — Workspace version `0.3.0-m7` (`Cargo.toml:21`); M3f/M3g merged un-tagged.
7. `unverified` — The independent-Tokio-task + real-endpoint-futures + forwarded-wakers design is
   waker-sound for the composed scenario. Verified by CODE READING only (channel endpoints wake both
   sides independently, unlike the passive-handle/one-shared-continuation shape behind the M3g/4c
   deadlock) — **not yet by execution. The P0 spike is the proof; HARD GATE.**
8. `unverified` — Whether dropped-receiver `send()` maps to Tokio `SendError` directly or needs a
   Yinz-level `errors` wrapper. **Resolved at P0.**
9. `unverified` — Whether `channel` needs a `[[keyword]]`/type-registry entry alongside
   `array`/`map`/`fixed` or is a pure typeck-level built-in constructor. **Resolved at P0.**
10. `unverified` — Whether the seq-cst opt-in surface (IMP-no-function-coloring: "final naming TBD")
    fits M4 or defers via `[[deferred_language_feature]]` (roadmap pre-authorizes). **Resolved at P0.**
11. `unverified` — Whether growing the frame header (a second handle slot) is needed at all, vs.
    keeping channel endpoint futures OUTSIDE the frame in the handle/runtime object. **Resolved by
    the P0 spike** — the spike's design deliberately avoids frame-header reuse (trap door 1c); if it
    proves a header slot IS needed, the `FRAME_HEADER_SIZE`/`FrameLayout` ripple is named P2 work.
12. `unverified` — At execution time, `git tag`/`git log` confirm the CHANGELOG span is `m7..HEAD`
    (do not trust a lazy "since last tag" default). **Verified at P6.**

### Risk Assessment (scored via `~/.claude/docs/reference/REF-risk-engine.md`, default code-domain anchors — no project override file exists)

Risk set = union of orchestrator-handed (R3–R7) + first-draft agent-found (R1, R2 — retained; a
handed set never suppresses agent-found hazards) + user-named (R8 — Patrick's r3 no-duct-tape catch,
tagged per the risk-input contract: user-named risks carry domain knowledge). Scored
authoritatively against the ACTUAL designed
solution below; cells picked per the axes anchors, tiers by the fixed 4×5 lookup only. No Floor B
class applies (no money/PII/security-injection/irreversible-op — compiler work, reversible via git,
pre-release). Severity II on R1/R2/R3/R5/R6/R7 is the silent-miscompile / silent-hang blast radius:
recoverable but expensive, matching the M3-series historical pattern.

| Risk | Prob | Sev | Initial | Mitigations (bucket) | Residual | Gate |
|------|------|-----|---------|----------------------|----------|------|
| **R6 — may-block twin-derivation drift.** `M2_MAY_BLOCK_INTRINSICS` twin-defined (`intrinsics.rs:24` + `emit.rs:819`, 7+ scattered consumers); this exact class shipped silent miscompiles in M3a/M3d/M3e/M3g (direct evidence → A). M4 must extend the set with channel ops (methods-on-a-value, which the name-keyed model may not express) — a fifth hand-extension of both lists is the banned pattern. | A | II | **EH** | **B1 unification** (prob −2): the two definitions become ONE authoritative suspension-source classifier (shared definition consumed by typeck AND codegen; shape extended beyond a name list to classify channel methods). Precondition: single source, all consumers threaded. Proof: the unification diff + grep showing zero remaining independent definitions. **PLUS B2 parity/RED test** (prob −1): a build-blocking test that FAILS if any consumer's view diverges from the authoritative source (permanent tripwire, defense-in-depth). Proof: the failing-test-first commit. | **M** (A→D) | recorded |
| **R5 — composed-suspension deadlock.** A `background`-spawned child suspends on `channel.send()` backpressure WHILE the parent polls `.receive()` on its handle. Novel composition, zero coverage, three CONFIRMED trap doors: (1a) modeling the child as a run-once `CpuJoinHandle` closure (`runtime.rs:1129-1133`) → circular wait; (1b) polling one-shot `ynz_rt_join_poll` twice (`runtime.rs:1308`) → dangling read; (1c) storing a channel future in the type-punned `sleep_handle` slot (`ynz-abi:18-39`, drop paths `runtime.rs:602`/`663`) → type-confused free on cancellation. No runtime telemetry → a shipped bug is a silent undiagnosable hang. | B | II | **H** | **B1 architectural elimination** (prob −2): independent joinable Tokio task + REAL endpoint futures + forwarded wakers (the `ynz_rt_async_sleep_poll` discipline), endpoint futures owned by the handle/runtime object — never the frame header; avoids 1a/1b/1c by construction (endpoints wake both sides independently). Precondition: design executes the composed scenario through the REAL compiler. Proof: the P0 runtime spike (spawn + send-on-full + parent-receive, actual `ynz run`, never a hand-written Rust model — the M2-spike false-ACCEPT lesson). **PLUS B2 composed hostile fixture** (prob −1): build-blocking fixture reproducing exactly this scenario, GREEN, gating P2 completion (M3d/M3g DECLINE→FIRE pattern). Proof: fixture file + CI run. | **L** (B→E) | pass |
| **R1 — channel send-on-full blocks a thread** (individual path — the M2 `block_on` corpse class, distinct from R5's composition). `send()` on full must suspend the task via the state-machine protocol, never a synchronous blocking call. | B | II | **H** | **B1 architectural elimination** (prob −2): send/recv route EXCLUSIVELY through poll-yield state-machine suspension; zero synchronous blocking calls in the emitted path. Precondition + proof: grep-audit of the emitted path + the P0 spike + timing-verified suspension through the real compiler. **PLUS B2 hostile fixtures** (prob −1): full-channel / closed-channel / never-drained fixtures, build-blocking. Proof: committed fixtures GREEN. | **L** (B→E) | pass |
| **R2 — handle-form substrate wrong-model** (never-received handle deadlock; one-shot-join misuse; `-> T errors` child return-staging). No handle-returning spawn exists (`runtime.rs:699`); handle drive must be poll-based with real endpoint futures. | B | II | **H** | **B1 correct-substrate design** (prob −2): new C-ABI handle over an independent joinable Tokio task + channels wired into the spawned frame — never a `CpuJoinHandle` wrapper, never a re-polled `join_poll`. Precondition + proof: the P0 spike + grep-audit. **PLUS B2 never-received / pool-exhaustion hostile fixtures** (prob −1, build-blocking, DECLINE→FIRE). Proof: fixtures GREEN + alloc=free. | **L** (B→E) | pass |
| **R8 — EC copy-before-free on handle-collected `-> T errors` results** (user-named, r3). The standalone EC wrapper reconstructs the `{err, ok}` struct from the frame's return slot and then `free_frame`s; for wide ok-values the ok-word points INTO the freed frame (IMP-concurrency:465). Safe today ONLY because fire-and-forget discards the result — P2's collecting handle makes the copy-before-free path reachable for the first time. An unguarded implementation is a use-after-free / stale-ok-pointer read (silent-wrong class); the surrounding EC-wrapper machinery is historically fragile (M3f same-callee staging-slot clobber, IMP-concurrency:473). New code on frame-free timing, zero coverage → B; memory-safety silent-wrong → II. | B | II | **H** | **B1 static-keyed collection design** (prob −2): the copy decision keys on the COMPILE-TIME spawn form, never runtime collection state — bare `background f()` keeps today's discard path byte-for-byte; `let h = background f()` ALWAYS copies the ok-value to a HANDLE-OWNED heap buffer before `free_frame` (buffer freed exactly once, at handle drop). Eliminates the collected-vs-discarded runtime-tracking hazard and the dangling-ok-pointer class by construction (a never-received handle wastes one bounded heap copy — safe, never dangling). Precondition: no runtime conditional-on-receive copy path exists; buffer lifetime = handle lifetime. Proof: P2 implementation diff + grep-audit + alloc=free including handle-drop/buffer-free. **PLUS B2 RED-repro fixture matrix** (prob −1, build-blocking): collected vs. fire-and-forget × `-> T errors` ok/error paths × receive-before/after-completion timing, PLUS the composed R5×R8 cell (r4): a collected `-> T errors` child that suspends on a full-channel `send()` MID-execution, then finishes and is collected via copy-before-free — the frame must survive the channel suspension AND then be freed exactly once at collection (the frame-lifetime/free-timing interaction neither R5's fixture nor the base matrix covers alone); collected values byte-correct; no dangling ok-pointer; no double-free; fire-and-forget path unchanged. Proof: committed matrix, RED→GREEN. | **L** (B→E) | pass |
| **R3 — auto-Arc boundary exactness.** False-Arc = silent cross-thread data race; false-reject = broken program (score the worse: II). Extending a working typeck arm (C), but the silent-skip gap for unresolvable/non-ident callees (`check.rs:2269-2270`) sits directly under the new boundary. | C | II | **H** | **B2 exhaustive RED-repro fixture matrix** (prob −1, build-blocking): share/lend/give × background-boundary × Arc-required, BOTH directions, INCLUDING the unresolvable/non-ident-callee edge cases. Proof: committed matrix, RED→GREEN. | **M** (C→D) | recorded |
| **R7 — channel op in a non-suspending context** (`--kernel` build, or any context with no state machine to host the suspension — no kernel gate exists today, unlike `wait`/`background` at `check.rs:2223-2233`). | C | II | **H** | **B1 compile-time reject** (prob −2): a kernel-mode gate for channel ops matching the existing pattern, PLUS channel ops classified as suspension sources via R6's authoritative source — so CPU-pool admission (`cpu_admission.rs`) can never admit a channel-using closure, and every non-kernel context hosting a channel op is a state machine by construction. Precondition: gate + classification both live. Proof: gallery trigger fixture + an admission-decline fixture. | **L** (C→E) | pass |
| **R4 — release-fold hazard.** M3f + M3g merged un-tagged; a lazy tag-diff CHANGELOG misses them. Reversible pre-push, cosmetic if caught. | C | IV | **L** | Gate-only explicit P6 step: verify with `git tag`/`git log` that the CHANGELOG span is `m7..HEAD` and demonstrably includes M3f + M3g + M4. | **L** | pass |

**Gate summary.** No residual lands HIGH/EX-HIGH as designed → no active override, and I sign
nothing. Three **dormant contingent overrides** are pre-positioned per REF-risk-engine (written,
unsigned, inactive unless their triggers fire — the signature is Patrick's, never mine):

```text
RISK OVERRIDE (DORMANT #1 — pre-positioned, not armed) — contingent residual: HIGH
  Risk:                     R6 (may-block twin-derivation drift)
  Arming trigger:           P0 concludes true unification is architecturally infeasible — the
                            concrete candidate reason being the methods-on-a-value vs. name-keyed
                            model mismatch forcing structurally separate representations in typeck
                            and codegen that cannot consume one shared classifier. B1 step zeroes;
                            only the B2 parity test (−1) remains: A→B, lookup(B,II) = HIGH.
  Why not mitigable to LOW: with the lists structurally separate, only the tripwire test guards a
                            class with four confirmed prior silent-miscompile occurrences; no
                            further frozen-catalog pattern applies without changing the architecture.
  Accepted by:              <blank — explicit human signature required if armed>
  Date:                     <blank>
  Trigger to revisit:       any subsequent milestone touching the may-block set (M5 is already known
                            to), or the architecture change that makes unification feasible.

RISK OVERRIDE (DORMANT #2 — pre-positioned, not armed) — contingent residual: HIGH
  Risk:                     R5 / R1 / R2 (composed-suspension deadlock class)
  Arming trigger:           the P0 spike REDs (composed scenario deadlocks or hits a trap door), OR
                            the grep-audit finds ANY synchronous blocking call in the channel
                            send/recv or handle path, OR any hostile / never-received /
                            pool-exhaustion / composed fixture deadlocks. B1 step zeroes (no proof →
                            step 0); residual returns to HIGH and the plan HALTS for redesign.
  Why not mitigable to LOW: if a blocking call or circular wait is structurally required, the
                            poll-yield architecture has failed and the M2 block_on-HALT class has
                            returned; no B2/B3 control recovers a thread-blocking deadlock in the
                            concurrency substrate.
  Accepted by:              <blank — explicit human signature required if armed>
  Date:                     <blank>
  Trigger to revisit:       any reintroduction of a blocking call in the send/recv/handle path.

RISK OVERRIDE (DORMANT #3 — pre-positioned, not armed; r4) — contingent residual: HIGH
  Risk:                     R8 (EC copy-before-free on handle-collected `-> T errors` results)
  Arming trigger:           the P2 R8 proof REDs — the grep-audit finds a runtime
                            conditional-on-receive copy path instead of the claimed unconditional
                            compile-time spawn-form-keyed copy, OR the R8 fixture matrix (including
                            the composed R5×R8 cell) surfaces any use-after-free / dangling
                            ok-pointer / double-free. B1 step zeroes (no proof → step 0); only the
                            B2 matrix (−1) remains: B→C, lookup(C,II) = HIGH. (Distinct from
                            dormant #2, whose arming trigger is scoped to the deadlock class —
                            this one covers R8's memory-safety RED, which #2 does not.)
  Why not mitigable to LOW: with the static-keyed design disproven, collection becomes runtime
                            state on frame-free timing inside a historically fragile wrapper (the
                            M3f same-callee staging-slot clobber, IMP-concurrency:473); no
                            frozen-catalog pattern short of a redesign moves a memory-safety
                            silent-wrong class further, and the plan HALTS for that redesign.
  Accepted by:              <blank — explicit human signature required if armed>
  Date:                     <blank>
  Trigger to revisit:       any change to EC-wrapper result staging or frame-free timing.
```

**Mandatory-factor coverage (this table + phases carry the applicable ones).** Applicable, woven in:
security (bounded-by-construction channels = memory-exhaustion/DoS resistance per stdlib Rule 4;
kernel gate closes the UB path; no new parsing/injection surface), perf/BigO (padding, acq-rel,
capacity 64, <10% `--release` analysis budget), race/TOCTOU (R3/R5 — the milestone's core subject),
resource-cleanup (alloc=free gates; handle-drop semantics; trap door 1c is a cleanup hazard; the
R8 handle-owned EC result buffer freed exactly once at handle drop),
error-handling (closed-channel typed `errors`; SendError mapping locked P0), type-safety
(`channel<T>` typeck; typed error returns), observability (build-time hostile fixtures compensate
for the confirmed zero-runtime-telemetry gap — parked with trigger in Future Requirements),
reusability/DRY (R6 unification IS the DRY fix; `[[lint_rule]]` built generic for M5), idempotency
(golden-regen script; P6 release steps verified pre-push and re-runnable pre-tag), docs (teaching
surface + `REF-concurrency.md` user-spec update + IMP amendments, P5/P6). N/A with reason:
accessibility (compiler CLI/LSP text output; hover/diagnostics are plain text — no visual-only
channel), PII/privacy (no user data touched), compliance (solo pre-release project; the one license
surface — Tokio MIT — is already bundled), SEO (no web surface).

## 2. Mission

Ship `channel<T>()` bounded task communication, the `background` handle-form
(`.send()`/`.receive()`), cross-thread auto-`Arc`, and false-sharing auto-padding on the real
M3b/M3d poll-based substrate — with the may-block analysis unified into ONE authoritative source
first, and every send/recv/handle path deadlock-safe by construction against the three named trap
doors — then cut the final `v0.3.0` tag folding un-tagged M3f + M3g, **because** v0.3.0 is the first
release where Yinz code genuinely runs concurrently and it must ship on proven, single-truth
substrate, not on a fifth hand-synced copy of a list that has already silently miscompiled four
milestones running.

## 3. Execution

### 3.1 Intent & End State  ← MANDATORY

**Purpose.** Deliver the concurrency-communication surface (channels + task handles + safe
cross-thread sharing) that makes `v0.3.0` a truthful "runs concurrently" release, and cut that
release. Two load-bearing constraints above all others, in priority order:

1. **One authoritative may-block source (R6).** The suspension-classification question is answered
   in exactly ONE place, threaded into every consumer (typeck, codegen, admission, hint pass). No
   phase may extend a second copy "to keep them in sync by hand" — that is the four-milestone
   corpse. If unification proves infeasible, HALT at P0 and arm dormant override #1; do not quietly
   ship the parity-test-only fallback as if it were the plan.
2. **No blocking call, no trap door (R5/R1/R2).** Every suspension in the channel/handle path
   routes through poll-yield state machines with real endpoint futures and forwarded wakers. The
   three trap doors (1a `CpuJoinHandle` modeling, 1b re-polled one-shot join, 1c `sleep_handle`
   slot reuse) are design-prohibited, spike-proven, and fixture-locked.

**Key outcomes (testable End State).**

1. `channel<T>()` compiles and runs: bounded, default capacity 64, `channel<T>(N)` override, no
   unbounded constructor; `send()`/`recv()` via `tokio::sync::mpsc` (the `sync` feature enabled at
   `ynz-runtime/Cargo.toml:23` — required P0 step). `send()` on a full channel SUSPENDS the task
   via an independent Tokio task + real endpoint futures with forwarded wakers (the
   `ynz_rt_async_sleep_poll` discipline) — never a synchronous blocking call. Channel ops join the
   may-block set **through the R6 authoritative source**, never a type-level marker
   (no-function-coloring invariant).
2. Background handle-form: `let h = background fn()` (lifting the locked error at
   `check.rs:1331-1342`) yields a handle supporting `.send()` into the running task AND repeated
   `.receive()` — covering BOTH message replies from a long-running task (the IMP-concurrency:174-179
   canonical shape) AND, when the spawned function is a plain suspending `-> T errors` function,
   delivery of its own completion value once the task finishes (one `.receive()` surface — "the next
   thing from the task" — not two APIs). The `ECWrapperResultCollection` copy-before-free fix lands
   WITH this feature per the design doc's own gating (IMP-concurrency:463-479): a collected result's
   ok-value is copied to a handle-owned heap buffer BEFORE `free_frame`; the bare fire-and-forget
   spawn path is byte-for-byte unchanged. A real new C-ABI surface: independent joinable Tokio task +
   real channel wired into the spawned frame. NOT a `CpuJoinHandle`/`ynz_rt_join_poll` wrapper
   (1a/1b), NOT a `sleep_handle` tenant (1c).
3. The composed scenario — child suspends on `send()`-on-full while the parent polls `.receive()` —
   does not deadlock: proven by the P0 spike through the real compiler AND locked by the
   build-blocking composed hostile fixture gating P2.
4. The may-block set is unified: one authoritative suspension-source classifier consumed by
   `intrinsics.rs`/`emit.rs` consumers alike, with a build-blocking parity/RED tripwire test.
   `emit.rs:819`'s independent copy no longer exists.
5. Auto-Arc wraps cross-thread shared state (acquire-release); the boundary against the reject at
   `check.rs:2275-2280`/`2287-2292` is exact BOTH ways, including the unresolvable/non-ident-callee
   silent-skip cases (`check.rs:2269-2270`) — no silent data race, no false compile error.
6. Shapes with fields touched by different `background` tasks get 64-byte cache-line auto-padding
   (codegen-only, no muted hint); `cross-thread-fields-not-padded` Tier 3 lint fires when padding
   can't apply; padding consumes the SAME authoritative cross-thread-access analysis the chosen
   lowering uses, so `--no-auto-parallel` sequential lowering self-gates it (first layout transform
   this flag ever gates — proven at P4, not assumed).
7. `[[lint_rule]]` registry entry-kind built from scratch (schema + parser + `build.rs` constants +
   LSP seam), generic enough that M5 adds `array-using-soa-layout` with zero rework; carries
   `cross-thread-fields-not-padded` + `prefer-yielding-sleep`. `channel_capacity` (Addition, `⟨64⟩`
   + default-vs-user-set in hover) and `auto_arc` (Informational, cautionary) muted-hint domains
   fire; kernel-mode channel gate + every new diagnostic is WHAT/WHAT-INSTEAD/WHY; VSCode extension
   bumped with screenshots.
8. `--no-auto-parallel` cross-impl oracle covers every new capability byte-identical, including the
   padding transform and the composed-suspension case.
9. Demo + gallery: `examples/pirates-roster/entrypoint.ynz` demonstrates channels, handle-form,
   auto-Arc hint, false-sharing padding; `examples/primantis-orders/v0_3_m4_errors.ynz` covers
   every new diagnostic; `docs/reference/REF-concurrency.md` gains the user-facing channel/handle
   surface INCLUDING the backpressure teaching text IMP-no-function-coloring mandates ("a suspended
   producer is backpressure working, not a deadlock").
10. Release: `Cargo.toml` `0.3.0-m7` → `0.3.0`; CHANGELOG spans `m7..HEAD` (verified with
    `git tag`/`git log` at execution — folds M3f + M3g + M4); `v0.3.0` tag cut (final, no `-mN`).

**Definition of done.** All ten outcomes verified through the real compiler; the R6 unification +
parity test, the R5 composed fixture, the R1/R2 hostile fixtures, the R8 collection matrix
(including the composed R5×R8 suspend-mid-execution-then-collect cell), and the
R3 boundary matrix are GREEN and build-blocking; no dormant override armed; design docs + user
spec amended. (Honest
scope notes, reconciled against this plan's own deferrals: the seq-cst opt-in API and the
`auto_arc` red-tint VISUAL may ship as recorded deferrals rather than features — see §3.1 decisions
and Future Requirements; neither is claimed above.)

**Disciplined-initiative fallback (when steps and reality diverge).** If any step tempts a
synchronous blocking call — or a "quick" second copy of the may-block list — STOP: those are the M2
block_on corpse and the M3a–M3g drift corpse respectively; route through the poll-yield protocol /
the authoritative source, or surface a BLOCK. If the spike or any deadlock-safety fixture REDs,
HALT (dormant override #2 arms); if the R8 grep-audit or collection matrix REDs — a memory-safety
finding, not a deadlock — HALT likewise (dormant override #3 arms). Either way, do not narrow the
fixture to pass. Demo/gallery scope conflicts
resolve in favor of concurrency-correctness outcomes (1–6). Do NOT pull auto-SoA back in — it is
M5. On any design-doc contradiction: HALT and surface "design doc X says A; the plan says B" — the
design doc wins unless Patrick changes it.

**Recorded durable decisions (made without a human, reasons on the record):**

- **R6 unification shape.** The authoritative source is a single suspension-source CLASSIFIER (not
  just a name list — it must classify named intrinsics, FFI `may-block`, AND channel
  methods-on-a-value), defined once and consumed by both typeck and codegen. The exact home (a
  shared definition in the crate dependency-graph position both can consume — typeck-exported
  constant/query vs. a shared lower crate) is locked at P0 after reading the actual crate DAG; the
  DIRECTION (one source, per `authoritative-derivation.md`) is decided here and is not renegotiable
  by an executor. Reason: fifth-touch of a four-times-failed twin is the exact banned pattern; the
  only open question is placement mechanics, not whether to unify.
- **Endpoint futures live in the handle/runtime object, not the frame header.** Avoids trap door 1c
  by construction and avoids the `FRAME_HEADER_SIZE`/`FrameLayout` ripple unless the spike proves a
  header slot unavoidable (then it becomes named P2 work, not a silent squeeze into `sleep_handle`).
- **Default capacity 64 ships as the locked constant.** IMP-no-function-coloring left the final
  number "TBD via real benchmarking pre-v0.2"; that benchmarking never happened and no workload
  data exists to move it. 64 is the doc's own reasoned proposal; re-tuning is a one-constant change
  parked with a trigger (real workload evidence) rather than an M4 benchmarking side-quest.
- **The handle collects completion values too — `ec-wrapper-collect-on-completion` ships IN P2,
  not as a deferral (r3 reversal of the r2 decision; Patrick's no-duct-tape catch).** The r2 draft
  narrowed the handle to "communication-only" so the EC copy-before-free path would stay
  unreachable — a scope-narrowing invented to dodge a required companion fix, not a legitimate
  four-field deferral. Three independent sources tie the fix to THIS milestone's feature:
  (a) IMP-concurrency:475's cost line reads "landing WITH the `background-handle-form` feature";
  (b) IMP-concurrency:477's trigger names `.send`/`.receive` — the exact surface P2 builds — as the
  collection mechanism (not some future collection API); (c) the registry entry itself carries
  `ships_in = "v0.3-M4"` (`features.toml:1168`). Per Patrick's standing rule — a fix the design
  docs require for the feature this plan builds is in-plan work (Phase-1-or-earmark test: this plan
  DEPENDS on it) — it is a build-blocking P2 deliverable, scored as R8. Design shape (durable call,
  reason on the record): the copy decision keys on the COMPILE-TIME spawn form, never on runtime
  "was `.receive()` called yet" state — bare `background f()` = fire-and-forget, the wrapper
  discards the EC result exactly as today (zero behavior change); `let h = background f()` =
  collecting — the wrapper reads the EC struct BEFORE `free_frame`, copies the ok-value to a
  HANDLE-OWNED heap buffer, repoints the ok-word, then frees the frame; the buffer's lifetime is
  the handle's (freed exactly once, at handle drop, alloc=free-gated). Static keying eliminates the
  scheduler's collected-vs-discarded runtime-tracking problem (the very thing IMP-concurrency:469
  says collection requires knowledge of) by construction: a handle that is never received wastes
  one bounded heap copy — safe — never a dangling pointer. This is a conservative realization of
  the doc's "conditional on whether the handle is collected" (:475), not a divergence.
  `.receive()` stays ONE operation ("give me the next thing from the task"): message replies from
  a long-running loop, or a plain suspending function's own completion value as its final
  delivery — the doc's canonical example (:174-179) and the completion-collection trigger (:477)
  describe the same surface, not two competing designs.
- **Handle-drop semantics: safe-drop now, full `.cancel()` API not in M4's End State.** The locked
  cancel-via-drop model (IMP-no-function-coloring) governs the design; P2 implements drop without
  leak or type-confused free (alloc=free-gated), with cancellation injection at the child's next
  suspension point per the locked model. If full cancel-injection for channel-suspended children
  proves milestone-sized, P2 surfaces it and records a four-field deferral with trigger — never a
  silent detach-and-leak.
- **`auto_arc` cautionary red-tint is STAGED, not skipped.** IMP-no-function-coloring calls for a
  red-tinted muted hint; recon confirms NO tint rendering exists in `ynz-lsp` (hints are
  label+tooltip only) — this is net-new LSP rendering, not a style flag. Ship `auto_arc` now as an
  Informational domain with the "reference counting has cost" caution in the WHAT/WHY hover (which
  renders today); attempt the tint at P3; if it requires a decoration-based renderer, record
  `[[deferred_tooling_feature]]` `auto-arc-cautionary-tint` with trigger. Teaching content ships;
  only the color stages. Not a design divergence — surfaced in Design-Doc Alignment.
- **`[[lint_rule]]` mechanism builds in P4, not P0.** P0 stays a pure risk-burndown HARD GATE whose
  STOP verdict must not entangle with unrelated registry mechanism work; the mechanism's first
  consumer is P4's lints and nothing earlier needs it. (Generality requirement unchanged: zero
  M4-specific hardcoding, M5 reuses as-is.)

### 3.2 Concept

Seven phases, risk-burndown-first. **P0 is a HARD GATE**: unify the may-block source (R6) and prove
the composed-suspension design by spike through the real compiler (R5) — with explicit STOP
conditions that halt the plan for redesign (arming the dormant overrides) rather than papering over
a broken foundation. Nothing durable builds until P0 is GREEN. P1 ships channels on the unified
source (+ kernel gate, R7); P2 ships the handle-form — including the design-doc-gated EC
copy-before-free collection fix (R8) — with the composed and collection fixtures as its
build-blocking gates; P3 lands auto-Arc with the exhaustive boundary matrix (R3); P4 builds the
generic `[[lint_rule]]` mechanism + padding + the two lints; P5 consolidates the teaching surface,
demo, gallery, user spec, and cross-impl sweep; P6 folds M3f+M3g and cuts `v0.3.0`. Each executable
phase (P1–P4) extends demo + gallery for its own surface; P5 consolidates and regenerates goldens.
Handoff = checkbox state + session-id chain.

### 3.3 Phases

#### Phase 0 — Risk burndown HARD GATE: may-block unification (R6) + composed-suspension spike (R5) + design locks
- **Task + purpose.** Kill the two blocker-class risks before anything builds on them: unify the
  twin-derived may-block set into one authoritative source, and prove the composed-suspension
  design executes through the real compiler. Lock every design detail later phases consume.
- **Steps.**
  1. Enable the Tokio `sync` feature (`ynz-runtime/Cargo.toml:23`) — the easy-to-forget dependency
     step, done first so the spike can link.
  2. **R6 unification.** Read the actual crate DAG; define the single authoritative
     suspension-source classifier (named intrinsics + FFI may-block + channel-method shapes) in the
     one home both typeck and codegen consume; thread it into EVERY consumer (`intrinsics.rs:24`,
     `emit.rs:819` — deleted as an independent definition — `may_block.rs:916`,
     `cpu_admission.rs:806`, `check.rs:2595`/`8339`, `emit.rs:627`/`3418`/`6726`); grep-verify zero
     independent definitions remain. Then add the **build-blocking parity/RED tripwire test**
     (defense-in-depth even after unification — it guards future consumers).
  3. **R5 runtime spike** (throwaway per spike discipline, explicit STOP conditions): implement the
     minimal independent-Tokio-task + real-endpoint-futures + forwarded-wakers path and execute
     spawn + channel-send-on-full + parent-receive through the REAL compiler (`ynz run` on real
     `.ynz`), never a hand-written Rust model (the M2-spike false-ACCEPT lesson). Verdicts: S1
     composed scenario completes (no deadlock, no hang, timing-verified suspension); S2 no
     synchronous blocking call in the executed path (grep-audit); S3 no trap door engaged (no
     `CpuJoinHandle` modeling, no `join_poll` re-poll, no `sleep_handle` tenancy; endpoint futures
     live in the handle object). Study `emit.rs:8827` (fused composed-poll) as prior art first.
     **STOP condition: any verdict RED → HALT the plan, arm dormant override #2, redesign.**
     Teardown: discard spike scaffolding; **persist two artifacts** (recorded opt-out per spike
     discipline): the spike report (verdicts + timings) and the composed-scenario `.ynz` source,
     which seeds P2's build-blocking hostile fixture.
  4. Resolve the design locks (Assumptions 8–11): dropped-receiver `send()` error mapping;
     `channel` keyword/type-registry question; seq-cst opt-in ship-or-defer (roadmap pre-authorizes
     the deferral); frame-header-slot verdict from the spike. Record each with its reason in this
     plan (FRAGO if any changes a phase) + the registry where applicable.
- **Exit criteria.** `cargo build --workspace` green with `sync` linked; ONE authoritative
  may-block source with all consumers threaded + grep proof + parity test RED→GREEN and
  build-blocking; spike verdicts S1–S3 GREEN with report + composed `.ynz` persisted; all four
  design locks recorded.
- **Reviewer fan-out.** code-reviewer (unification diff vs `authoritative-derivation.md`; consumer
  completeness) + adversarial-tester (spike verdict audit — no self-graded ACCEPT).
  - **Agent-availability note (structural substitution, disclosed):** `adversarial-tester` is not an
    available agent type in this execution environment — the conductor confirmed this against the
    live agent roster; no such type exists. At the start of Phase 0 conducting, the conductor made an
    explicit, disclosed decision to fold adversarial-tester's mandate — independently auditing the R5
    spike verdicts so they are never self-graded by the executor — into code-reviewer's dispatch
    scope instead. This substitution actually happened and the audit substance is real: in round 1,
    code-reviewer was explicitly instructed to "verify the plan's own claim... that a runtime spike
    proved the composed-suspension scenario deadlock-safe... confirm its persisted artifacts exist and
    are non-vacuous," and reported back (verbatim): "Spike artifacts exist and are non-vacuous —
    confirmed. Both `composed-scenario.ynz` and `R5-composed-suspension-spike-report.md` persist. The
    report carries measured output through the real compiler (`received_sum=30`,
    `parent_pending_count=2`, `child_send_suspended=true`, `send2_after_first_recv=true`, clean exit
    0, 30s deadlock-guard never fired), documents the torn-down throwaway mechanism, and explicitly
    refuses a self-graded ACCEPT. Not a rubber stamp." This satisfies the "no self-graded ACCEPT"
    requirement — an honest, on-the-record structural substitution, not a silent gloss-over.
- **Model tag.** `(concurrency-codegen-runtime, maximum-adversarial, large)`.
- **✅ P0 STATUS — COMPLETE (executor-2026-07-02-m4-p0, 2026-07-02). All exit criteria met; NO STOP
  condition triggered; NO dormant override armed. Reviewer fan-out COMPLETE across two rounds (see the
  "Agent-availability note" above for the adversarial-tester → code-reviewer substitution): round 1
  ran the full reviewer fleet — code-reviewer, acceptance-verifier, rules-compliance, deviation-judge,
  test-quality — 0 blockers, 1 should-fix (found independently by both code-reviewer and test-quality:
  the R6 tripwire scan was too narrow) + 1 FRAGO candidate (the SuspendSet deferral-recording gap);
  round-2 fix-loop landed both fixes; round-2 re-verify (code-reviewer + deviation-judge) re-confirmed
  clean — 0 findings on the fixes themselves — but deviation-judge found 3 should-fix + 1 minor in the
  plan-seam recording itself (this round-3 plan-seam reconciliation resolves them — see FRAGO 001's
  restored canonical field shape in [`audit.md`](audit.md) and this banner + the agent-availability
  note above, all landed in round 3).**
  - **Step 1 — Tokio `sync` feature:** enabled at `crates/ynz-runtime/Cargo.toml:23` (+`[dev-dependencies]`);
    `cargo build --workspace` green.
  - **Step 2 — R6 unification (DONE):** single authoritative classifier
    `ynz_typeck::suspension_source` (`BASE_SUSPENSION_INTRINSICS` + `is_base_suspension_intrinsic`,
    new file `crates/ynz-typeck/src/suspension_source.rs`). The `emit.rs:819` twin DELETED; the dead
    `intrinsics.rs` `is_may_block_callee` removed. All 7 consumers threaded onto the one source:
    typeck — `may_block.rs:916`, `cpu_admission.rs:806`, `check.rs:2597`, `check.rs:8341`; codegen —
    `emit.rs:627`, `emit.rs:3413`, `emit.rs:6721`. Grep proof: zero `M2_MAY_BLOCK_INTRINSICS`
    references remain; exactly ONE literal list (in the authoritative home). Build-blocking tripwire
    `crates/ynz-typeck/tests/suspension_source_single_definition.rs` proven RED→GREEN (re-inject a
    twin ⇒ RED at the exact file:line; remove ⇒ GREEN). typeck+codegen+driver+runtime suites all
    green (no behavioral regression). NOTE for P1: channel `.send()`/`.receive()` are methods-on-a-value
    — add their classification as a sibling arm INSIDE `suspension_source.rs`, never a new list.
  - **Round-2 fix-loop resolution (executor-2026-07-02-m4-p0-r2, following the full reviewer fleet —
    0 blockers, 2 real findings).** Two fixes, no new runtime behavior:
    - **(a) `SuspendSet` type-alias twin UNIFIED (was: "left for a follow-on").** The P0 recon had
      surfaced a second twin — `pub type SuspendSet = HashSet<String>;` independently declared at BOTH
      `cpu_admission.rs:33` AND `emit.rs:132` — and DEFERRED it as a bare audit sentence with no
      four-field deferral. The conductor ruled this a **risk-neutral FRAGO** (auto-applied, no
      signature — see [`audit.md`](audit.md) FRAGO 001) because a `HashSet<String>` type alias is
      structurally transparent and cannot semantically drift the way R6's content-list twin could.
      Fixed now: `emit.rs:132`'s independent declaration is deleted and replaced by
      `pub use ynz_typeck::cpu_admission::SuspendSet;` (a re-export, not a second declaration), so
      `emit::SuspendSet` consumers (`queries.rs:15`, `frame_layouts_query.rs:28`) keep their path
      while exactly ONE `type SuspendSet = ...` declaration exists workspace-wide (`cpu_admission.rs:33`,
      grep-confirmed). `cargo build`/`cargo test --workspace` green, tally unchanged (zero behavior).
    - **(b) R6 tripwire drift-scan HARDENED** (code-reviewer + test-quality, independently). The
      parity/RED tripwire (`suspension_source_single_definition.rs`) previously required both leaf
      literals AND `[` on the SAME line — a `cargo fmt --all` reformat of a long single-line literal
      (one name per line) or a `"a" | "b" => ...` match-arm re-derivation silently defeated it. Now it
      keys on both quoted leaf literals co-occurring within 5 lines (well below the ~20-line spacing of
      the legitimate divergent per-intrinsic dispatch arms in `check.rs`/`emit.rs`). Re-proven: all
      THREE twin shapes (single-line literal, `cargo fmt` multi-line literal, match arm) go RED at the
      exact offending line-pair; clean tree GREEN; the backtick-prose mentions (`may_block.rs:6`,
      `cpu_admission.rs:747` use `` `sleep` `` not `"sleep"`) do not false-positive.
  - **Step 3 — R5 composed-suspension spike (DONE, GREEN):** executed the composed scenario (child
    suspends on `send()`-on-full while parent polls `.receive()`) through the REAL compiler
    (`ynz run`) via a throwaway sentinel-sleep driver + throwaway runtime shims (all torn down;
    persisted artifacts in [`p0-spike/`](p0-spike/)). Verdicts: **S1 GREEN** (sum=30, parent
    suspended ×2 via forwarded waker, child suspended-on-full, ordering held send#2-after-recv#1,
    ~26ms, clean exit — no deadlock/hang under 30s guard); **S2 GREEN** (no synchronous blocking call
    in the send/recv path — `poll_recv`/`.await`/`try_send` only); **S3 GREEN** (independent joinable
    `tokio::spawn` child — not `CpuJoinHandle`; `poll_recv` drive — not re-polled `join_poll`;
    endpoints owned by the runtime handle object with `sleep_handle` NULL — trap door 1c avoided).
    Report + composed `.ynz` seed persisted at
    [`p0-spike/R5-composed-suspension-spike-report.md`](p0-spike/R5-composed-suspension-spike-report.md)
    and [`p0-spike/composed-scenario.ynz`](p0-spike/composed-scenario.ynz).
  - **Step 4 — Design locks resolved (Assumptions 8–11), each with reason:**
    - **Lock 8 (dropped-receiver `send()` error mapping) → Yinz-level `errors` wrapper.**
      `channel.send(value)` is `-> nothing errors`; a dropped/closed receiver yields a TYPED Yinz
      channel-closed error, never the raw Tokio `SendError<T>` (jargon + generic-type leak, Golden
      Rule 12 / vocabulary). Never a silent drop (Safety invariant); the unsent value is dropped by
      ownership. P1 step 4 implements this.
    - **Lock 9 (`channel` keyword vs type-registry) → typeck-level built-in generic type
      constructor, NO `[[keyword]]`.** `channel<T>` follows the exact `array<T>`/`map<K,V>`/`fixed<T>`
      pattern — the parser already handles `channel<int>` via `parse_generic_type`; typeck maps the
      name `"channel"` to a new `Type::BuiltinChannel { elem }` variant. Reason: `array`/`map`/`fixed`
      are NOT lexer keywords (verified — absent from the registry `[[keyword]]` list); a keyword would
      be inconsistent and need lexer changes for no benefit. Confirms the plan's "no new `[[keyword]]`
      unless the P0 lock says so" default.
    - **Lock 10 (seq-cst opt-in ship-or-defer) → DEFER via `[[deferred_language_feature]]`.** Added
      `seq-cst-ordering-opt-in` to `registry/features.toml` (`ships_in = "v0.4+"`). Ship acquire-release
      only (correct for channel handoff + Arc refcount; cheaper on ARM). Reason: no v0.3-M4 workload
      needs seq-cst, naming is TBD, an unused ordering-tunable would surface memory-ordering jargon.
      Roadmap pre-authorized; a documented deferral, not a divergence.
    - **Lock 11 (frame-header slot verdict) → NO new frame-header slot is FORCED by the runtime
      architecture (spike-proven).** Endpoint futures live in the handle/runtime object (`sleep_handle`
      stayed NULL); the composed scenario ran deadlock-free with endpoints OUTSIDE the frame. P2 must
      persist ONE opaque handle pointer across the parent's suspension in a DEDICATED slot (never the
      type-punned `sleep_handle` — 1c); whether that dedicated slot is a new frame-header slot or a
      crossing-local is a P2 codegen decision, and the `FRAME_HEADER_SIZE`/`FrameLayout` ripple is
      named-but-not-forced P2 work. Confirms the §3.1 recorded decision ("endpoint futures live in the
      handle/runtime object, not the frame header").
  - **Pre-existing failure surfaced (NOT caused by P0, out of scope):** `ynz-registry` test
    `design_future_sync::every_future_doc_has_a_registry_entry_or_is_skipped` fails with
    `cannot read design/future/: No such file or directory` — the test hardcodes the `design/future/`
    path removed by the 2026-07-01 docs migration (commit `93506c0`). Docs-migration fallout, unrelated
    to may-block/spike/registry-entry work; a TOML edit cannot cause a directory-read error. Noted, not
    fixed (scope). Belongs on a docs-migration cleanup pass.

#### Phase 1 — `channel<T>()` type + bounded send/recv + suspension via the unified source + kernel gate  ⚠ M2-HALT-adjacent
- **Task + purpose.** Make `channel<T>()` real: bounded typed construct whose `send()`-on-full
  suspends the task through the poll-yield protocol — R1 and R7 mitigations in code.
- **Steps.**
  1. `channel<T>` type + typeck + codegen; `ynz_channel_send`/`ynz_channel_recv` C-ABI over
     `tokio::sync::mpsc`; bounded default 64, `channel<T>(N)` override, no unbounded constructor.
  2. Route send/recv suspension through the state-machine protocol per the spike-proven design;
     classify channel ops as suspension sources **via the R6 authoritative classifier only** (this
     also makes `cpu_admission` decline channel-using closures for free — verify with a decline
     fixture).
  3. **Kernel-mode gate (R7):** channel ops → COMPILE ERROR in `--kernel`, matching the
     `check.rs:2223-2233` pattern, WHAT/WHAT-INSTEAD/WHY.
  4. Closed-channel/dropped-receiver `send()` → typed `errors` per the P0 lock; channel-full
     backpressure teaching text; never a silent drop.
  5. `channel_capacity` muted-hint domain (Addition, `⟨64⟩` in the empty parens; hover shows
     capacity AND default-vs-user-set per IMP-no-function-coloring).
  6. **Deadlock-safety gate (build-blocking):** grep-audit (no blocking call in the emitted path) +
     hostile fixtures (full-channel backpressure, closed-channel, never-drained) GREEN through the
     real compiler; alloc=free via `YNZ_ALLOC_COUNTER_OUTPUT`.
  7. Extend demo + `v0_3_m4_errors.ynz` (channel surface; kernel-gate + closed-channel triggers
     with `// WHY:` comments).
- **Exit criteria.** Channel programs run via `./target/debug/ynz run`; send-on-full suspends
  (timing-verified, no thread block); kernel gate + admission decline fire; closed-channel `errors`
  handled; grep-audit clean; hostile fixtures GREEN; alloc=free; `--no-auto-parallel`
  byte-identical on channel fixtures.
- **Reviewer fan-out.** code-reviewer + adversarial-tester (deadlock-safety gate, MANDATORY) +
  per-phase opus adversarial gate.
- **Model tag.** `(concurrency-codegen-runtime, maximum-adversarial, large)`.

#### Phase 2 — Background handle-form `.send()` / repeated `.receive()` + the composed fixture  ⚠ M2-HALT-adjacent
- **Task + purpose.** Lift `check.rs:1331-1342`; ship the handle-form on the spike-proven
  substrate — including the EC copy-before-free collection fix the design doc gates on this exact
  feature (R8, IMP-concurrency:463-479); lock R5 with the composed hostile fixture and R8 with the
  collection fixture matrix.
- **Steps.**
  1. New C-ABI handle machinery per the spike design: independent joinable Tokio task + real
     channel(s) wired into the spawned frame; endpoint futures owned by the handle object (never
     `sleep_handle` — 1c); poll-based drive only (never `join_poll` re-poll — 1b); never a
     `CpuJoinHandle` model (1a). If the spike verdicted that a frame-header slot is unavoidable,
     execute the named `FRAME_HEADER_SIZE`/`SPIKE_HANDLE_BASE_OFFSET`/`FrameLayout` ripple here —
     explicitly, const-asserts updated, M3e cross-module serialization included.
  2. Lift + re-type the handle-form compile error at `check.rs:1331-1342`; typeck the handle's
     `.send()`/`.receive()` surface.
  3. Handle-drop semantics per the locked cancel-via-drop model: no leak, no type-confused free;
     cancellation injected at the child's next suspension point — or, if milestone-sized, the
     recorded four-field deferral (surface, never silently detach-and-leak).
  4. **`ECWrapperResultCollection` lift (R8, build-blocking — the fix IMP-concurrency:475/477
     gates on THIS feature):** implement copy-before-free in the standalone EC wrapper per the §3.1
     recorded design — spawn-form-keyed at compile time: handle spawns read the EC struct BEFORE
     `free_frame`, copy the ok-value to a handle-owned heap buffer, repoint the ok-word, THEN free
     the frame (buffer freed exactly once, at handle drop); bare fire-and-forget spawns keep
     today's discard path byte-for-byte (no runtime conditional-on-receive copy path anywhere —
     grep-audited). `.receive()` on a plain suspending `-> T errors` child delivers the completion
     value (typed `T errors`) as the task's final delivery, through the same handle machinery as
     message replies.
  5. Retire the M1 `[[deferred_tooling_feature]]` `background-handle-form` entry AND the
     `[[deferred_language_feature]]` `ec-wrapper-collect-on-completion` entry (`features.toml:1164`)
     — both ship here; the IMP-concurrency §463-479 shipped-status amendment lands at P6 step 1.
  6. **R5 composed + R8 collection gates (build-blocking):** the composed hostile fixture grown
     from the persisted spike `.ynz` (child sends-on-full while parent polls receive) GREEN; plus
     never-received-handle and pool-exhaustion hostile fixtures (DECLINE→FIRE pattern); PLUS the R8
     RED-repro matrix — collected vs. fire-and-forget × `-> T errors` ok/error paths ×
     receive-before/after-completion timing — collected values byte-correct, no dangling
     ok-pointer, no double-free, fire-and-forget output unchanged; PLUS the **composed R5×R8 cell**
     (r4, build-blocking; a dedicated matrix cell, not a full cross-product axis — recorded call:
     the R5×R8 intersection is only reachable on the collected arm, so an axis would manufacture
     meaningless fire-and-forget×suspend cells): a collected `-> T errors` child suspends on a
     full-channel `send()` mid-execution (R5's scenario), then finishes, and its completion value
     is collected via `.receive()` through the copy-before-free path (R8's scenario) — asserting
     the frame survives the channel suspension correctly, the collected value is byte-correct, no
     dangling ok-pointer, no double-free, and the frame + buffer are each freed exactly once;
     alloc=free proof including handle-drop and buffer-free paths.
  7. Extend demo + gallery (handle-form surface).
- **Exit criteria.** Handle round-trips run through the real compiler; the composed fixture GREEN
  and build-blocking; never-received handle does NOT deadlock (bounded/observed, not a hang);
  grep-audit clean (no blocking call; no runtime conditional-on-receive copy path); pool-exhaustion
  GREEN; the R8 collection matrix — including the composed R5×R8 suspend-then-collect cell — GREEN
  and build-blocking (collected `-> T errors` values byte-correct; the frame survives a
  mid-execution channel suspension and is then freed exactly once at collection; fire-and-forget
  unchanged); alloc=free (including handle-drop and R8 buffer-free
  paths); both registry retirements landed; `--no-auto-parallel` byte-identical.
- **Reviewer fan-out.** code-reviewer + adversarial-tester (composed + deadlock gates, MANDATORY) +
  per-phase opus adversarial gate.
- **Model tag.** `(concurrency-codegen-runtime, maximum-adversarial, large)`.

#### Phase 3 — Auto-Arc cross-thread wrapping + boundary exactness
- **Task + purpose.** Emit auto-`Arc` (acquire-release) for cross-thread shared state with the
  boundary exact both ways against the confirmed reject — R3 mitigation in code.
- **Steps.**
  1. Codegen auto-Arc where cross-`background` sharing requires refcounting; acquire-release per
     IMP-no-function-coloring "Atomic Ordering."
  2. Boundary exactness against `check.rs:2275-2280` (share) / `2287-2292` (lend): should-reject
     stays rejected; should-Arc is not falsely rejected. **Explicitly design for the silent-skip
     gap** (`check.rs:2269-2270` — non-ident/unresolvable callees): the auto-Arc boundary must not
     inherit that skip as a silent no-Arc no-reject hole; close it or reject it loudly, decided on
     the record.
  3. `auto_arc` muted-hint domain (Informational; cautionary WHAT/WHY hover). Attempt the red-tint
     visual (expect net-new LSP rendering per recon); if it needs a decoration renderer, record
     `[[deferred_tooling_feature]]` `auto-arc-cautionary-tint` with trigger (§3.1 decision).
  4. **R3 gate (build-blocking):** exhaustive RED-repro matrix — share/lend/give ×
     background-boundary × Arc-required, both directions, INCLUDING unresolvable/non-ident-callee
     cases.
  5. Extend demo + gallery (auto-Arc surface).
- **Exit criteria.** Matrix GREEN both directions incl. edge cases; auto-Arc programs correct under
  repeated runs (no intermittent race); hint fires with correct hover; `--no-auto-parallel`
  byte-identical; alloc=free (Arc control blocks freed).
- **Reviewer fan-out.** code-reviewer (design-doc diff) + adversarial-tester (boundary matrix).
- **Model tag.** `(typeck-ownership-codegen, maximum-adversarial, medium)`.

#### Phase 4 — `[[lint_rule]]` mechanism + false-sharing auto-padding + the two lints
- **Task + purpose.** Build the net-new generic lint registry machinery; ship the codegen-only
  padding transform and its lint, plus `prefer-yielding-sleep`.
- **Steps.**
  1. Build `[[lint_rule]]` end-to-end, **generically** (TOML schema, `ynz-registry` parser,
     `build.rs` typed constants, LSP consumption seam) — zero M4-specific hardcoding; M5 adds
     `array-using-soa-layout` with no rework (do NOT build the SoA lint itself).
  2. Padding transform: detect shape fields accessed from different `background` tasks **via the
     same authoritative cross-thread/crossing analysis the chosen lowering uses** (no second
     derivation — `authoritative-derivation.md` again) → 64-byte alignment + inter-field padding.
     Codegen-only, NO muted hint (no typeable form).
  3. `cross-thread-fields-not-padded` Tier 3 lint when padding can't apply (e.g. FFI-shaped);
     `prefer-yielding-sleep` Tier 3 lint on `sleepBlocking(ms)` in non-kernel programs (suggestion,
     dismissable, NOT an error). Both WHAT/WHAT-INSTEAD/WHY.
  4. **Prove the `--no-auto-parallel` gating** (first layout transform this flag ever gates): under
     sequential lowering the authoritative analysis yields no cross-thread fields → padding
     self-gates; test asserts byte-identical output in both modes AND unpadded layout in sequential
     mode. Confirm no conflict with the existing shape-field auto-reorder.
  5. Extend demo + gallery (padding + both lint surfaces).
- **Exit criteria.** `[[lint_rule]]` parses to typed constants, LSP-readable, generic (reviewer
  checks: could M5 add a rule with zero mechanism edits?); padded field offsets verified on 64-byte
  lines; both lints fire with three-part text; gating test GREEN both modes; no auto-reorder
  conflict.
- **Reviewer fan-out.** code-reviewer (mechanism generality + design-doc diff).
- **Model tag.** `(codegen-lint-infrastructure, high, medium)`.

#### Phase 5 — Teaching surface + user spec + demo/gallery consolidation + cross-impl sweep
- **Task + purpose.** Consolidate every teaching surface and run the mandatory verification sweeps.
- **Steps.**
  1. Wire `inlay_hint.rs` for `channel_capacity` + `auto_arc` (established pattern: typeck hint-pass
     fn → LSP import → registry-sourced hover via `lsp_inlay_hint_hover_for`).
  2. Update `docs/reference/REF-concurrency.md` (user spec) for channels + handle-form + auto-Arc,
     INCLUDING the mandated backpressure teaching text ("a suspended producer is backpressure
     working correctly, not a deadlock") — HS-grad register per spec-writing rules.
  3. VSCode extension version bump + screenshots (channels, handle-form, auto-Arc hint).
  4. Final `pirates-roster/entrypoint.ynz` consolidation (ALL M4 surfaces in realistic context);
     regenerate `expected_stdout.txt` via the regenerate script; finalize `v0_3_m4_errors.ynz`
     (count + key-phrase assertions in `error_galleries.rs`; byte-exact golden convention, not
     `insta`).
  5. `jargon_audit` clean on every new diagnostic; full `--no-auto-parallel` cross-impl sweep across
     every new fixture (byte-identical), including padding and the composed case.
- **Exit criteria.** Hints + lints fire in-editor; screenshots attached; REF-concurrency updated;
  demo golden matches; gallery emits every M4 diagnostic; jargon audit clean; cross-impl oracle
  GREEN workspace-wide.
- **Reviewer fan-out.** code-reviewer (jargon + spec-register + oracle completeness).
- **Model tag.** `(docs-tests-integration, standard, medium)`.

#### Phase 6 — v0.3.0 release fold (M3f + M3g + M4)
- **Task + purpose.** Cut the final `v0.3.0` tag folding the un-tagged M3f + M3g work.
- **Steps.**
  1. Amend design docs to mark the v0.3 concurrency surface shipped (IMP-no-function-coloring
     Channel/False-Sharing/Sleep-lint sections' milestone notes; IMP-concurrency as needed —
     including marking the `ECWrapperResultCollection` §463-479 deferral SHIPPED: the
     copy-before-free fix landed with the handle-form at P2, exactly per that section's own
     "landing WITH the `background-handle-form` feature" gating).
  2. `Cargo.toml` `0.3.0-m7` → `0.3.0` (`Cargo.toml:21`).
  3. **R4 explicit step:** verify with `git tag` + `git log` that the CHANGELOG generation span is
     `m7..HEAD` — never a naive "since last tag" — and that the output demonstrably includes M3f +
     M3g + M4 before anything is pushed.
  4. `/release` cuts `v0.3.0` (final, NO `-mN` suffix); VSCode `.vsix` assets per convention
     (`yinz-{version}.vsix` + `yinz-latest.vsix --clobber`).
- **Exit criteria.** CHANGELOG demonstrably spans M3f + M3g + M4; `Cargo.toml` = `0.3.0`; tag cut
  with Patrick's explicit approval; `.vsix` assets uploaded.
- **Reviewer fan-out.** code-reviewer + **Patrick sign-off** (cutting the release is an explicit
  human act — never self-authorized).
- **Model tag.** `(release-engineering, high, small)`.

### 3.4 Coordinating Instructions

- **Sequencing.** P0 → P1 → P2 (handle-form reuses channel machinery); P3 and P4 require only P0
  and may overlap P1/P2 if capacity allows (P4's padding needs no channel code; P3's boundary work
  is typeck/codegen-local) — EXCEPT P4 step 5 and P3 step 5 (demo/gallery) which append after the
  earlier phases' sections exist. P5 gates on P1–P4 complete; P6 gates on P5.
- **P0 is a HARD GATE.** Its STOP conditions (spike RED, unification infeasible) HALT the plan and
  arm the corresponding dormant override — never a soft "noted, proceeding." No durable phase
  starts until P0 is GREEN.
- **Verify-before-complete.** No concurrency claim completes on assertion — everything runs through
  `./target/debug/ynz run` on real `.ynz` (the M2 spike false-ACCEPT lesson). The build-blocking
  gates: R6 parity test (P0), R1 hostile fixtures (P1), R5 composed + R2 never-received/exhaustion
  + R8 collection-matrix fixtures incl. the composed R5×R8 cell (P2), R3 matrix (P3). A failing
  gate is evidence, never something to weaken.
- **Reviewer triggers.** Any diff touching the send/recv/handle path → adversarial-tester
  deadlock-safety gate. Any diff touching the may-block classifier or its consumers → code-reviewer
  checks against `authoritative-derivation.md` (no second derivation anywhere). Every phase
  reviewer diffs against the CITED DESIGN DOCS, not only the plan (plan-invariants Step 9a).
- **CCIR — surface immediately, do not proceed.** (a) Any temptation toward a synchronous blocking
  call OR a second may-block derivation — the dormant overrides' arming triggers. (b) Any
  hostile/composed/never-received/pool-exhaustion fixture that deadlocks, or any R8
  collection-matrix cell (incl. the composed R5×R8 cell) surfacing a use-after-free / dangling
  ok-pointer / double-free, or the R8 grep-audit finding a runtime conditional-on-receive copy
  path — dormant override #3's arming trigger. (c) The spike verdicting
  that a frame-header slot is needed (named ripple work, not a silent squeeze). (d) An auto-Arc
  boundary case ambiguous both ways that the matrix can't resolve. (e) Any design-doc
  contradiction. (f) Auto-SoA scope creep. (g) Handle cancel-injection turning milestone-sized.
  (h) Any temptation to re-narrow the handle-form's scope (e.g. back to "communication-only") to
  route around the R8 copy-before-free work — that is the r3 corpse; surface it, never
  self-approve it.

## 4. Sustainment

- **Env / tooling.** All build/test in the `dev` docker service (`docker compose run --rm dev
  cargo …`) — `cargo`/`rustc` are not host-native. LLVM 18 / inkwell; Node 22 for the VSCode build.
- **Dependency change.** Tokio `sync` feature at `ynz-runtime/Cargo.toml:23` (Tokio already
  bundled; cargo-registry volume caches survive rebuilds).
- **Fixtures.** `crates/ynz-codegen/tests/`, `crates/ynz-driver/tests/cross_impl_consistency.rs` +
  `error_galleries.rs`, `examples/pirates-roster/` (+ golden + regenerate script),
  `examples/primantis-orders/v0_3_m4_errors.ynz`, the persisted P0 spike report + composed `.ynz`.
- **Verification env vars.** `YNZ_NO_AUTO_PARALLEL=1` (cross-impl oracle), `YNZ_ALLOC_COUNTER_OUTPUT`
  (alloc=free proofs).
- **Release tooling.** `/pr` per phase; `/release` for the tag; `.vsix` upload convention.

## 5. Command & Signal

- **Ownership.** Per-phase executor dispatched by the orchestrator per the Model tag; P0's HALT
  authority sits with the orchestrator (override signatures are Patrick's); P6 tag gated on Patrick.
- **Succession.** Resume from `plan-id: 2026-07-02-v0-3-m4-channels-arc-release` + the session-id
  chain + checkbox state. Slices travel with ¶2 Mission, ¶3.1 Intent & End State, and the dispatched
  phase's ¶1 risk rows (P0 carries R6+R5; P1 carries R1+R7; P2 carries R5+R2+R8; P3 carries R3; P6
  carries R4).
- **Audit trail.** `audit.md` in this directory (append-only; this correction pass is logged
  there). Roadmap back-pointer: `roadmap-id: 2026-05-21-v0-3-concurrency-perf`.

## Invariants This Milestone Must Preserve

### Safety
- Exactly ONE definition of the may-block/suspension-source classification exists in the workspace;
  every consumer (typeck, codegen, admission, hints) threads it; the parity/RED tripwire test fails
  the build on any divergence. (`emit.rs:819`'s independent copy is deleted, not deprecated.)
- No synchronous blocking call exists anywhere in the channel send/recv or background-handle path
  (grep-audited; spike- and fixture-proven).
- `send()` on a full channel suspends the calling task; it never blocks an OS thread. The composed
  scenario (child send-on-full + parent receive-poll) never deadlocks (build-blocking fixture).
- The three trap doors are structurally absent: no `CpuJoinHandle` modeling of a communicating
  child; no second poll of `ynz_rt_join_poll`; no channel future in the `sleep_handle` slot (both
  cancellation drop paths `runtime.rs:602`/`663` stay type-correct).
- `send()` to a closed channel / dropped receiver returns a typed `errors` value — never silent.
- A handle-collected `-> T errors` background result has its ok-value copied to a handle-owned heap
  buffer BEFORE `free_frame` — no dangling ok-pointer, no double-free (buffer freed exactly once,
  at handle drop); the copy decision is compile-time spawn-form-keyed (no runtime
  conditional-on-receive path exists); the bare fire-and-forget spawn path is byte-for-byte
  unchanged. This holds ALSO when the child suspended on a full-channel `send()` mid-execution:
  the frame survives the channel suspension, then undergoes copy-before-free at completion — the
  composed R5×R8 frame-lifetime interaction, locked by its own build-blocking P2 matrix cell.
- Channel ops in `--kernel` mode are a compile error (new gate, matching `check.rs:2223-2233`); the
  CPU admission gate can never admit a channel-using closure (classification via the one source).
- Auto-Arc applies only where cross-thread sharing requires it; the reject at
  `check.rs:2275-2280`/`2287-2292` still fires where it must; the `check.rs:2269-2270` silent-skip
  gap does not become a silent no-Arc/no-reject hole; legitimately-crossing values are not falsely
  rejected.
- Channel `.send()`/`.receive()` suspension is detected by the may-block analysis — never a
  type-level marker (no-function-coloring invariant).
- No frame, channel, handle, or Arc control block leaks: alloc=free on every fixture, including
  handle-drop paths (`YNZ_ALLOC_COUNTER_OUTPUT`).

### Performance
- Channel default capacity is the single locked constant 64 (backpressure within seconds of
  sustained overproduction; buffer bounded at `capacity × sizeof(T)` by construction).
- Auto-Arc uses acquire-release (not seq-cst) — cheaper on ARM, correct for handoff/refcount.
- False-sharing padding: 64-byte cache-line isolation for cross-thread-accessed fields (deliberate
  memory-for-throughput trade; recovers the documented ~3.1× false-sharing collapse).
- No per-send heap allocation beyond the bounded buffer + one Arc per cross-thread value.
- Analysis passes stay within the roadmap's <10% `--release` wall-clock budget on `pirates-roster`.
- **Auto-promotion analysis.** `channel<T>()` → default-capacity codegen + always-on
  `channel_capacity` muted hint (`⟨64⟩`, Addition; click writes `64` into source; hover shows
  default-vs-user-set); override is existing syntax `channel<T>(N)` — no new API; no lint (writing
  the default everywhere is noise, matching the `wait`-insertion precedent). False-sharing padding
  is codegen-only (no typeable form → no muted hint), surfaced via the
  `cross-thread-fields-not-padded` Tier 3 lint only; no user opt-out keyword (boundary cases — FFI
  shapes — are handled at the boundary, per the shape-auto-reorder precedent). Auto-Arc is
  codegen + Informational muted hint (cautionary), no lint (no cleaner user-typeable form exists;
  click jumps to the boundary). Lint rule names follow `prefer-X-when-Y` where applicable
  (`prefer-yielding-sleep`); `cross-thread-fields-not-padded` is the design doc's locked name.

### Teaching
- `channel_capacity` (Addition) + `auto_arc` (Informational, cautionary hover) fire via
  `inlay_hint.rs`; hover text WHAT/WHAT-INSTEAD/WHY; `auto_arc`'s red-tint visual ships or lands as
  the recorded `auto-arc-cautionary-tint` deferral — never a silent skip.
- Every new compile error/warning (kernel-mode channel gate, closed-channel `send()`,
  channel-full backpressure text, auto-Arc boundary diagnostics, handle-form typeck errors) is
  WHAT/WHAT-INSTEAD/WHY, with WHY contextual to the call site.
- Both Tier 3 lints carry three-part text; `prefer-yielding-sleep` is a dismissable suggestion,
  never an error (rare legit uses; respect explicit intent).
- `docs/reference/REF-concurrency.md` teaches the backpressure model explicitly ("suspended
  producer = backpressure working, not a deadlock" — mandated by IMP-no-function-coloring).
- No banned jargon in any new diagnostic (`tests/jargon_audit.rs`).
- VSCode extension version-bumped with screenshots of the new surfaces.

### Runtime Dependencies
- `channel<T>()`: Tokio runtime + `tokio::sync::mpsc` (bundled in `libynz_rt.a`; `sync` feature
  enabled); heap for the bounded buffer.
- Background handle-form: scheduler + independent joinable Tokio task + per-handle channel wiring;
  heap per spawned task tree + handle object (endpoint futures live in the handle object).
- Auto-Arc: atomic refcount ops (acquire-release); heap for the Arc control block.
- False-sharing padding: compile-time layout only — no runtime dependency.
- `[[lint_rule]]` machinery + muted-hint domains: compile-time only.

### Kernel-Mode Behavior
- `channel<T>()` + all channel ops: **COMPILE ERROR** in `--kernel` (NEW gate this milestone —
  none exists today; matches the `wait`/`background` gates at `check.rs:2223-2233`),
  WHAT/WHAT-INSTEAD/WHY pointing at IMP-no-runtime-mode.
- Background handle-form + auto-Arc/suspension paths: COMPILE ERROR in `--kernel` (existing
  `background` gate covers the spawn; the handle typeck inherits it — verified by a gallery
  trigger).
- False-sharing padding: harmless in `--kernel` — pure layout, no runtime dependency; kernel mode
  has no scheduler to create cross-thread access, so the authoritative analysis finds no
  cross-thread fields there anyway.
- `prefer-yielding-sleep`: fires only in NON-kernel programs (kernel steers the opposite way,
  toward `sleepBlocking`); no kernel false positive.

### Demo & Error Gallery
- Each executable phase (P1–P4) extends `examples/pirates-roster/entrypoint.ynz` with its feature
  in realistic context (channel communication, handle-form round-trip, auto-Arc hint,
  false-sharing padding) and adds intentional triggers to
  `examples/primantis-orders/v0_3_m4_errors.ynz` for EVERY new compile-error class (kernel-mode
  channel gate, closed-channel send, handle-form misuse, auto-Arc boundary rejects), each with a
  `// WHY:` comment naming the diagnostic class.
- P5 consolidates the demo, regenerates the byte-exact `expected_stdout.txt` golden via the
  regenerate script, and finalizes the gallery (diagnostic-count + key-phrase assertions in
  `error_galleries.rs`). Byte-exact golden / count+phrase convention — NOT `insta` — for these two.

### Feature Registry Entries
- **New entry-KIND (net-new schema + parser + build.rs + LSP seam, built generic):** `[[lint_rule]]`
  — carrying:
  - New `[[lint_rule]]`: `cross-thread-fields-not-padded`
  - New `[[lint_rule]]`: `prefer-yielding-sleep`
- **New `[[muted_hint_domain]]`:** `channel_capacity` (placement_category: addition), `auto_arc`
  (placement_category: informational).
- **Retire:** `[[deferred_tooling_feature]]` `background-handle-form` (ships in P2).
- **Retire:** `[[deferred_language_feature]]` `ec-wrapper-collect-on-completion`
  (`features.toml:1164` — its own `ships_in = "v0.3-M4"`): the copy-before-free fix ships in P2
  WITH the handle-form, per IMP-concurrency:475/477's gating (r3 correction — the r2 draft's
  "Modify: trigger re-scoped" line was the scope-narrowing Patrick caught; retired, not re-scoped).
- **Conditional (resolved at P0/P3, each recorded with reason):** possibly a `[[keyword]]`/type
  entry for `channel`; possibly `[[deferred_language_feature]]` for the seq-cst opt-in (roadmap
  pre-authorizes); possibly `[[deferred_tooling_feature]]` `auto-arc-cautionary-tint`; possibly a
  `[[deferred_*]]` entry for handle cancel-injection if P2 defers it.
- **`[[diagnostic_template]]`:** kernel-mode-channel gate + closed-channel send + backpressure
  teaching text — added as canonical templates if reused across sites; per-site dynamic messages
  stay in code (confirm at P1).
- **Explicitly NOT added:** no new `[[banned_jargon]]`; no `array-using-soa-layout` (M5, on this
  milestone's mechanism); no new `[[keyword]]` unless the P0 lock says `channel` needs one.

## Design-Doc Alignment

**Governing docs cited:**
[`docs/internal/implementation/IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md),
[`docs/internal/implementation/IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md),
plus the roadmap [`../2026-05-21-v0-3-concurrency-perf/roadmap.md`](../2026-05-21-v0-3-concurrency-perf/roadmap.md)
§Milestone 4, and [`.claude/rules/authoritative-derivation.md`](../../rules/authoritative-derivation.md)
(the project-scoped design-time guard R6 enforces). Both IMP docs were re-read in full for this
correction pass — the plan is diffed against the design, not against the prior draft.

1. **Channel/Queue Primitives** (IMP-no-function-coloring) — bounded, default 64, `⟨64⟩` Addition
   hint with default-vs-user-set in hover, click-writes-`64`, no unbounded constructor
   (`channel<T>(int.max)` is the escape), send-on-full suspends with cascading backpressure, docs
   must teach "suspended producer ≠ deadlock." **Match** — all carried in P1/P5 + Invariants. The
   doc leaves the default constant "TBD via real benchmarking"; the plan locks 64 with the recorded
   reason (§3.1) and a re-tune trigger — a documented resolution of a doc-acknowledged TBD, not a
   divergence.
2. **Atomic Ordering Default** (IMP-no-function-coloring) — acquire-release for channel + Arc ops;
   seq-cst as named opt-in, "final naming TBD." **Match**, with the pre-authorized
   `[[deferred_language_feature]]` path if the opt-in surface exceeds M4 (P0 lock).
3. **False Sharing Auto-Padding** (IMP-no-function-coloring, "Locked Pre-v0.3") — codegen-only,
   64-byte, no typeable form → no muted hint, `cross-thread-fields-not-padded` Tier 3 lint,
   milestone v0.3. **Exact match** (P4).
4. **Auto-Arc cautionary red-tinted hint** (IMP-no-function-coloring, Runtime §4) — the hint ships
   (Informational + cautionary hover); the red-tint VISUAL is staged behind a recorded deferral if
   the editor surface can't render it (recon: no tint path exists in `ynz-lsp`). Implementation
   staging with the teaching content intact — **surfaced explicitly, not a silent divergence**.
5. **No-coloring Invariant** (IMP-no-function-coloring, "Invariant") — channel send/recv suspension
   detected by the may-block analysis, never a type-level marker; nothing here requires callers to
   be marked. **Match — load-bearing; R6's unified classifier is HOW the invariant stays true while
   the set gains a non-name-keyed member.**
6. **Sleep Intrinsics** (IMP-no-function-coloring) — `prefer-yielding-sleep` ships in M4 riding the
   `[[lint_rule]]` infra built here; the doc itself notes M4's handle-form removes the last legit
   non-kernel blocking-sleep use. **Match** (P4).
7. **`background` — two patterns, one keyword** (IMP-concurrency) — the handle-form
   (`monitor.send(...)` / `monitor.receive()`) is the doc's exact locked surface. **Match** (P2).
8. **Ownership across `background`** (IMP-concurrency) — `.share`/`.lend` compile error, `.give`/
   `.copy` inference. Auto-Arc extends this boundary; the reject stays where the design puts it.
   **Match**, with the corrected citations (`check.rs:2275-2280`/`2287-2292`; block at 2253;
   inference at 1183-1197) — the roadmap's `check.rs:1216` is stale (behavior real, line drifted).
9. **Task Cancellation — cancel-via-drop** (IMP-no-function-coloring, "Locked Pre-v0.2") — handle
   drop cancels at the next suspension point, cleanup always runs. P2 implements safe drop under
   this model; if full cancel-injection for channel-suspended children is milestone-sized, the
   deferral is recorded four-field with trigger — **surfaced, not silent** (CCIR g).
10. **`ECWrapperResultCollection`** (IMP-concurrency §463-479) — the design doc gates the
    copy-before-free fix on the `background-handle-form` feature P2 ships, in its own words: cost
    "landing WITH the `background-handle-form` feature" (:475); trigger "collecting the completed
    value … via its handle (`.send`/`.receive`) — gated on the `background-handle-form` feature"
    (:477); registry `ships_in = "v0.3-M4"` (`features.toml:1168`). **Match — the fix is a
    build-blocking P2 deliverable (R8), landing with the feature exactly as the doc requires.**
    History on the record: the r2 draft narrowed the handle to "communication-only" to keep this
    path unreachable and defer the fix — a plan-invented scope-narrowing that contradicted the
    doc's gating language and the roadmap's joinable-handle (result-collection-primitive) framing;
    Patrick's r3 no-duct-tape catch reversed it. The doc's canonical message-loop example
    (:174-179) and the completion-collection trigger describe ONE `.receive()` surface, not two
    competing designs — no design change required of Patrick.
11. **Suspension vs. Ordering / Model A** (IMP-concurrency, LOCKED 2026-06-05) — channel suspension
    is automatic (analysis-detected); `wait` remains ordering-only; nothing in M4 adds a manual
    suspension keyword. **Match.**
12. **Milestone-boundary assumptions** — Auto-SoA + its lint + SIZE_THRESHOLD harness + DAP are M5
    (roadmap-documented deferral; this plan builds the lint MECHANISM generically for M5 but not
    the SoA lint); M3f/M3g fold into `v0.3.0` (Patrick's call, no standalone tags). Both deferrals
    are roadmap-documented, not invented here.
13. **Behavior-claims about untouched code verified at recon** (plan-invariants §Design-Doc
    Alignment #4): every load-bearing claim in ¶1 carries a file:line citation from the second
    recon pass (reject sites, one-shot join, run-once contract, sleep_handle punning, twin
    definitions, missing sync feature, missing kernel gate, version truth) — none inherited from
    the roadmap or the first draft without re-confirmation.

**No un-surfaced divergence.** The two staged items (seq-cst opt-in, red-tint visual) and the one
remaining doc-flagged contingency (cancel-via-drop scope) are documented deferrals/resolutions with
triggers, not silent divergences; the EC-wrapper fix is in-plan P2 work (r3 — no longer a
deferral of any kind). Nothing requires Patrick to change a design doc.

## Future Requirements / Revisit

Durable punt-list — every risk-engine output the gate did not clear to nothing, plus non-risk
deferrals; each entry: what · why-deferred · cost · trigger.

- **R6 residual MEDIUM (recorded).** What: the may-block set, even unified, is a single point every
  future suspension source must thread. Why-parked: residual M after unification + parity tripwire.
  Cost: n/a (mitigated in-plan). Trigger: any new suspension source (M5's SoA has none, but future
  I/O intrinsics do) → MUST extend the one classifier, never a consumer-local list; the parity test
  is the tripwire.
- **R3 residual MEDIUM (recorded).** What: auto-Arc boundary false-Arc/false-reject hazard. Why-
  parked: residual M after the exhaustive matrix. Cost: n/a. Trigger: any new ownership form or
  `background`-boundary shape not covered by the P3 matrix (extend the matrix first).
- **Runtime deadlock/hang observability (agent-found gap, factor: observability).** What: no
  runtime telemetry exists to diagnose a hang in the field; build-time fixtures are the only
  control. Why-deferred: runtime deadlock detection is real design work (IMP-no-function-coloring
  lists it as an open question) and not required while fixtures gate every known path. Cost: a
  design pass + runtime instrumentation (~1 milestone-lite). Trigger: first field report of a hang,
  or the v0.4 concurrency-polish milestone.
- **Seq-cst opt-in API (pre-authorized deferral, resolved at P0).** What: the global-total-order
  opt-in (`.withGlobalOrdering()` candidate, naming TBD). Why-deferred: surface likely exceeds M4
  (roadmap-authorized). Cost: a focused API-design pass. Trigger: a real workload needing seq-cst,
  or v0.4. Lands as `[[deferred_language_feature]]` at P0 if deferred.
- **`auto_arc` red-tint visual (deferred if unsupported at P3).** What: cautionary tint rendering.
  Why-deferred: no per-hint tint path exists in `ynz-lsp`/VSCode inlay hints (recon-confirmed
  net-new). Cost: a decoration-based renderer or editor capability. Trigger: hints migrate to a
  decoration renderer, or the editor supports per-hint tinting. `[[deferred_tooling_feature]]`
  `auto-arc-cautionary-tint`.
- **Handle cancel-injection scope (contingent, resolved at P2).** What: injecting cancellation into
  a channel-suspended child per the locked cancel-via-drop model. Why-contingent: safe-drop
  (no leak/no UB) is committed; full injection may be milestone-sized. Cost: estimated at P2 if
  deferred. Trigger: P2's surfaced verdict; if deferred, a `[[deferred_*]]` entry + the next
  concurrency milestone.
- **Channel default-capacity re-tune.** What: the locked 64 constant. Why-parked: no workload data
  exists; the doc's pre-v0.2 benchmarking never happened. Cost: one-constant change + hover-text
  update. Trigger: real workload evidence that 64 mis-sizes typical backpressure behavior.
- **Roadmap corrections (non-risk).** What: the roadmap's stale `check.rs:1216` citation and the
  Capability Ledger's stale `active` row for `v0-3-m3d-cpu-parallelization` (its plan is `done`).
  Why-parked: the roadmap is a separate artifact this plan does not edit. Cost: two one-line edits.
  Trigger: next roadmap-maintenance pass.
- **Auto-SoA × padding interaction (owned by M5).** What: two layout transforms on one shape. Why-
  parked: SoA is M5; M4 ships padding with no SoA present. Trigger: M5 planning must resolve
  composition/precedence — and M5's lint lands on this milestone's `[[lint_rule]]` mechanism.
