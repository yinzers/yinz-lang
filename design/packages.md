# Package Manager — Design Decisions

User spec: `spec/packages.md`

---

## Built-In Package Manager

`ynz add`, `ynz remove`, `ynz update` are part of the `ynz` CLI. No separate tool.

**Why**: npm, cargo, go get — the most successful compiled language ecosystems have a first-party package manager. Third-party package managers fragment the ecosystem and create incompatibilities. One tool, one registry, one convention.

---

## Lock File Committed to Git

`yinz.lock` is auto-generated and committed. Every machine gets exact same versions.

**Why**: "Works on my machine" is almost always a version mismatch. Lock files solved this. The lock file is cheap to commit (it's just text), and the cost of NOT having it (non-reproducible builds) is high. Modeled after Cargo.lock (Rust) and package-lock.json (npm).

---

## `yinz_modules/` Git-Ignored

Local package storage directory is git-ignored. The lock file is the source of truth.

**Why**: Committing `node_modules` to git is an anti-pattern. `yinz_modules/` can always be reconstructed from the lock file with `ynz install`. Keeping it out of git keeps repos small and diffs readable.

---

## `[dev-dependencies]` Separate from `[dependencies]`

Dev dependencies (test utilities, mock servers) don't end up in production binaries.

**Why**: Tree shaking handles most of this, but `[dev-dependencies]` signals intent and allows the package manager to avoid even resolving dev deps in production builds. Consistent with every major package manager (npm, Cargo, pip).

---

## Tree Shaking Applies to Packages

Unused code from packages is stripped at compile time — same mechanism as the standard library.

**Why**: Package ecosystems accumulate large libraries with many features. If you import a math utilities package to use one function, you shouldn't ship all 200 functions. Tree shaking makes the package ecosystem safe to use without discipline about "keeping dependencies small."
