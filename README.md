# Yinz

A compiled systems programming language. Rust-level performance, TypeScript-level readability. Memory-safe, zero-cost abstractions, approachable by developers at any experience level.

File extension: `.ynz`. Compiler target: LLVM native machine code.

For the language specification, see [`spec/overview.md`](spec/overview.md).

---

## Mental model — two compile steps

There are two separate compilation steps in Yinz:

1. **Cargo compiles the compiler.** The Yinz compiler is written in Rust. `cargo build --release` reads the Rust source under `crates/` and produces a native binary at `target/release/ynz`. **Run this whenever you change the Rust compiler source** (i.e., whenever you're working on a new milestone — M3 compiler changes, M4 type-system, etc.).

2. **The `ynz` binary compiles Yinz programs.** Once `target/release/ynz` exists, running `ynz run hello.ynz` reads your Yinz source, type-checks it, generates LLVM IR, links it into a native binary, and runs it. **Run this every time you change a `.ynz` source file.** No need to re-run `cargo build` for this — only when the compiler itself changes.

```
[Rust compiler source]  --cargo build-->  [ynz binary]  --ynz run-->  [native binary] -> runs
                                            ^^^^^^^^^^
                                            this is the
                                            Yinz compiler
```

---

## Building the compiler

### Prerequisites

**LLVM 18** must be installed before building.

**Linux (Debian/Ubuntu):**
```
sudo apt-get install llvm-18 llvm-18-dev libclang-18-dev clang-18
```

**macOS (Homebrew):**
```
brew install llvm@18
export LLVM_SYS_181_PREFIX=$(brew --prefix llvm@18)
```

Add the `LLVM_SYS_181_PREFIX` export to your shell profile on macOS. On Linux it is set automatically via `.cargo/config.toml`.

**Rust stable toolchain:** Install via [rustup](https://rustup.rs).

### Build (do this once per compiler change)

```
cargo build --release
```

This produces the compiler binary at `target/release/ynz`. Run again whenever you change Rust source under `crates/` (typically: starting work on a new milestone, after a `git pull` that changed compiler code, etc.).

`cargo build` without `--release` produces an unoptimized binary at `target/debug/ynz` — fine for development, slightly slower runtime. Use `--release` when you want the production-grade compiler speed.

### Verify the compiler built

```
./target/release/ynz --version
```

---

## Writing and running Yinz programs

Once the compiler binary exists, write any `.ynz` file and run it.

**Example** — save this as `hello.ynz`:

```
function main() -> nothing {
  let name = "Patrick"
  let age = 35
  print("hello, world")
  print(name)
  print(age + 1)
}
```

**Run it:**

```
./target/release/ynz run hello.ynz
```

Output:
```
hello, world
Patrick
36
```

**Or build to an executable** (without running):

```
./target/release/ynz build hello.ynz
./hello                                   # the produced native binary
```

### Optional: make `ynz` a global command

Typing `./target/release/ynz` every time is tedious. Add it to your `PATH` for the session:

```
export PATH=/path/to/ynz/target/release:$PATH
```

(Replace `/path/to/ynz` with the absolute path to your repo root.)

Or alias it:

```
alias ynz=/path/to/ynz/target/release/ynz
```

Then you can use `ynz run hello.ynz` directly. To persist across shell sessions, add either line to your `~/.bashrc` or `~/.zshrc`.

**Note**: if you re-run `cargo build --release` (after a compiler change), the binary at `target/release/ynz` is replaced automatically — the alias/PATH continues to point at the latest build. No re-aliasing needed.

---

## CLI reference

```
ynz build <file>    # compile only — produces an executable next to the source
ynz run <file>      # compile and execute — for development iteration
ynz --version       # print compiler version
ynz --help          # full CLI help
```

---

## Editor support (VSCode)

The **Yinz Language** extension provides syntax highlighting, inline diagnostics, autocomplete, and hover docs for `.ynz` files.

- **GitHub Release**: download `yinz-0.2.0-m2.vsix` from the [latest release](https://github.com/yinzers/yinz-lang/releases/tag/ynz-vscode-v0.2.0-m2) and run `code --install-extension yinz-0.2.0-m2.vsix`
- **Manual install**: see [`tooling/vscode-ynz/README.md`](tooling/vscode-ynz/README.md) for local build + install instructions

The extension requires the `ynz-lsp` binary on your PATH: `cargo build -p ynz-lsp --release && cp target/release/ynz-lsp ~/.local/bin/`

---

## Status

The compiler is in active development. See [`design/mvp-scope.md`](design/mvp-scope.md) for the milestone roadmap, and [`.claude/plans/active/v0-1-compiler.md`](.claude/plans/active/v0-1-compiler.md) for the current v0.1 implementation plan.

**Current shipped milestones**:
- M1 ✓ — Hello world end-to-end (tag `v0.1.0-m1`)
- M2 ✓ — Variables, arithmetic, decimal128 numerics (tag `v0.1.0-m2`)
- M3 — Control flow + user-defined functions (in progress)
- M4-M8 — see roadmap

**What works today**: variables (`let`, `const`), arithmetic across `int`/`float`/`number`, `print`, three-part teaching error messages. You can write meaningful programs RIGHT NOW with M2 — try the example above.
