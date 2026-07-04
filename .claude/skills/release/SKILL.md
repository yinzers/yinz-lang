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
- Version in [`Cargo.toml`](../../../Cargo.toml) (`[workspace.package].version`) = `{MAJOR}.{MINOR}.{PATCH}-m{N}` (no `v` prefix in TOML, `v` prefix on git tags only)
- CHANGELOG section template lives in [`CHANGELOG.md`](../../../CHANGELOG.md) — match the existing milestone-section format
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

Match the existing [`CHANGELOG.md`](../../../CHANGELOG.md) format. The template:

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
4. Run a `grep` of the draft against the banned-jargon list in [`docs/reference/REF-compiler-errors.md`](../../../docs/reference/REF-compiler-errors.md) (propagate, narrow, infer, polymorphic, etc.) — these belong in `design/`, never in user-facing release notes

---

## STEP 5 — Apply changes

After user approves the CHANGELOG draft:

1. Update [`Cargo.toml`](../../../Cargo.toml) — bump `[workspace.package].version` to `{X.Y.Z}-m{N}`
2. Update [`CHANGELOG.md`](../../../CHANGELOG.md) — prepend the new section (newest at top, after the `# Changelog` header)
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

## STEP 7 — Publish the VS Code extension (ONLY if the release touched `tooling/vscode-ynz/`)

Runs after the tag is pushed (STEP 6). It creates a GitHub release with attached `.vsix` assets and publishes to both the VS Code Marketplace and the Open VSX Registry (the registry Cursor, VSCodium, Gitpod, and other VS Code forks pull from) — but **only when the extension actually changed this release.**

### 7a — Detection gate

