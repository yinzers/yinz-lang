---
name: "SCRATCH-future-array-by-value-element-storage"
description: "GRADUATED (v0.3-M5) — by-value inline element storage shipped (ArrayShapeRuntimeFieldWithWait lifted); the living design record is now IMP-collections.md's 'Array element storage — by-value inline (v0.3-M5)' section. This file is a pointer stub."
tags:
  - "yinz-compiler"
created_at: "2026-06-04"
updated_at: "2026-07-04"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Array By-Value Element Storage — GRADUATED (shipped v0.3-M5)

**This design shipped in v0.3-M5** (plan `2026-07-03-v0-3-m5-auto-soa`, Phases 2–3 — the folded-in
`v0-3-m3c-array-by-value` work; the M3a interim guard `ArrayShapeRuntimeFieldWithWait` was lifted
in Phase 3). The living design record is now:

- [`docs/internal/implementation/IMP-collections.md` — "Array element storage — by-value inline (v0.3-M5)"](../implementation/IMP-collections.md#array-element-storage--by-value-inline-v03-m5)
  (the forcing stack-dangling bug, the one-allocation buffer decision + rejected alternatives,
  element-blind drop parity, field-wise value `contains`, copy-on-persist snapshot semantics with
  the TS-aliasing teaching note, serialization forward-compat)
- What the guard lift removed: [`docs/internal/implementation/IMP-concurrency.md`](../implementation/IMP-concurrency.md)
  (v0.3-M5 Phase 3 lift record + acceptance coverage)

This file is kept as a pointer stub (historical links + the `design_future_sync` skip list name
it). Do not add design content here — amend the IMP-collections sections instead.
