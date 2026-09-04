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
| Channel feeders | 1–2 × `function feedN(lend wire: channel<int>)` — sends 1–3 values, then `close()` |
| Shape readers | one per declared shape — `function weighN(item: CrateN, out: channel<int>)` |

`entrypoint` body statements, drawn by weight from a 14-way menu, 10–19 statements per program:

- independent `let` bindings and small int expressions (`+`, `-`, and `*` between literals only);
- `print(v.toString())` and interpolated `print(\`v is ${v.toString()}\`)`;
- calls to the pure helpers;
- `wait fetchN(k)` — always `wait`-prefixed (see "Why the grammar is shaped this way" below);
- `array<int>` literals, `.add()`, `.count()`, in-range `arr[i].or(-1)`;
- `map<string, int>` literals, index read `.or(0)`, index assign, `.count()`;
- shape literals and field reads;
- inline channels: `channel<int>(cap)`, `send`, `receive` + `.or(0)`, and — half the time —
  `close()` followed by a receive past end-of-stream (the v0.3-M8 Phase 4 contract: `none`, never
  a hang, never an error);
- **handle-form spawn**: `let h = background fetchN(k)` then a blocking `h.receive()`;
- **the taught drain-loop idiom**: `background feedN(wire)` + a `while` loop on
  `.exists()`/`.value` until end-of-stream;
- **the two-spawn Auto-Arc topology**: the same read-only shape binding handed to two
  `background` spawns with no write between them, each task reporting through one channel, and
  the caller reading its own view of the shape afterwards.

Roughly 90% of generated programs spawn `background` work — the surface this milestone is about.

## What it deliberately does NOT cover, and why

- **Owned-heap channel payloads** (`channel<array<int>>`, `channel<map<…>>`). v0.3-M8's transfer
  rules (`ConsumedBySend`, `TransferNeedsCopy`, `ParamNeedsGive`) require `.copy()`/`give`
  plumbing the grammar does not model; emitting them would produce correctly-REJECTED programs
  that read as harness failures. Every channel is `channel<int>`, locked by a generator test.
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
