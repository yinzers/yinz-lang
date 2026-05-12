---
name: release
description: Cut a tagged release of the Yinz compiler. Bumps Cargo.toml version, generates CHANGELOG section from merged PRs since last tag, commits, tags, and pushes (with user approval). Detects when the user actually meant /pr and routes accordingly.
argument-hint: "[optional-version-override e.g. 0.1.0-m2]"
---

# Cut a Release (Yinz project)

Cut a tagged release. Used when a milestone completes (M1, M2, ... M8) or a minor/major version is shipped.

---

## STEP 0 — Routing check (MANDATORY, before anything else)

The user might have invoked `/release` when they meant `/pr`. Check these signals **before** doing any work:

| Signal | Suggests |
|--------|----------|
| `git branch --show-current` != `main` | **PR** (releases are cut from main) |
| No merge commits in `git log $(git describe --tags --abbrev=0)..HEAD --merges` | **PR** (nothing has been merged since last tag — too early to release) |
| Working tree has feature-work changes (not just version/changelog) | **PR** (release changes are minimal — version bump + CHANGELOG only) |
| On `main` with clean tree and merged PRs since last tag | **Release (correct)** |

**If signals point to PR**, stop and ask the user:

> "Hold up, chief — I see [signal: e.g. 'you're on a feature branch' / 'no PRs merged since last tag']. Did you mean `/pr`? Say `pr` to switch, or `release` to continue as a release anyway."

Only proceed past Step 0 once the user confirms or the signals are clean.

---

## Yinz release conventions

- Tags: `v{MAJOR}.{MINOR}.{PATCH}-m{N}` for pre-1.0 milestones (e.g. `v0.1.0-m2`). For v0.1 final, drop the `-m{N}` suffix (`v0.1.0`).
- Version in `Cargo.toml` (`[workspace.package].version`) = `{MAJOR}.{MINOR}.{PATCH}-m{N}` (no `v` prefix in TOML, `v` prefix on git tags only)
- CHANGELOG section template lives in `CHANGELOG.md` — match the existing milestone-section format
- One release commit + one tag, both on main

---

## STEP 1 — Pre-flight

```bash
git branch --show-current           # MUST be main
git status                          # MUST be clean (no uncommitted changes outside this skill's work)
git fetch --tags
git describe --tags --abbrev=0      # last shipped tag, e.g. v0.1.0-m1
grep '^version' Cargo.toml          # current Cargo version, should match last tag
```

If main is behind origin/main → ask user to pull first. Never `git pull` autonomously.

---

## STEP 2 — Determine target version

1. If `$ARGUMENTS` is provided (e.g. `0.1.0-m2`), use that
2. Else, bump from current Cargo.toml version:
   - `0.1.0-m{N}` → `0.1.0-m{N+1}` (milestone bump)
   - For a non-milestone bump (final v0.1 release, jumping minor, etc.) — **ASK** the user
3. Confirm the target with the user before proceeding:

> "Cutting release `v{X.Y.Z}-m{N}`. Last shipped was `v{prev}`. Sound right?"

---

## STEP 3 — Gather merged PRs since last tag

```bash
git log $(git describe --tags --abbrev=0)..HEAD --merges --pretty=format:'%H %s'
```

For each merge commit, pull the PR title and number via `gh pr view {N} --json title,number,body`.

If no merges exist between last tag and HEAD, **stop and tell the user** — there's nothing to release.

---

## STEP 4 — Generate CHANGELOG section

Match the existing `CHANGELOG.md` format. The template:

```markdown
## v{X.Y.Z}-m{N} — {milestone name} (M{N} milestone)

**Release tag:** `v{X.Y.Z}-m{N}`

### What ships

{1-2 paragraph summary — what this milestone delivers, what the user can now do}

### Language surface (M{N})

{bullet list of new language features — only if applicable}

### Compiler features

{bullet list grouped by crate, derived from merged PRs:}
- **{Crate}** (`ynz-{crate}`): {what changed}

### Tests

{N tests across M crates, all passing. Note any new test types: snapshot, golden, integration, etc.}
```

**Generate a DRAFT**, then show it to the user for review. They edit before commit. Do NOT auto-commit the CHANGELOG.

Suggested process:
1. Group merged PRs by crate (parse `crates/X` from PR titles or PR descriptions)
2. Pull the "What" / "Why" lines from each PR body
3. Write a coherent narrative, not a flat bullet dump — milestones are stories
4. Run a `grep` of the draft against the banned-jargon list in `design/compiler-errors.md` (propagate, narrow, infer, polymorphic, etc.) — these belong in `design/`, never in user-facing release notes

---

## STEP 5 — Apply changes

After user approves the CHANGELOG draft:

1. Update `Cargo.toml` — bump `[workspace.package].version` to `{X.Y.Z}-m{N}`
2. Update `CHANGELOG.md` — prepend the new section (newest at top, after the `# Changelog` header)
3. Run `cargo build --workspace` to refresh `Cargo.lock` with the new version
4. `cargo test --workspace` — confirm green locally before tagging
5. If tests fail → STOP, surface the failure, do not tag

---

## STEP 6 — Commit, tag, push (with explicit user OK)

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "release: v{X.Y.Z}-m{N}

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
git tag -a v{X.Y.Z}-m{N} -m "Release v{X.Y.Z}-m{N} — {milestone name}"
```

Then **stop and ask the user**:

> "Release commit and tag created locally. Push to origin? (`git push origin main && git push origin v{X.Y.Z}-m{N}`)"

Only push after explicit go-ahead. Never push autonomously.

---

## STEP 7 — Post-release

After successful push:
1. Print the tag URL: `https://github.com/{org}/{repo}/releases/tag/v{X.Y.Z}-m{N}`
2. Suggest the user create a GitHub release from the tag (manual, via UI or `gh release create`) if they want release notes visible on GitHub
3. Update `.claude/state.md` — add a one-line decision entry: `[YYYY-MM-DD] **M{N} compiler released as v{X.Y.Z}-m{N}**: {summary}`

---

## What this skill does NOT do

- Does not auto-pull from origin (user does this)
- Does not auto-merge PRs into main (PRs merge via the GitHub PR flow, not release)
- Does not push without explicit user approval
- Does not create a GitHub release page automatically — just the git tag
- Does not edit `design/` or `spec/` files — those track design state, separate from release artifacts
