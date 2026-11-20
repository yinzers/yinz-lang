---
name: "REF-tooling"
description: "The ynz CLI builds, runs, formats and watches your code."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Tooling

The `ynz` CLI builds, runs, formats and watches your code. Every command takes a single `.ynz` file or a project folder. A project folder needs a `yinz.toml`.

---

## Building

```
ynz build entrypoint.ynz    // compile one file into a native program
ynz build .                 // compile the project in this folder
```

The compiled program lands next to the source file (one file) or in the project folder (project). When the build works, `ynz build` prints `Build succeeded:` and the path of the program.

| Flag | What it does |
|------|--------------|
| `--no-optimize` | Skips the optimizer for a faster build. The program behaves the same, but runs slower. |
| `--emit-ir` | Also writes the generated LLVM code next to the program, as `<program>.ll`. |
| `--json` | Checks your code and prints each compiler message as one JSON object per line, for editors and other tools. It does not produce a program. |

`ynz build` exits with `0` on success, `1` when your code has an error, and `2` when something outside your code fails, such as a missing linker.

---

## Running

```
ynz run entrypoint.ynz    // compile, run, then delete the program
ynz run .                 // the same for a project
```

| Flag | What it does |
|------|--------------|
| `--keep` | Keeps the compiled program after it exits. |
| `--emit-ir` | Writes the generated LLVM code, as in `ynz build`. |
| `--reveal-sensitive` | Prints `sensitive` values in plain text instead of `[REDACTED]`. See [Sensitive Values](REF-sensitive.md). |

`ynz run` returns your program's exit code to your shell.

---

## Watch mode

```
ynz watch .             // rebuild and re-run when you save a file
ynz watch --check .     // rebuild only; do not run the program
```

When you save a file, the compiler rebuilds the project and runs the new program.

| Flag | What it does |
|------|--------------|
| `--check` | Builds but does not run the program. Exits with `1` if the first build fails. |
| `--json` | Prints watch events as one JSON object per line. |
| `--no-clear` | Does not clear the terminal between rebuilds. |

Stop watching with Ctrl+C.

---

## Formatting

```
ynz fmt entrypoint.ynz    // rewrite one file in the standard style
ynz fmt --all .           // format every .ynz file in the project
ynz fmt --check entrypoint.ynz    // change nothing; exit with 1 if the file would change
ynz fmt --all --check .   // the same check for every .ynz file in the project
ynz fmt --stdin           // read code on standard input, print the formatted code
```
