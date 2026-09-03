# Test Parallelism — Project Limb

**The principle lives globally**, in `~/.claude/rules/testing.md` (independent per-item work runs
in parallel; serial earns a named reason; wall-clock budgets in a parallel lane are liveness, not
performance) and `~/.claude/rules/tooling.md` (in Docker, throwaway paths belong on tmpfs). This
file carries only what is specific to THIS repo — the canonical implementation, the measured
numbers, and the local corpse list. It does not restate the rule.

**Load when**: adding a fixture/corpus/sweep-style harness here, or when a test's wall-clock cost
comes up in this project.

---

## The canonical implementation

`crates/ynz-driver/tests/cross_impl_consistency.rs::parallel_sweep` — this is the
`parallel_sweep` the global rule names. Reuse it rather than writing a second one:

- `std::thread::scope` — no added dependency, borrows the corpus slice directly.
- **Atomic cursor, not pre-chunked ranges.** Per-fixture cost here varies by more than an order
  of magnitude (a two-line program vs. a suspension-heavy state machine); static chunks strand
  workers behind whichever chunk drew the slow tail.
- `available_parallelism()`, capped at item count — self-scales from a 16-core workstation to a
  4-core CI runner with no tuning knob.
- **Findings sorted before asserting.** Completion order is nondeterministic; failure output
  must not be.

## Local independence facts, already established

Do not re-derive these when parallelizing something here:

- **`ynz run` is concurrency-safe on disk.** It builds into a per-invocation temp directory
  (random name, mode 0o700) — see the contract on `crates/ynz-driver/src/run.rs::run`. Concurrent
  invocations cannot collide.
- **`/tmp` is tmpfs in the dev container**, with `exec` (mandatory — `ynz run` links a binary
  there and executes it). See `docker-compose.yml`.

## Profile BEFORE parallelizing — the expensive lesson from 2026-09-02

**A slow test is not evidence of a slow loop.** It may be a fast loop calling one slow function,
and parallelism cannot divide a single call.

Two suites here — `ynz-fmt/tests/idempotency.rs` (177.9s) and `semantic_roundtrip.rs` (89.5s) —
looked like textbook targets for this rule: serial `for` loops over independent files. Both were
parallelized under this rule. **The change bought 2 seconds.**

The real cause was `ynz_fmt::format()` itself taking **91 seconds on a single 1,352-line file**
(`comment_merge::line_of` rescanned the source from byte 0 on every call, compounding to
O(nodes × comments × filesize)). The suites were 177.9s and 89.5s because they call that one
function twice and once respectively — 91+91 and 91. Sixteen cores cannot split one 91-second
call. Fixing `line_of` took both suites to **0.38s and 0.14s**; the parallelism contributed
almost nothing.

So, before applying this rule to a slow test:

1. **Time the per-item work in isolation** (a `--example` probe against one representative item
   is usually enough). If a single item is most of the suite's runtime, the loop is not the
   problem and parallelizing it is a symptom patch.
2. **Split the phases.** Timing `parse` / `lex` / `format` separately is what localized the
   above to 100% inside one crate in a single run.
3. **Then parallelize** — for the items that are genuinely many and genuinely independent.

Parallelizing first is exactly the "patch the product, not the producer" failure
`~/.claude/rules/root-cause.md` names; this entry exists because it was committed here in full,
on the very rule that forbids it.

## Measured, 2026-09-02 — why this is load-bearing here

| | before | after |
|---|---|---|
| `cross_impl_consistency` (2 tests) | 2,291.8s | **154.9s** (14.8x) |
| Share of full-suite wall clock | 73.7% | ~16% |
| Host during the sweep | 81% iowait, 385ms write latency | **0% iowait** |

Two tests held three-quarters of the entire suite's runtime while using one core of sixteen. The
fix was parallelism *plus* tmpfs — parallelism alone moved the bottleneck from CPU to disk and
bought far less than it appeared to.

## Local corpses — the wall-clock class

Four instances in one session (2026-09-02), all the same producer: a fixed time budget calibrated
on an idle machine, invisible until the environment changed.

| Test | Assumption | Broke under |
|---|---|---|
| `panic_reraises_in_parent` | 50ms sleep, then a single poll | TSan (fixed, `12f397b`) |
| `check_preempt_noop_..._acceptable` | 200ns/call budget | TSan, real ~229ns (deleted, `67b3148`) |
| `test_cross_file_reference_count_..._fast` | wall-clock completion | (deleted, `87a4674`) |
| 9 × `*_is_deterministic_across_runs` | 10s `RUN_TIMEOUT` per `ynz run` | nextest contention — **OPEN** |

The last row is unfixed. Under contention *passing* tests took 22–28s against a 10s budget, and
all 9 failures were the timeout assertion — **not one was a value mismatch**. Nine red tests, zero
real bugs. Widening those budgets is what unblocks `cargo nextest` here.

## Cross-References

- `~/.claude/rules/testing.md` — the parent rule; the principle's SSOT
- `~/.claude/rules/tooling.md` — the tmpfs bullet, with this repo's measurement as its evidence
- [`.claude/graveyard.md`](../graveyard.md) — corpse candidate: *wall-clock budget calibrated on an
  idle machine*; detection signature `Duration::from_(secs|millis)` in a test asserting
  `!timed_out` or elapsed-under-N
