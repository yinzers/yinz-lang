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
13. **Capital letter = type. Everything else = lowercase.** Scan any line of code — capital letter means type, no capital means everything else (function, variable, keyword, module). `Player` is a type. `player` is a variable. `http` is a module. `Http` would be a type. Zero ambiguity.

---

## Project Layout

| Path | Purpose |
|------|---------|
| `/spec/` | Language specification — for users of Yinz |
| `/design/` | Design decisions and open questions — for contributors |
| `.claude/rules/` | Detailed rule files (loaded on demand) |
| `CLAUDE.md` | This file — rules for Claude |

---

## Rules Files

| File | Load when |
|------|-----------|
| `.claude/rules/vocabulary.md` | Any docs work — authoritative reference for Yinz user-facing terms (shape, value, map, options, etc.) |
| `.claude/rules/naming.md` | Capital-letter-=-type rule, module/type case distinctions, renamed-concepts table |
| `.claude/rules/inference.md` | Designing IDE behavior, ownership UI, type-inference UI, any teaching surface where the compiler figures things out automatically |
| `.claude/rules/auto-promotion.md` | Designing any new feature, stdlib type, or compiler optimization — mandates auto-promotion analysis (silent codegen + muted hint + Tier 3 lint) when a stricter/faster form fits. The "fast by design even for inexperienced developers" pattern. |
| `.claude/rules/stdlib-design.md` | Designing or reviewing any stdlib module — six rules: pure-named methods are pure, no parallel APIs, no platform-default config, bounded queues, receiver-first args, codegen serialization. |
| `.claude/rules/plan-invariants.md` | Writing or reviewing milestone plans (M4 onward must include the 5-subsection Invariants block) |
| `.claude/rules/spec-writing.md` | Writing or editing `/spec/` files |
| `.claude/rules/language-design.md` | Making or reviewing language design decisions |
| `.claude/rules/docs-checklist.md` | Adding new design docs, future-list ideas, or spec sections |

---

## When Working on This Project

- Check every proposed language feature against all 12 golden rules before suggesting it
- Always use Yinz terms — see `.claude/rules/naming.md` for the full reference
- Spec files are written for a HS grad — short sections, example-heavy, plain English
- No method chaining in code examples; step-by-step with named variables
- Design decisions go in `/design/decisions.md` with the WHY captured
- Open questions go in `/design/open-questions.md`
- When a design decision is made, move it from open questions to decisions

---

## Tech Stack (fill in as compiler is built)

**Compiler target**: LLVM (planned)
**Package manager**: TBD
**Compiler implementation language**: TBD

```bash
# CLI commands (spec — not yet implemented)
ynz build main.ynz
ynz run main.ynz
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


---

## Claude Code Conventions

This project uses `.claude/` for AI-assisted workflow state — plain markdown files, no special tooling required.

- **`.claude/state.md`** — project radar: environment, decisions, active workstreams. Read at session start.
- **`.claude/todos.md`** — cross-workstream backlog.
- **`.claude/plans/active/<slug>.md`** — one in-progress workstream per file. **Source of truth for that work** (not chat history, not state.md).
- **`.claude/plans/paused/`** and **`.claude/plans/done/`** — parked or completed workstreams.
- **`.claude/graveyard.md`** — known failure patterns specific to this project.

If you see an active plan file, read it before continuing the work it describes.
