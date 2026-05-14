# Packages

Add, remove, and update dependencies with `ynz` commands.

---

## Commands

```
ynz add http-server                  // install a package
ynz add graphics@1.2.0               // install a specific version
ynz remove http-server               // uninstall
ynz update                           // update all to latest compatible
ynz update http-server               // update one package
ynz publish                          // publish your package to the registry
```

---

## Dependencies in yinz.toml

```toml
[dependencies]
http-server = "1.2.0"
graphics = "0.9.0"

[dev-dependencies]
test-utils = "0.5.0"
```

`[dependencies]` — included in all builds.

`[dev-dependencies]` — only available when running tests or building in development mode. Not included in production binaries.

---

## Lock file — deterministic builds

`yinz.lock` is auto-generated and committed to git. It records the exact version of every package (and every dependency's dependencies) so every machine and every CI run gets identical code.

The lock file is TOML — same format as `yinz.toml` — so it's readable, diffable in git, and (in emergencies) editable by hand. `ynz add` and `ynz update` normally maintain it for you.

Here's what a small one looks like:

```toml
# yinz.lock — auto-generated, do not edit unless you know what you're doing.

# Format version of the lock file itself.
version = 1

# Each [[package]] is one resolved package. The list is flat regardless of
# where each package sits in the dependency tree — the graph is rebuilt
# from each package's "dependencies" list when ynz install runs.

[[package]]
name = "http-client"
version = "2.4.0"
source = "registry+https://yinz.pkg/v1"
checksum = "sha256:abc123def456..."
dependencies = [
  "tls 3.1.7",
  "url-parser 0.8.2"
]

[[package]]
name = "json"
version = "1.0.3"
source = "registry+https://yinz.pkg/v1"
checksum = "sha256:7890abcdef..."
dependencies = []

[[package]]
name = "tls"
version = "3.1.7"
source = "registry+https://yinz.pkg/v1"
checksum = "sha256:fedcba9876..."
dependencies = []

[[package]]
name = "url-parser"
version = "0.8.2"
source = "registry+https://yinz.pkg/v1"
checksum = "sha256:13579bdf..."
dependencies = []
```

**Reading it:**

- Each `[[package]]` is one entry. The list is flat, alphabetized for stable diffs.
- `dependencies` lists what THIS package needs, by `"name version"` so the resolver can disambiguate when multiple versions of the same package coexist.
- `source` says where to fetch the package from — `registry+url`, `git+url@commit`, or `path+./vendored/foo`.
- `checksum` is the hash of the downloaded tarball. On install, the hash is re-verified — catches registry tampering.

**Diamond dependencies (same package, two versions):**

If `http-client` needs `json 1.0.3` and `markdown-parser` needs `json 2.0.0`, both versions show up in the lock file. The `name version` reference disambiguates which one each parent uses. The compiler keeps them separately linked so they don't interfere.

---

## Package directory

`yinz_modules/` stores installed packages locally. Add it to `.gitignore` — the lock file is enough to reproduce it exactly on any machine.

---

## Tree shaking applies to packages

Install a large utility package, use one function — only that function ends up in your compiled binary. The same tree shaking that strips unused standard library code works identically for packages.
