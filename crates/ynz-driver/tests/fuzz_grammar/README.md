# Structured fuzzing harness — scope, mode matrix, budget, replay

Design note for the v0.3-M8 Track 4b harness. The generator is `mod.rs` beside this file; the
oracle it feeds is `../cross_impl_consistency.rs`.

---

## What it is

A **grammar-constrained** `.ynz` program generator. It emits only constructs it has already
typed, drawing every operand from a typed environment it maintains as it builds — so a generated
program is **type-valid by construction**. Token-level fuzzing is deliberately not what this is:
an unconstrained generator would spend its entire budget re-proving that typeck rejects garbage,
and every rejection would read as a harness failure rather than a compiler finding.

The consequence is a sharp contract worth stating plainly: **a generated program that fails to
compile is a GENERATOR bug, not a compiler finding.** The sweep reports it under that exact
heading.

## What the grammar covers

Declarations, emitted per program from the seed:

| Construct | Shape emitted |
|---|---|
| Shapes | 0–2 × `shape CrateN { weight: int, label: string }` |
| Pure helpers | 1–2 × `function tallyN(a: int, b: int) -> int` |
| Suspending helpers | 1–2 × `function fetchN(n: int) -> int { wait sleep(1) … }` |
| Channel feeders | 1–2 × `function feedN(lend wire: channel<T>)` — sends 1–3 values of `T`, then `close()`; `T` is drawn from `{int, array<int>, map<string,int>, number}` |
| Shape readers | one per declared shape — `function weighN(item: CrateN, out: channel<int>)` |

`entrypoint` body statements, drawn by weight from a 14-way menu, 10–19 statements per program:

- independent `let` bindings and small int expressions (`+`, `-`, and `*` between literals only);
- `print(v.toString())` and interpolated `print(\`v is ${v.toString()}\`)`;
- calls to the pure helpers;
- `wait fetchN(k)` — always `wait`-prefixed (see "Why the grammar is shaped this way" below);
- `array<int>` literals, `.add()`, `.count()`, in-range `arr[i].or(-1)`;
- `map<string, int>` literals, index read `.or(0)`, index assign, `.count()`;
- shape literals and field reads;
- inline channels: `channel<T>(cap)`, `send`, `receive`, and — half the time — `close()` followed
  by a receive past end-of-stream (the v0.3-M8 Phase 4 contract: `none`, never a hang, never an
  error). `T` is drawn from `{int, array<int>, map<string,int>, number}` (v0.3-M8 Phase 8 fix
  round 3 — see "The owned-heap widening" below); `int` receives use `.or(0)`, the other three
  kinds use `.exists()`/`.value`;
- **handle-form spawn**: `let h = background fetchN(k)` then a blocking `h.receive()`;
- **the taught drain-loop idiom**: `background feedN(wire)` + a `while` loop on
  `.exists()`/`.value` until end-of-stream, `wire`'s element kind matching `feedN`'s;
- **the two-spawn Auto-Arc topology**: the same read-only shape binding handed to two
  `background` spawns with no write between them, each task reporting through one channel, and
  the caller reading its own view of the shape afterwards.

Roughly 90% of generated programs spawn `background` work — the surface this milestone is about.

## The owned-heap widening (v0.3-M8 Phase 8 fix round 3)

Channel element types were, until this round, locked to `int` — the prior text here claimed
`array<int>`/`map<string,int>` payloads "would produce correctly-REJECTED programs" because
v0.3-M8's transfer rules (`ConsumedBySend`, `TransferNeedsCopy`, `ParamNeedsGive`) "require
`.copy()`/`give` plumbing the grammar does not model." **That claim was checked directly and was
false.** A `let` built inside a feeder function (or directly in `entrypoint`, never read again
after its send) needs neither: nothing else in the program reads it back, so no consume-tracking
diagnostic can fire. The only real bookkeeping the widening needed was keeping a SENT binding out
of the pool a later statement could read from (`take_or_make_array`/`take_or_make_map` in
`mod.rs`) — verified by hand with three programs in the generator's own idiom before the widening
landed (a `background` feeder + drain loop for `channel<array<int>>`; a `channel<map<…>>` and a
`channel<number>` in one program; an inline `channel<array<int>>` send with no `background`
involved at all), all three compiling and running clean, byte-identical across the full mode
matrix. `channel<number>` exercises fr12's copy-through contract directly: the SAME `number`
binding is sent more than once and is still readable afterward, proving it never joins the give
set.

**The widening surfaced two genuine runtime defects**, not generator bugs — full repro, bisection,
and disposition in Future Requirements #11 (`plan.md`) and the v0.3-M8 plan's `audit.md`,
FRAGO 015:

