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

You never edit the lock file manually. `ynz add` and `ynz update` maintain it for you.

---

## Package directory

`yinz_modules/` stores installed packages locally. Add it to `.gitignore` — the lock file is enough to reproduce it exactly on any machine.

---

## Tree shaking applies to packages

Install a large utility package, use one function — only that function ends up in your compiled binary. The same tree shaking that strips unused standard library code works identically for packages.
