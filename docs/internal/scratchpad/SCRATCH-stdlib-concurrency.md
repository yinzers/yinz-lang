---
name: "SCRATCH-stdlib-concurrency"
description: "Short scratchpad note listing the few stdlib additions (e.g. await all()) that support concurrency, clarifying concurrency itself is a language feature, not a stdlib module."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Standard Library — Concurrency

**Status**: Concurrency is a language-level feature, not a stdlib module. Full design and rationale lives in [`docs/internal/implementation/IMP-concurrency.md`](../implementation/IMP-concurrency.md).

Standard library additions that support concurrency patterns:

- `await all(task1, task2, ...)` — wait for multiple background tasks at once
- Batch processing utilities for parallel loop patterns (design TBD)

See [`docs/internal/implementation/IMP-concurrency.md`](../implementation/IMP-concurrency.md) for complete design decisions, rationale, and open questions.
