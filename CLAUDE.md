# Yinz — Claude Project Rules

Yinz is a compiled systems programming language. Goal: Rust-level performance, TypeScript-level readability. Memory-safe, zero-cost abstractions, approachable by junior developers.

File extension: `.ynz`. Compiler target: LLVM native machine code.

---

## Golden Rules (check every design decision against these — lower number wins)

1. **Dot-first design.** If something can be `.method()` with autocomplete, do it that way. No cryptic symbols, no keywords to memorize.
2. **Self-documenting syntax.** Every keyword, function, and pattern should be readable by a jr dev who's never seen Yinz. If docs are required to understand a line, the design failed.
3. **No garbage collector.** Ownership-based memory management only. One memory model. The compiler handles allocation and freeing.
4. **Compiler does the hard work.** Smart defaults everywhere. Type inference inside function bodies. Developer writes simple code; compiler makes it fast.
5. **Compile-time safety.** Wrong = caught at compile time, never at runtime.
6. **Familiar syntax.** Borrow from TypeScript/JavaScript wherever possible.
7. **Step-by-step over chaining.** No method chaining. Each operation gets its own line with a named variable. Compiler fuses into a single optimized pass.
8. **Zero-cost abstractions.** High-level syntax compiles to the same machine code as hand-written low-level code.
9. **Fast to type.** Quick to write without sacrificing readability. `function` over `fn` is worth it; don't add ceremony where it adds nothing.
10. **Efficiency first, dynamic after.** Default = most performant. `fixed<T>` by default, opt into `array<T>` when you need growth.
11. **The compiler is a teacher.** Errors explain what went wrong AND why. Suggestions explain why one approach beats another. Every diagnostic answers WHAT happened, WHAT to do instead, and WHY. The WHY must be **specific and contextual** — not generic ("avoids allocation") but tied to the actual call site ("scores isn't used again after this, so sortInPlace() skips the allocation"). The compiler is a senior developer mentoring a junior developer — see `design/teaching-mission.md` for the full mission.
12. **Human-readable over programmer jargon.** `options` not `enum`. `follows` not `implements`. `nothing` not `void`. If a non-programmer can guess the meaning, the naming is right. (Note: union types use `|` for consistency with TypeScript — `or` was triple-overloaded.)
13. **Capital letter = type. Everything else = lowercase.** Scan any line of code — capital letter means type, no capital means everything else (function, variable, keyword, module). `Player` is a type. `player` is a variable. `request` is a module. `Request` would be a type. Zero ambiguity.

---

## Project Layout

| Path | Purpose |
|------|---------|
| `/spec/` | Language specification — for users of Yinz |
| `/design/` | Design decisions and open questions — for contributors |
| `design/feature-registry.md` | SSOT registry schema + carve-out policy (see `registry/features.toml`) |
| `design/lsp.md` | LSP server architecture — salsa wiring, JSON-RPC dispatch, capability negotiation, framework choice rationale, self-hosting migration plan |
| `registry/features.toml` | Single source of truth for all feature inventories (keywords, jargon, intrinsics, deferred features, hint domains) |
| `crates/ynz-registry/` | Crate that parses `registry/features.toml` + generates typed Rust constants via `build.rs` |
| `crates/ynz-lsp/` | LSP server — wraps existing salsa queries in JSON-RPC, consumes `ynz-registry` for autocomplete/hover/diagnostics |
| `crates/ynz-tmgrammar/` | TextMate grammar generator — reads `ynz-registry`, emits `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` |
| `crates/ynz-fmt/` | Formatter library — zero-config canonical Yinz formatting, consumed by `ynz fmt` subcommand and v0.2-M5 LSP format-on-save |
| `crates/ynz-watch/` | Watch daemon — long-running terminal command for rebuild-on-save + re-run; consumes salsa-backed compiler queries shared with the rest of the workspace |
| `tooling/vscode-ynz/` | VSCode extension — spawns `ynz-lsp`, ships syntax highlighting and language association |
| `.claude/rules/` | Detailed rule files (loaded on demand) |
| `CLAUDE.md` | This file — rules for Claude |

---

## Rules Files

| File | Load when |
|------|-----------|
| `.claude/rules/non-oop.md` | **LOAD FIRST** for any feature touching shapes/methods/dispatch/inheritance/contracts. Yinz is NOT object-oriented — data shapes + standalone functions + UFCS dot-call sugar. Drift back into OOP patterns is the most common modeling mistake. |
| `.claude/rules/dot-postfix.md` | Designing any syntax using dot-postfix (`value.x` vs `value.x()`). Parens for actions, no parens for access. |
| `.claude/rules/vocabulary.md` | Any docs work — authoritative reference for Yinz user-facing terms (shape, value, map, options, etc.) |
| `.claude/rules/naming.md` | Capital-letter-=-type rule, module/type case distinctions, renamed-concepts table |
| `.claude/rules/inference.md` | Designing IDE behavior, ownership UI, type-inference UI, any teaching surface where the compiler figures things out automatically |
| `.claude/rules/auto-promotion.md` | Designing any new feature, stdlib type, or compiler optimization — mandates auto-promotion analysis (silent codegen + muted hint + Tier 3 lint) when a stricter/faster form fits. The "fast by design even for inexperienced developers" pattern. |
| `.claude/rules/stdlib-design.md` | Designing or reviewing any stdlib module — six rules: pure-named methods are pure, no parallel APIs, no platform-default config, bounded queues, receiver-first args, codegen serialization. |
| `.claude/rules/feature-registry.md` | Adding any new keyword, jargon entry, primitive method, type constant, deferred feature, diagnostic template, or muted-hint domain — all go in `registry/features.toml` first |
| `.claude/rules/plan-invariants.md` | Writing or reviewing milestone plans (M4 onward must include the 7-subsection Invariants block; v0.2-M2+ plans also require `### Feature Registry Entries`) |
| `.claude/rules/spec-writing.md` | Writing or editing `/spec/` files |
| `.claude/rules/language-design.md` | Making or reviewing language design decisions |
| `.claude/rules/docs-checklist.md` | Adding new design docs, future-list ideas, or spec sections |

