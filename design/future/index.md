# Future Designs — Index

This directory holds design docs for decisions LOCKED but NOT YET IMPLEMENTED. Most target v0.2 or v0.3+. The principles are decided; the implementation is on a future milestone's roadmap.

Each doc in this directory has a **Status header** stating one of:
- **Locked, v0.2 implementation** — design fully decided, ships in v0.2 alongside stdlib/concurrency work
- **Locked, v0.3+ implementation** — design fully decided, ships in a later milestone
- **Decision-needed-before-implementation** — design leaning toward an approach but Patrick must confirm during the implementation milestone's planning

---

## Designs Locked Here

| Doc | Status | Target | One-line description |
|-----|--------|--------|----------------------|
| [`concurrency.md`](concurrency.md) | Locked | v0.2 | No-function-coloring async — compiler does whole-program may-block analysis, IDE shows muted `wait` hints, FFI annotated explicitly. |
| [`panic-safety.md`](panic-safety.md) | Locked | v0.2 | Errors auto-propagate; panics auto-isolate to `background` tasks. NO try/catch ever. Drop-on-scope-exit + supervisor pattern. |
| [`supervisor.md`](supervisor.md) | Locked | v0.2 | Stdlib supervisor helpers (`supervise.alwaysRestart`, `.withBackoff`, `.maxRestarts`, `.onPanic`). Meta-rule: any long-running stdlib loop is supervised by default. |

**v0.3+ deferred designs**:

| Doc | Status | Target | One-line description |
|-----|--------|--------|----------------------|
| [`self-references.md`](self-references.md) | Locked (Approach A) | v0.3+ | Self-referential shapes via relative pointers (~1 cycle/access). Opt-in via `self-referential` modifier or compiler-inferred. |
| [`no-runtime-mode.md`](no-runtime-mode.md) | Locked | v0.3 | `--kernel` (or `--bare`) flag — plug-in runtime architecture for chipset/NASA targets. |
| [`arena.md`](arena.md) | Locked | v0.2 (A1/A2) + v0.3+ (B) | `arena {}` scope blocks ship v0.2 as the default; explicit `Arena()` + `.reset()` deferred to v0.3+. |
| [`http-framework.md`](http-framework.md) | Locked | v0.3+ | Supervision-by-default HTTP server. |
| [`auto-soa.md`](auto-soa.md) | Locked (commitment) | v0.3+ | Compiler auto-transforms `array<shape>` to Struct-of-Arrays layout for hot loops accessing 1-2 fields. User code unchanged; IDE shows the transform as muted hint. |
| [`packages.md`](packages.md) | Locked design, v0.1 binary-format reservation | v0.1 + v0.2 | Binary package format reserves space for may-block metadata, ownership signatures, kernel flags from v0.1; populated in v0.2. |
| [`release-mode.md`](release-mode.md) | Locked direction | v0.4+ (TBD) | `--release` flag: LLVM `-O3`, strip debug info, disable dev-only flags (`--reveal-sensitive`, `--emit-ir`). Strips dev-only env-var checks via `cfg(release_build)`. |
| [`string-ptr-len-overhaul.md`](string-ptr-len-overhaul.md) | Locked direction, implementation deferred | TBD (likely v0.5 alongside file I/O) | Migrate strings from NUL-terminated C strings to `{ptr, len}` slices. Removes the NUL-byte footgun, makes `length` O(1). Multi-day rewrite across parser/codegen/runtime/stdlib. |
| [`doc-generator.md`](doc-generator.md) | Parking lot — direction confirmed | v0.3+ or v0.4+ | `ynz doc` command: generates structured HTML/JSON/Markdown docs from `//` leading comments + type signatures derived from the AST. No `@param` tags — types ARE the structured docs. |
| [`macos-platform-support.md`](macos-platform-support.md) | Deferred (infra) | TBD — needs a Mac | macOS dropped from CI 2026-06-01 (codegen golden tests x86_64-linux-pinned; some failures may be real macOS codegen differences unverifiable from Linux). Re-add `macos-latest` once codegen is validated + per-triple goldens recorded on a Mac. |

---

## Parking Lot — Mentioned but Not Yet Committed

These came up in conversation but were not committed to a design. They live here so we don't forget them; they're NOT promises.

- **Formal verification for NASA-grade software** (v3+ research). Yinz's type system + ownership rules give us a head start vs. C, but formal verification is a real research project. Triggered by: an actual user request from a regulated industry, or a specific safety-critical contract.
- **Option B arenas** (explicit `Arena()` + `.reset()` with lifecycle tracking) — deferred from `arena.md`. Triggered by: a real user workload where A1/A2 scope-blocks aren't expressive enough (per-connection arenas, pooled arenas across iterations).
- **Self-referential structs alternative B (fix-up on move) or C (pin-in-place)** — Yinz locks Approach A (relative pointers). B and C exist in the discussion record as rejected alternatives. Triggered by: Approach A turning out to have an unforeseen problem during v0.3 implementation.
- **GPU dispatch for ML/compute** — referenced in `design/gpu.md` as v2+. The teaching-language angle (jr devs writing GPU code via a clean abstraction) is interesting. Triggered by: ML stdlib work begins OR specific user workload.
- **Cross-crate monomorphization caching** — perf win mentioned during the design-lockdown conversation. Triggered by: v0.2 performance work; benchmark the impact on real workloads first.

---

## How to use this directory

- **Adding a v0.2/v0.3 design doc**: write the doc with the Status header, add it to "Designs Locked Here" above, add an entry to `design/decisions.md` index.
- **Adding a parking-lot idea**: write a bullet here only. No design doc until the trigger condition fires and Patrick approves implementation.
- **Promoting parking-lot to locked**: move the entry from "Parking Lot" to "Designs Locked Here", write the design doc, update `design/decisions.md`.
- **Implementing a locked design**: when the target milestone is planned, copy the design into the milestone plan's context section, then move this file to `design/` proper (out of `future/`) and update `design/decisions.md`.

---

## Cross-references

- `design/decisions.md` (the main design index that links to these)
- `design/mvp-scope.md` (the milestone sequence — what's in v0.1, v0.2, etc., including deferred features with substitutes and triggers)
- `.claude/rules/plan-invariants.md` (M4+ plans must reference relevant future docs in their `### Runtime Dependencies` section)
