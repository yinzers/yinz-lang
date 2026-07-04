---
name: "SCRATCH-future-auto-soa"
description: "GRADUATED (v0.3-M5) — auto-SoA shipped; the living design record is now IMP-collections.md's 'Auto-SoA layout (v0.3-M5)' section. This file is a pointer stub."
tags:
  - "yinz-compiler"
created_at: "2026-05-15"
updated_at: "2026-07-04"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Auto-SoA Transformation — GRADUATED (shipped v0.3-M5)

**This design shipped in v0.3-M5** (plan `2026-07-03-v0-3-m5-auto-soa`, Phases 4–5, riding the
by-value element-storage substrate from Phases 2–3). The living design record is now:

- [`docs/internal/implementation/IMP-collections.md` — "Auto-SoA layout (v0.3-M5)"](../implementation/IMP-collections.md#auto-soa-layout-v03-m5)
  (admission criteria, one-layout-authority + padding-wins precedence, kernel-mode gate, Tier 3
  lint teaching surface, honest O0/-O2 performance provenance, deferral triggers)
- [`docs/internal/implementation/IMP-collections.md` — "Array element storage — by-value inline (v0.3-M5)"](../implementation/IMP-collections.md#array-element-storage--by-value-inline-v03-m5)
  (the substrate SoA rides: one-allocation buffer, snapshot-on-persist semantics, field-wise value
  `contains`, drop parity, serialization forward-compat)

This file is kept as a pointer stub (historical links + the `design_future_sync` skip list name
it). Do not add design content here — amend the IMP-collections sections instead.
