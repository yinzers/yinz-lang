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

**Windows note**: hard-links work without administrator elevation on Windows (unlike symlinks). The install mechanism never uses symlinks — hard-links throughout. Before shipping Windows support, verify that the `yinz_modules/` layout does not produce paths exceeding 260 characters (Windows legacy MAX_PATH limit) for deeply nested packages. The flat cache + hard-link model shouldn't create extra nesting, but this must be tested explicitly.

---

## Registry Policy — Immutability and No Unpublish

Published packages are **immutable and permanent**. Once a version is published to the Yinz registry, it cannot be deleted or modified.

- **No unpublish.** There is no `ynz unpublish` command. A published version stays accessible forever. This is the lesson from the npm left-pad incident (March 2016): 11 lines of code that 2.5 million downloads/month depended on, deleted by its author, broke builds at Facebook, Netflix, PayPal, and Spotify within hours. The "author owns existence" model is incompatible with a reliable ecosystem.
- **Archive instead of delete.** Package authors can mark a package as **archived** — indicating it is no longer maintained. Archived packages remain fully installable; the archive flag is a maintenance-status signal, not a removal. `ynz add` will warn when adding an archived package as a new dependency.
- **Content-addressed storage is the enforcement mechanism.** The lock file records `checksum = "sha256:..."` per package version. The registry stores packages by content hash. There is no registry-side mutation path that could silently serve different bytes under the same version string.
- **Semver ranges still carry risk.** The lock file + checksums guarantee bit-for-bit reproducibility for committed lock files. For fresh installs against version ranges (before the lock is committed), the registry's immutability means at least the resolved version is always the same bytes. Behavioral changes in new minor/patch versions remain the author's responsibility — the registry cannot enforce semver compliance programmatically.

Full registry design (auth, publishing, search, namespacing) deferred to `design/registry.md` when v0.2+ registry work starts.

---

## No Install-Time Code Execution

Yinz packages **cannot declare code to run at install time**. No `postinstall` scripts, no `build.rs` equivalent, no lifecycle hooks in `yinz.toml`.

`ynz install` fetches tarballs, verifies checksums, and hard-links files. It never executes package-provided code. This is not configurable.

**Why**: npm's `postinstall` and Cargo's `build.rs` both allow arbitrary code execution during installation. Both have been used as supply-chain attack vectors — running as the installing user with full filesystem access. The March 2026 Axios npm attack used a malicious dependency's postinstall script to deploy a cross-platform RAT. The pattern cannot be made safe while remaining permissive: pnpm v10 disabled postinstall by default in 2024; npm has not followed suit as of 2025 because too much of the ecosystem depends on it.

Yinz starts from zero. There is no `postinstall` to be backward-compatible with.

**FFI compilation (v2+)**: when FFI ships, native library compilation will be addressed via a sandboxed, opt-in mechanism that the consumer explicitly enables — not a publisher-declared hook that runs silently on install. The design for that mechanism lives in `design/ffi.md` when FFI is designed.
