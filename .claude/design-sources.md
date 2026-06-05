# Design-Source Registry
# Format spec: ~/.claude/memory/design-sources.md
# Bindingness: [locked] = gate blocks on unjustified contradiction; [aspirational] = gate warns

# Concurrency — load-bearing design docs; the M2-HALT was caused by a plan
# contradicting these without ever diffing against them. Both are locked so
# the design-compliance gate blocks on any unjustified divergence.
- [locked] design/concurrency.md                — (internal) auto-parallelization design + suspension semantics + wait-ordering model (LOCKED 2026-06-05)
- [locked] design/future/concurrency.md         — (internal) no-coloring model + channel/scheduler primitives; "future/" path is misleading — this doc is load-bearing for v0.3 and later
- [locked] design/ide-hints.md                  — (internal) muted-hint placement categories + hover format; governs wait_points and background_routing hint implementation
