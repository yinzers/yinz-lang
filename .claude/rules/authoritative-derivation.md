# Authoritative Derivation — Thread the One Source, Don't Re-Derive It

**Load when**: designing or reviewing any compiler pass, guard, or codegen path that consumes a
DERIVED answer another pass already computes — a crossing/suspend set, an ABI/aliasing predicate, an
admission-gate decision, a "declared-before-`wait`" membership test — or anywhere two or more code
paths must AGREE on the same computed question.

This is the project-scoped, design-time limb of the global `no-duct-tape.md` #7 / the "Parallel
Implementation" failure (`~/.claude/docs/reference/REF-no-duct-tape.md` §5). The graveyard catches
specific greppable *instances* after they ship; this rule stops the *class* at the moment you reach
for a second derivation.

---

## The rule

> When two or more code paths must agree on a derived question, thread the SAME authoritative
> value/query into all of them. Never let a second surface re-derive its own "equivalent" answer.

A fresh re-derivation that is *currently equivalent* at today's call sites is exactly what silently
diverges tomorrow — when a third call site, a new value type, or a new intrinsic changes one
derivation and not its twin. In this compiler the divergence is usually **silent-wrong** (no crash:
wrong output across a suspension, or an under/over-sized frame that detonates as SIGILL later), so the
default is one source, not two "kept in sync by hand."

## Why this earns its own rule — a four-milestone recurrence

The same disease has shipped silent miscompiles across four consecutive milestones:

- **M3a** — two parallel per-type frame-flush dispatches drifted (`number`/`shape`/`string` branches
  present in one, wrong in the other) → `0.000`/stack-garbage across `wait`; ~10 whack-a-mole rounds.
- **M3d** — an admission gate re-derived its decline predicate wider than the real suspend set → 11 of
  17 fixtures silently stopped firing while every output test stayed green.
- **M3e** — an injected frame-size resolver was bypassed by memo ordering; the re-derived fallback (32)
  coincided with the resolved value → dead resolver, SIGILL on any non-trivial callee frame.
- **M3g** — three in one plan: an admission decline re-scanning the AST instead of reading the
  authoritative `base_suspends` set; a hint pass classifying against a narrower re-derived suspend set
  instead of codegen's real unioned set; two independently-computed ABI predicates with no
  compile-time link between them.

Three of these already have graveyard corpses (see Cross-references). This rule is the design-time
guard so the fifth occurrence never gets written.

## Apply it

When you catch yourself about to compute "is X in the crossing set?" / "does this suspend?" / "what's
this callee's frame size?" a SECOND time:

1. **Find the authoritative producer** (`crossing_local_names` / `locals_crossing_wait` /
   `base_suspends` / codegen's unioned suspend set / `flush_var_slot_to_frame` / the frame-size
   resolver) and thread ITS output into your consumer.
2. **If the authoritative value isn't reachable** from where you need it, thread it there — pass it as
   a parameter, seed the memo before the pass runs, add a query. Do NOT re-derive a local copy because
   plumbing the real one is inconvenient; the inconvenience is the tell, not the excuse.
3. **If two predicates genuinely must live apart**, give them a compile-time link — one defined in
   terms of the other, or a parity test asserting they agree — so a future edit to one breaks loudly
   instead of drifting silently.

## Not this

Not "never compute a derived value" — deriving it ONCE, in its owning pass, is correct. The ban is on
the SECOND, parallel derivation of the same question. Extending the single authoritative producer
(adding a value-type arm inside the one dispatch, a case inside the one analysis) is right; forking a
second producer is the bug.

## Cross-references

- Global parent: `no-duct-tape.md` #7 / `REF-no-duct-tape.md` §5 "The Parallel Implementation" — this
  compiler's analysis passes are a recurring, silent-wrong specialization worth strengthening the
  global catalog with (flagged to Patrick from the M3g AAR).
- [`.claude/graveyard.md`](../graveyard.md) "Parallel Per-Type Dispatch / Flat-Scan Re-Derivation in Suspension Codegen"
- [`.claude/graveyard.md`](../graveyard.md) "Injected Resolver Dead via Memo-Cache Ordering"
- [`.claude/graveyard.md`](../graveyard.md) "Silent Envelope Narrowing"