---

## When Working on This Project

- **Yinz is NOT object-oriented.** Data shapes hold fields + contract signatures; methods are standalone functions; `value.method()` is parser-level sugar for `method(value)` (UFCS — both call forms work). NO methods inside shape declarations; NO `override` keyword; `extends` is data-only inheritance. See `.claude/rules/non-oop.md` for the full model — this is the most common modeling mistake to drift back into. Locked r10–r13 (2026-05-16).
- **Every milestone plan MUST grow the canonical demo project + error gallery.** Per `.claude/rules/plan-invariants.md` `### Demo & Error Gallery` subsection: each phase that adds executable surface MUST extend `examples/basics/entrypoint.ynz` with the new feature in context AND extend `examples/errors/m{N}_errors.ynz` with intentional triggers for every new compile error class. This is how Patrick reviews the language UX after each phase — without it, features ship and never get hands-on validation. The basics project covers EVERY v0.1 language feature (M1–M8) in one growing demo; stdlib modules (v0.5+) get their own per-module example projects (use the SINGLE-ENTRY layout — mirror `examples/basics/`'s shape, one yinz.toml + one entrypoint.ynz + plain subfolders).
- **Project layout has two locked shapes (per `examples/README.md`)**: single-entry (`yinz.toml` with one `entry = "..."`, code in plain subfolders, used by `examples/basics/` and all stdlib module examples — the ~95% case) and multi-entry (one `yinz.toml` with `[entries]` table, ships under `ships/`, shared code in plain folders — v0.22 feature, previewed in `examples/ships_demo/`). When in doubt, single-entry. The `[entries]` multi-ship shape is opt-in for projects that genuinely have N co-shipped binaries.
- Check every proposed language feature against all 12 golden rules before suggesting it
- Always use Yinz terms — see `.claude/rules/naming.md` for the full reference
- Spec files are written for a HS grad — short sections, example-heavy, plain English
- No method chaining in code examples; step-by-step with named variables
- Design decisions go in `/design/decisions.md` with the WHY captured
- Open questions go in `/design/open-questions.md`
- When a design decision is made, move it from open questions to decisions
- Every example in spec/design/plan/rule files MUST use real Yinz operations from the current scope — no invented APIs for illustration (see `.claude/rules/dot-postfix.md` "Examples-must-use-real-operations rule")

---

## Tech Stack (fill in as compiler is built)

**Compiler target**: LLVM (planned)
**Package manager**: TBD
**Compiler implementation language**: TBD

```bash
# CLI commands (spec — not yet implemented)
ynz build entrypoint.ynz
ynz run entrypoint.ynz
```

---

## Release Workflow

Two project-local skills handle the ship cycle:

- `/pr` — opens a draft PR for the current feature branch (auto-detects milestone from Cargo.toml; scans `.claude/plans/active/` for plan reference)
- `/release` — cuts a tagged milestone release (bumps `Cargo.toml`, generates CHANGELOG section from merged PRs since last tag, commits, tags, pushes with user approval)

**When to invoke each**:

| Situation | Skill |
|---|---|
| Phase work complete on feature branch | `/pr` |
| Milestone complete (all phases merged to main) | `/release` |
| Unsure which | Just invoke either — they auto-detect and route to the other if the signals point that way |

**Proactive reminders** — Claude should suggest `/release` when:
- All tasks in the active milestone plan are checked off, AND
- All milestone PRs are merged to main, AND
- `Cargo.toml` still shows the previous milestone version (i.e., the bump hasn't happened yet)

Don't wait to be asked. If you see "ready to ship" signals, surface them.

**VSCode extension release convention** — every milestone release that touches `tooling/vscode-ynz/` MUST attach TWO `.vsix` assets to the GitHub release:
1. `yinz-{version}.vsix` — versioned artifact
2. `yinz-latest.vsix` — always overwritten (`--clobber`) so the stable URL never changes:
   `https://github.com/yinzers/yinz-lang/releases/latest/download/yinz-latest.vsix`

This stable URL is how external projects pin to the Yinz extension without updating their install script every release. Never skip the `yinz-latest.vsix` upload.

---

## Claude Code Conventions

This project uses `.claude/` for AI-assisted workflow state — plain markdown files, no special tooling required.

- **`.claude/state.md`** — project radar: environment, decisions, active workstreams. Read at session start.
- **`.claude/todos.md`** — cross-workstream backlog.
- **`.claude/plans/active/<slug>.md`** — one in-progress workstream per file. **Source of truth for that work** (not chat history, not state.md).
- **`.claude/plans/paused/`** and **`.claude/plans/done/`** — parked or completed workstreams.
- **`.claude/graveyard.md`** — known failure patterns specific to this project.

If you see an active plan file, read it before continuing the work it describes.