Compare the **previous** shipped tag (the one you recorded in STEP 1, before this release's tag — call it `v{prev}`) against HEAD:

```bash
git diff --name-only v{prev}..HEAD -- tooling/vscode-ynz/
# (if you didn't record v{prev}, HEAD~1 works here — the new release tag is on HEAD,
#  so `git describe --tags --abbrev=0 HEAD~1` returns the previous tag)
```

- **Empty output** → the extension did not change. **Skip the entire rest of STEP 7** and go to STEP 8.
- **Non-empty** → the extension changed; continue with 7b–7e. The two-`.vsix` attachment to the GitHub release is the [`CLAUDE.md`](../../../CLAUDE.md) "VSCode extension release convention" — mandatory when this gate is non-empty. The Marketplace publish (7d) and Open VSX publish (7e) are this skill's own release policy, layered on top, riding the same gate.

**Tooling note:** `node` / `npm` / `npx` are not host-native here — probe `command -v npx` first. If absent, run the *build* (7b) through the dev container per [`CLAUDE.md`](../../../CLAUDE.md) (`docker compose run --rm dev bash -c "cd tooling/vscode-ynz && npm install && npm run build && npx vsce package --no-yarn"`). The *publishes* (7d Marketplace, 7e Open VSX) must each run in the same environment that holds their respective credential — the vsce Marketplace PAT from `npx vsce login yinz-lang` for 7d, the Open VSX access token for 7e — and the skill manages neither.

### 7b — Build the `.vsix`

```bash
cd tooling/vscode-ynz
npx vsce package --no-yarn      # matches the `package` script in package.json
```

`vsce` names the file `yinz-<version>.vsix` from the **extension's own** [`package.json`](../../../tooling/vscode-ynz/package.json) `version` field (independent of the Cargo release version). Before packaging, confirm that version reflects this release — if it's stale for the milestone you're shipping, **ask the user** whether to bump it, then re-package.

### 7c — GitHub release + attach BOTH `.vsix` assets

The `yinz-latest.vsix` asset is a copy of the versioned one — it keeps the stable download URL (`https://github.com/yinzers/yinz-lang/releases/latest/download/yinz-latest.vsix`) pointing at the newest release, per [`CLAUDE.md`](../../../CLAUDE.md).

```bash
# from repo root
cp tooling/vscode-ynz/yinz-<version>.vsix tooling/vscode-ynz/yinz-latest.vsix

# create the release from the pushed tag (skip create if it already exists)
gh release create v{X.Y.Z}-m{N} \
  --title "v{X.Y.Z}-m{N} — {milestone name}" \
  --notes "<the CHANGELOG section for this release>"

# attach both assets; --clobber lets a re-run overwrite (esp. yinz-latest.vsix)
gh release upload v{X.Y.Z}-m{N} \
  tooling/vscode-ynz/yinz-<version>.vsix \
  tooling/vscode-ynz/yinz-latest.vsix \
  --clobber
```

Never skip the `yinz-latest.vsix` upload — external install scripts pin to that stable URL.

### 7d — Publish to the VS Code Marketplace (explicit stop-and-confirm gate)

This pushes the extension live to every VS Code user who installs or updates it. It is a public-registry action — it gets the **same explicit approval** STEP 6 requires for `git push`. Do NOT auto-run it just because the vsce credential is cached.

**Stop and ask the user:**

> "About to publish `v{X.Y.Z}-m{N}` (extension version `<version>`) to the VS Code Marketplace under publisher `yinz-lang`. This goes live to everyone who installs or updates the Yinz extension. Proceed?"

Only after an explicit go-ahead:

```bash
cd tooling/vscode-ynz
npx vsce publish
```

**Failure handling — surface, do not swallow, do not roll back.** If `npx vsce publish` fails (auth expired, network, version already published, etc.), STOP and show the user the actual error verbatim. The git-side release is **independent and already succeeded** — do not undo the commit, the tag, the push, or the GitHub release. Tell the user exactly where things stand:

> "Marketplace publish FAILED — [paste the real error]. The git-side release is live and intact (commit, tag `v{X.Y.Z}-m{N}`, GitHub release, and both `.vsix` assets). Only the Marketplace step failed. Retry with `npx vsce login yinz-lang` (if auth) then `npx vsce publish`. I'll still attempt the Open VSX publish (7e) — the two registries are independent."

### 7e — Publish to the Open VSX Registry (explicit stop-and-confirm gate)

Open VSX is a **separate, independent registry** from the VS Code Marketplace — it's where Cursor, VSCodium, Gitpod, and other VS Code forks pull extensions from. Publishing here is a distinct public-registry action that goes live to those users, so it gets the **same explicit approval** as 7d and STEP 6. It publishes the **exact same `.vsix` already built in 7b** — no rebuild, no re-package.

**One-time setup (NOT a per-release step — do this once, ever):** before 7e can run at all, the `yinz-lang` namespace must be claimed on [open-vsx.org](https://open-vsx.org) and an Open VSX access token generated from the account settings. Claiming is a manual, GitHub-login-based step the skill cannot automate — either via the web UI or `npx ovsx create-namespace yinz-lang -p <token>`. If a first-ever 7e publish fails with a namespace/access error, this setup hasn't been done — surface that to the user; do not attempt to auto-claim.

The token is a **separate credential** from the vsce/Azure PAT used in 7d — a distinct Open VSX access token. Like 7d, the skill does not manage or store it; it expects the token to already be available in the environment 7e runs from.

**Stop and ask the user:**

> "About to publish `v{X.Y.Z}-m{N}` (extension version `<version>`) to the Open VSX Registry under namespace `yinz-lang`. This goes live to Cursor / VSCodium / Gitpod users who install or update the Yinz extension. This is a separate registry from the VS Code Marketplace (7d). Proceed?"

Only after an explicit go-ahead:

```bash
# reuse the SAME .vsix built in 7b — point ovsx at it directly, no rebuild
npx ovsx publish tooling/vscode-ynz/yinz-<version>.vsix -p <open-vsx-token>
```

**Sequencing with 7d:** the two registries are fully independent — a 7d failure does **not** block 7e (attempt 7e anyway) and a 7e failure does **not** roll back 7d. If the user's go-ahead on the 7e prompt says otherwise (e.g. "skip Open VSX this time"), honor that.

**Failure handling — surface, do not swallow, do not roll back.** If `npx ovsx publish` fails (auth expired, network, version already published, namespace not claimed, etc.), STOP and show the user the actual error verbatim. The git-side release (STEP 6) and the Marketplace publish (7d) are **independent and unaffected** — do not undo the commit, tag, push, GitHub release, or the Marketplace publish. Tell the user exactly which of the three independent outcomes succeeded vs failed:

> "Open VSX publish FAILED — [paste the real error]. Status of the three independent release targets: git-side release (commit, tag `v{X.Y.Z}-m{N}`, GitHub release, `.vsix` assets) = [live/failed]; VS Code Marketplace (7d) = [live/failed/skipped]; Open VSX (7e) = FAILED. Only the Open VSX step needs a retry — `npx ovsx publish tooling/vscode-ynz/yinz-<version>.vsix -p <token>` (confirm the namespace is claimed and the token is valid)."

---

## STEP 8 — Post-release

After successful push (and STEP 7 if it ran):
1. Print the tag URL: `https://github.com/{org}/{repo}/releases/tag/v{X.Y.Z}-m{N}`
2. GitHub release: if STEP 7 ran, the release page already exists (with the `.vsix` assets). If STEP 7 was skipped (extension unchanged), suggest the user create a GitHub release from the tag manually (`gh release create`) if they want release notes visible on GitHub.
3. Update [`.claude/state.md`](../../state.md) — add a one-line decision entry: `[YYYY-MM-DD] **M{N} compiler released as v{X.Y.Z}-m{N}**: {summary}` (if STEP 7 ran, note the extension version and which of the two registries — Marketplace (7d) and Open VSX (7e) — published successfully).

---

## What this skill does NOT do

- Does not auto-pull from origin (user does this)
- Does not auto-merge PRs into main (PRs merge via the GitHub PR flow, not release)
- Does not push, publish to the Marketplace, publish to Open VSX, or take any other irreversible action without explicit user approval
- Does not create a GitHub release page or publish to either extension registry when the release does **not** touch `tooling/vscode-ynz/` — those releases stop at the git tag (STEP 7 is skipped entirely). When the extension **does** change, STEP 7 creates the GitHub release (with both `.vsix` assets) and publishes to both the VS Code Marketplace (7d) and the Open VSX Registry (7e).
- Does not manage or store the vsce Marketplace PAT — that lives in vsce's own credential store from `npx vsce login yinz-lang`; the skill only invokes `npx vsce publish` and stops on any failure
- Does not manage or store the Open VSX access token — that's a separate credential the skill expects to already be in the environment; the skill only invokes `npx ovsx publish` and stops on any failure
- Does not claim the `yinz-lang` Open VSX namespace — that's a one-time, manual, GitHub-login-based setup (open-vsx.org web UI or `npx ovsx create-namespace yinz-lang`), not a per-release action
- Treats the git-side release (STEP 6), the Marketplace publish (7d), and the Open VSX publish (7e) as three independent outcomes — a failure in one is reported without blocking or rolling back the others
- Does not edit [`docs/internal/implementation/`](../../../docs/internal/implementation/) or [`docs/reference/`](../../../docs/reference/) files — those track design state, separate from release artifacts