1. An `array<int>`/`map<string,int>` LOCAL declared before ANY suspension point in `entrypoint`
   (a `wait`, a `background`-handle `.receive()`, a channel `.receive()`) and later `.send()`-ed
   into a channel AFTER that suspension reads back corrupted (`SIGABRT`, null/misaligned pointer
   in `ynz_map_count`/`ynz_array_count`). A fresh local built and sent immediately, or the same
   local declared AFTER the suspension, is unaffected — confirmed both ways.
2. A `background` producer forced to actually BLOCK on a full channel buffer
   (`send_count > capacity`) can read back a HEAP ADDRESS where a sent value belongs — an
   uninitialized-or-freed read, general to both `int` and `number` (NOT `number`-specific as
   first recorded) and non-deterministic (~17-30% of runs, NOT the fixed rate first recorded). 2
   sends into capacity 1, or 3 sends into capacity 4 (neither forces the producer to block), are
   both unaffected. v0.3-M8 Phase 8 fix round 4 corrected this record against fix round 3's
   original claim, which had all three of those wrong (`number`-only, deterministic, an
   arithmetic "lost value" rather than a garbage read).

Neither is fixed inline (per this plan's CCIR item 5 / risk R5 — a genuine finding routes through
the plan-amendment seam, never a same-round patch). Both are avoided at the generator level with
narrow, evidence-backed guards documented at their exact code sites (`take_or_make_array`'s doc
comment for #1; the capacity computation in `stmt_background_drain_loop` for #2) — neither guard
narrows an entire element kind or construct; each targets only the specific shape that reproduces.
Verified with two independent 256-program local runs (seed bases 0 and 31337000) after landing
both guards: 256/256 compiled and ran, 0 findings, both runs.

**Fix round 4, BLOCKER 1: the reuse-from-pool branch this round's defect #1 guard was built
around was dead code from the moment it landed.** `stmt_channel_inline_kind` set
`Builder::suspension_seen` at the TOP of the composite — before the Array/Map arms below it ran
— so `take_or_make_array`/`take_or_make_map`'s `!suspension_seen` reuse check was already false
by the time either function's only caller reached it. Fixed by moving the assignment to AFTER
the composite's own sends and receives (see the comment at that call site in `mod.rs`); a
`fired_pool_reuse` counter and its floor in `per_construct_floors_hold_over_a_fixed_corpus` now
guard against this construct silently going dead again.

## What it deliberately does NOT cover, and why

- **Un-`wait`ed adjacent suspending calls.** Two of those form an auto-parallel I/O group whose
  members may legitimately finish in either order — the documented Model-A intended reorder
  (`IMP-concurrency.md`; the hand-written corpus carries `v0_3_m3b_p4_model_a_intended_reorder.ynz`
  as an oracle exclusion for exactly this). The oracle would flag it as a divergence; it is not
  one. Locked by a generator test.
- **`.close()` on a channel a `background` feeder was spawned on.** Send-after-close from an
  in-flight task is a refusal path with its own dedicated fixtures; the generator does not
  stumble into it blindly.
- Modules/imports, `errors`, `options`, `dynamic`, generics, `for`-in, nested functions. Each is
  a candidate for a later widening of the grammar; none is needed for the concurrency surface
  Track 4b exists to cover.
- **`background` argument-evaluation order** (parked item 41). Every argument the grammar hands
  to a `background`-spawned call is restricted to a plain identifier or a literal — never a call
  expression with a side effect, so the grammar cannot emit a program shaped like
  `f(x, mutate(x))` where a plain call and a `background` spawn are entitled to disagree on
  whether the callee sees `x` before or after `mutate` runs. This harness does not exercise that
  question at all; it is not a gap this generator closes by construction, it is a construct the
  grammar never reaches for.
- **A blocked channel send (producer forced to wait for buffer space).** v0.3-M8 Phase 8 fix
  round 3's guard against defect #2 above (`stmt_background_drain_loop`'s
  `send_count.max(1 + below(4))` capacity floor) guarantees the consumer's channel capacity is
  never smaller than the producer's own send count — so that composite's producer can never
  block. The other two channel composites already couldn't block by construction
  (`stmt_channel_inline_kind` draws its send count FROM the capacity; the two-spawn Auto-Arc
  topology hardcodes `channel<int>(4)` against 2 sends). Corpus-wide, that leaves ZERO coverage
  of the blocked-send path — coverage this generator HAD before this round, and lost as the
  direct cost of containing defect #2. Restoring it is exactly the trigger recorded for defect
  #2 in Future Requirements #11 / the v0.3-M8 plan's `audit.md`, FRAGO 015: fix the blocked-send path, then let
  `stmt_background_drain_loop`'s capacity floor go back to drawing independently of
  `send_count`.

