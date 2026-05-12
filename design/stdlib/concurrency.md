# Standard Library — Concurrency

**Status**: Concurrency is a language-level feature, not a stdlib module. Full design and rationale lives in `design/concurrency.md`.

Standard library additions that support concurrency patterns:

- `await all(task1, task2, ...)` — wait for multiple background tasks at once
- Batch processing utilities for parallel loop patterns (design TBD)

See `design/concurrency.md` for complete design decisions, rationale, and open questions.
