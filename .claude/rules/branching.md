# Branching

Where work lands in this repo. Decided by Patrick 2026-09-03, in response to `/execute-plan`'s
pre-flight gate finding no written rule.

- **`main` is protected.** Never commit directly to it. Never force-push it.
- **Plan and feature work lives on its own branch**, named for what it is: `feat/<slug>` for a
  milestone or feature, `fix/<slug>` for a hotfix, `perf/<slug>` for a measured performance pass.
  A `/execute-plan` run's branch SHOULD carry the plan's own slug (e.g. plan-id
  `2026-07-04-v0-3-m8-concurrency-completion` → `feat/v0-3-m8-concurrency-completion`) so a cold
  resume can find the ref from the plan alone.
- **A branch reaches `main` through a pull request**, via the project's `/pr` skill — never a
  direct merge, never a fast-forward push. Opening the PR is a shared, visible act and rides the
  same confirmation gate every commit here does.
- **One live checkout per branch.** When a second session needs the same branch, it adds a git
  worktree rather than moving the existing checkout under the first session's feet.