## Why the grammar is shaped this way (determinism)

The oracle compares programs byte-for-byte. Two properties make that legitimate here:

1. **Helpers never print.** Only `entrypoint` produces output, so a reordering the auto-parallel
   pass is entitled to make cannot move a print.
2. **Every receive is accounted for.** Every channel composite the grammar can emit is
   self-balancing by construction — it emits its own sends and its own matching receives inside
   one statement, so no generated program parks forever. (The corpus sweep also carries its own,
   independent liveness bound — see "The budget" below — as a backstop against a future grammar
   arm separating a send from its receive across statements.)

Order relaxation itself is **not** re-derived here. The oracle's
`output_order_is_scheduler_dependent` reads the SOURCE for `background`, so a generated
concurrent program is auto-classified with no exclusion list to maintain — the same authoritative
classifier the hand-written corpus runs under (`authoritative-derivation.md`). What stays strict
for every generated program, concurrent or not: the exit code, the stderr text, and the complete
multiset of stdout lines.

## The mode matrix

Each generated program is compiled and run across the full 2×2:

| | optimized (default) | `YNZ_NO_OPTIMIZE=1` |
|---|---|---|
| **auto-parallel (default)** | baseline | compared |
| **`YNZ_NO_AUTO_PARALLEL=1`** | compared | compared |

The three non-baseline corners are compared pairwise against the baseline, which is transitively
all-pairs equality. Both axes must be observably invisible: auto-parallel changes *when* work
runs, the optimizer changes *how fast* — neither may change what the program observes. A
divergence is a silent miscompile.

## The budget

Bounded three ways, none of them open-ended:

| Bound | Value | Where |
|---|---|---|
| Corpus size, local default | 24 programs | `FUZZ_DEFAULT_PROGRAMS` |
| Corpus size, CI | 96 programs | `YNZ_FUZZ_PROGRAMS` in `.github/workflows/ci.yml` |
| Per (program × mode) liveness kill | 90s | `FUZZ_RUN_BUDGET` |
| Whole CI job wall clock | 30 min | `timeout-minutes` on the `fuzz` job |

Measured on a 16-core dev container: 64 programs × 4 modes = **14.5s**; 256 programs × 4 modes
(1,024 compile+link+execute invocations) = **60.6s**. That is ~59ms of wall clock per invocation
at this parallelism, so the 90s per-invocation bound is a liveness timeout three orders of
magnitude above the
observed cost, per `~/.claude/rules/testing.md` — it exists to catch a hang, never to assert
performance.

The CI job is **non-blocking on first landing** (`continue-on-error: true`), with its
promote-to-blocking trigger written beside the flag. It carries a three-part vacuity guard: zero
programs generated, zero compiled, or zero tests matched all FAIL the step.

**A genuine finding does not get an inline fix.** It routes through the plan-amendment/FRAGO seam
(risk R5) instead — never a silent same-round patch — until the lane has proven it does not flake
(the thirty-consecutive-clean-runs trigger above). This is the same rule stated beside the
`continue-on-error` flag in `.github/workflows/ci.yml`; it lives here too because a finding is
usually read first from a local run, not from that CI comment.

## Replay

A finding is reproducible from its **seed** alone (with the generator revision). Every generated
file carries its seed in the filename and in a header comment, and the sweep prints the seed base
on every run.

```bash
# Read the exact program a finding named:
YNZ_FUZZ_SEED=<seed> cargo test -p ynz-driver --test cross_impl_consistency \
  print_generated_program -- --ignored --nocapture

# Re-run a slice:
YNZ_FUZZ_SEED=<seed> YNZ_FUZZ_PROGRAMS=8 cargo test -p ynz-driver \
  --test cross_impl_consistency generated_corpus -- --nocapture
```

On failure the sweep **keeps** its scratch directory (it prints the path) and embeds the full
generated source in the failure text, so a finding survives even a later generator revision that
would change what a seed produces.

**Where an interesting case is promoted.** A generated program that reproduces a genuine
miscompile does not stay in the fuzz corpus — a seed is a weak pin, since any grammar change
invalidates it. It gets copied verbatim into `crates/ynz-driver/tests/fixtures/` as a named
fixture, at which point the hand-written sweeps above pick it up automatically and it is covered
forever regardless of what the generator does next. The fuzz corpus finds cases; `fixtures/` is
where they live.
