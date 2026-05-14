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

---

## Lock File Format — TOML

`yinz.lock` is TOML. Same parser, same syntax, same tooling as `yinz.toml` (the manifest). One format to learn, one format to inspect.

**Why TOML over binary:**

- **Diff-able in git.** Code review can see what changed in a PR — a single dep version bumped vs 50 sketchy new transitive deps added. Binary lock files break this entirely ("binary files differ").
- **Manually editable in emergencies.** Pin a specific version when the registry briefly serves a bad one. Patch a checksum after a republish. Switch a `source = "registry+..."` to `source = "path+./vendored/foo"` for an emergency override. All trivially possible with text; impossible with binary without a dedicated tool.
- **Same format as `yinz.toml`.** Developers learn one format. The lock file parser is the same parser as the manifest parser.
- **Every modern ecosystem chose text.** Cargo.lock (TOML), package-lock.json (JSON), poetry.lock (TOML), Gemfile.lock (custom text), pnpm-lock.yaml (YAML). Revealed-preference evidence — if binary were strictly better, someone would have done it by now. Bun's binary lockfile is the outlier and isn't where bun's install speed actually comes from.
- **Performance "win" of binary is microscopic.** Parse time of a typical lock file: ~5-20ms (TOML) vs ~0.5-2ms (binary). `ynz install` is dominated by network and disk I/O — typically seconds. The lock parse is a rounding error either way.

---

## Lock File Structure — Flat Array of Tables

The lock file is a FLAT list of all transitively-resolved packages, regardless of where each one sits in the dependency tree. The graph is reconstructed at install time by walking each package's `dependencies` array.

**Why flat instead of nested:**

- Diff-friendly — changing one dep version edits one block, leaves everything else byte-identical.
- Trivially alphabetizable for deterministic output.
- Duplicate versions of the same package are just two entries with the same `name` and different `version`.
- No nesting-depth issues for deep dep trees.

**Schema:**

```toml
# Format version of the lock file itself. Bumped when the schema changes.
version = 1

# Each [[package]] block is one resolved package, listed once regardless of depth.
[[package]]
name = "http-client"
version = "2.4.0"
source = "registry+https://yinz.pkg/v1"
checksum = "sha256:abc123..."
dependencies = [
  "tls 3.1.7",          # name + version disambiguates which entry
  "url-parser 0.8.2"
]
```

**`source` field encoding:**
- `registry+<url>` — fetched from a package registry. Normal case.
- `git+<url>@<commit-sha>` — fetched from a git repo at a specific commit.
- `path+<relative-path>` — vendored locally; no network fetch.

This is identical to Cargo.lock's encoding — the format is well-tested by Rust's ecosystem.

---

## Install Mechanism — Bun-Class Speed

The TOML format does not constrain install speed. Speed comes from the install mechanism, which is independent of lock format. Target performance for `ynz install` (v0.2 work):

- **Content-addressed global cache** at `~/.yinz/cache/`. Every package version stored once on the machine, identified by sha256 of its tarball.
- **Hard-links from cache to `yinz_modules/`**, not copies. Project's `yinz_modules/json/` points at the same inode as the cache — no file system thrashing for repeated installs across projects.
- **Parallel resolver and downloader.** Fetch all top-level deps' metadata in parallel; recurse breadth-first; downloads happen concurrently up to a configurable connection cap.
- **Lazy integrity verification.** Packages already verified in the cache don't get re-hashed every install. Only first-time downloads trigger full verification.
- **Native binary.** No Node.js startup overhead (npm's slowest 200ms).

These mechanisms together get us to bun-class install times regardless of the text-format lock file.
