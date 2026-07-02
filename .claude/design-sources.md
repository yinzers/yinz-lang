# Design-Source Registry
# Format spec: ~/.claude/memory/design-sources.md
# Bindingness: [locked] = gate blocks on unjustified contradiction; [aspirational] = gate warns.
#
# POLICY (Patrick, 2026-06-13): if it's designed, it's locked.
#   "Locked" does NOT mean frozen. A doc that turns out wrong gets UPDATED in place
#   (that IS the fix, per no-duct-tape.md) — but no plan or diff may *silently contradict*
#   a doc. To diverge you either change the doc or record a real named-cost rationale in
#   the plan's `## Design Divergences` section. We do not keep half-committed
#   "aspirational" docs around: anything written down is a commitment. Hence every entry
#   below is [locked] and the [aspirational] tier is intentionally unused. The only
#   "aspirational" thing in this project is a design we haven't WRITTEN yet — and an
#   unwritten design isn't a doc, so it never appears here.

# ---- Broad coverage: every spec + design doc is binding ----
- [locked] spec/**/*.md     — (external) public language spec users write code against; a diff that contradicts it must update the spec, never route around it
- [locked] design/**/*.md   — (internal) governing source of truth per CLAUDE.md; recursively covers design/ top-level + design/future/ + design/stdlib/

# ---- High-stakes docs called out for stakes-context + navigation ----
# (Already covered by the design/**/*.md glob above — listed here so the design-compliance
#  reviewer gets the "this is load-bearing / caused a HALT" context, per the format spec's
#  domain-note guidance. No tier override: everything is locked.)
- [locked] docs/internal/implementation/IMP-concurrency.md             — (internal) auto-parallelization design + suspension semantics + wait-ordering model (LOCKED 2026-06-05)
- [locked] docs/internal/implementation/IMP-no-function-coloring.md      — (internal) no-coloring async model + channel/scheduler primitives. The v0.3-M2 HALT was caused by a plan contradicting THIS doc without ever diffing against it. (Was design/future/concurrency.md until 2026-06-13 — moved out of future/ because v0.3 is implementing it now.)
- [locked] docs/internal/implementation/IMP-no-runtime-mode.md  — (internal) kernel-mode plug-in allocator API; plan-invariants.md treats `--kernel` behavior as a hard per-milestone contract, so this doc is load-bearing now
- [locked] docs/reference/REF-ide-hints.md               — (internal) muted-hint placement categories + hover format; governs wait_points + background_routing hint implementation
