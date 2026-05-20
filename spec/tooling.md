# Tooling

The `ynz` CLI builds, runs, and watches your code.

---

## Building

```
ynz build            // compile your project — debug mode (fast compile)
ynz build --release  // compile with full optimization (slower compile, fastest runtime)
```

**Debug mode** (`ynz build`) compiles quickly. Use it during development. The binary runs at a decent speed but isn't fully optimized.

**Release mode** (`ynz build --release`) turns on full LLVM optimization. Compiles slower. The output binary is as fast as possible — this is what you deploy to production.

Think of it as: debug = fast to build, release = fast to run.

---

## Running

```
ynz run              // compile in debug mode and run immediately
ynz run --release    // compile with full optimization and run
```

---

## Watch Mode

```
ynz watch            // watch for file changes, recompile automatically
ynz watch --run      // watch, recompile, and restart the program on changes
```

`ynz watch --run` is the typical development loop for servers and long-running programs:

1. You save a file
2. The compiler detects the change
3. Only the changed file and anything that depends on it is recompiled
4. The program restarts with the new binary

The goal is sub-second recompile for typical single-file changes. You save, blink, it's running.

For web servers:

```
ynz watch --run
// Save users.ynz → recompile → server restarts → ready
```

---

## Testing

```
ynz test                    // run all tests
ynz test players            // run tests matching "players"
ynz test --watch            // rerun on file change
```

See [Testing](testing.md) for the full test spec.

---

## Package management

```
ynz add markdown-parser          // install
ynz add graphics@1.2.0       // specific version
ynz remove markdown-parser       // uninstall
ynz update                   // update all
ynz publish                  // publish to registry
```

See [Packages](packages.md) for the full package spec.

---

## What gets cached

The compiler caches:
- Type analysis results (only reruns for changed files)
- Ownership proofs (only reruns for changed ownership paths)
- Compiled output for unchanged files

A clean build (`ynz build --clean`) clears all caches and compiles from scratch. Use this if something seems wrong with incremental state.

```
ynz build --clean            // clear caches, full rebuild in debug mode
ynz build --clean --release  // clear caches, full rebuild in release mode
```
