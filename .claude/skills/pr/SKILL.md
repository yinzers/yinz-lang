---
name: pr
description: Create a GitHub draft PR for the Yinz compiler project. Auto-detects milestone from Cargo.toml and active plan file. Detects when the user actually meant /release and routes accordingly.
argument-hint: "[optional-title-override]"
---

# Create Draft PR (Yinz project)

Create a draft pull request following Yinz conventions. This **overrides** the global `/pr` skill — Yinz does not use Notion tickets, VPM phase tables, or bun test plans.

---

## STEP 0 — Routing check (MANDATORY, before anything else)

The user might have invoked `/pr` when they meant `/release` (or vice versa). Check these signals **before** doing any work:

| Signal | Suggests |
|--------|----------|
| `git branch --show-current` == `main` | **Release** (PRs never originate on main) |
| `git diff HEAD -- Cargo.toml` shows `version = "..."` changed | **Release** (version bumps belong in release flow, not feature PRs) |
| Staged/uncommitted CHANGELOG.md additions for a new version section | **Release** |
| On a `feat/`, `fix/`, `refactor/`, etc. branch with normal feature work | **PR (correct)** |
| `git log main..HEAD --merges` shows merge commits | **Release** (you're on main with merged PRs to tag) |

**If signals point to release**, stop and ask the user:

> "Quick sanity check, chief — I see [signal: e.g. 'you're on main' / 'Cargo.toml version changed']. Did you mean `/release`? Say `release` to switch, or `pr` to continue as a PR anyway."

Only proceed past Step 0 once the user confirms or the signals are clean.

---

## Yinz PR conventions

- PRs are **always drafts** against **main**
- Title format: plain `{branch-name}` or a human-written override (no `[main]` prefix — single-branch repo)
- Branch prefixes used: `feat/`, `fix/`, `refactor/`, `test/`, `chore/`, `ci/`, `doc/`, `design/`
- No Notion tickets — milestone tracking via plan slug + Cargo.toml version
- Test plan = Rust toolchain (`cargo test`, `cargo clippy`, `cargo fmt --check`) — same checks CI runs

---

## STEP 1 — Pre-flight

1. `git status` — warn on uncommitted changes
2. `git branch --show-current` — must NOT be `main`
3. `grep '^version' /workspaces/ynz/Cargo.toml` — read shipped version (e.g. `0.1.0-m1`)
4. `find /workspaces/ynz/.claude/planning/active -maxdepth 2 -name plan.md 2>/dev/null` — find the active plan file(s) (see Step 2). (Migrated 2026-07-01 from the old flat `.claude/plans/active/*.md` layout to `.claude/planning/active/<plan-id>/plan.md`.)
5. If there are uncommitted changes, commit them (Conventional Commits style) — never push without user approval

---

## STEP 2 — Derive milestone context

The PR is work toward the **next** milestone after the one currently in Cargo.toml.

Algorithm:
1. Current Cargo.toml version = last shipped (e.g. `0.1.0-m1` means M1 shipped)
2. Active milestone in flight = M{N+1} (e.g. M2)
3. v0.X comes from Cargo.toml major.minor (e.g. `0.1.0-m1` → `v0.1`)

**Plan file reference** (for the PR body):
- One file in `.claude/planning/active/*/plan.md` → use it
- Multiple files → ask user which one this PR relates to (or "none")
- Zero files → omit the Plan line from the PR body

---

## STEP 3 — Create the PR

Push the branch if not already on remote (after user OK), then:

```bash
gh pr create --draft --base main --title "{title}" --body "$(cat <<'EOF'
## What

{one-sentence summary}

## Why

{motivation — what design decision, milestone task, or bug this addresses}

📋 **Plan**: `.claude/planning/active/{plan-id}/plan.md` (omit line if no active plan)
🎯 **Milestone**: v{X.Y} M{N} — {milestone name if known, else omit}

---

## Changes

| Area | Change |
|---|---|
| `crates/{crate}` | {what changed} |

{group git diff main...HEAD --stat by crate}

## Test Plan

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt --check` passes
- [ ] {any milestone-specific verification, e.g. running a new spec example}

## Risk Notes

> [!NOTE]
> {anything worth flagging — or remove this section if nothing}

<details>
<summary>🔍 <strong>Implementation details</strong> (click to expand)</summary>

{deeper context — algorithm notes, design tradeoffs, alternatives considered. Skip if not needed.}

</details>
EOF
)"
```

Return the PR URL to the user.

---

## What this skill does NOT do

- Does not run tests locally — CI runs them; PR is draft so reviewer sees CI results
- Does not push without explicit user approval
- Does not auto-merge or mark ready-for-review
- Does not bump versions — that's `/release`'s job
- Does not write CHANGELOG entries — that's `/release`'s job
