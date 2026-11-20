# Yinz

A compiled systems programming language. Rust-level performance, TypeScript-level readability. Memory-safe, zero-cost abstractions, approachable by developers at any experience level.

File extension: `.ynz`. Compiler target: LLVM native machine code.

For the language specification, see [`docs/reference/REF-language-overview.md`](docs/reference/REF-language-overview.md). All project documentation starts at [`docs/README.md`](docs/README.md).

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

Add the `LLVM_SYS_181_PREFIX` export to your shell profile on macOS. On Linux it is set automatically via [`.cargo/config.toml`](.cargo/config.toml).

**Rust stable toolchain:** Install via [rustup](https://rustup.rs).

**Node.js 22:** needed only to build the VSCode extension in [`tooling/vscode-ynz/`](tooling/vscode-ynz/).

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

### Dev container

[`docker-compose.yml`](docker-compose.yml) defines a `dev` service (image `ynz-dev`, built from [`Dockerfile`](Dockerfile)). The image holds the full toolchain: Rust (stable, plus nightly with Miri and `rust-src` for the sanitizer lanes), LLVM 18, `cargo-nextest` and Node 22. Compiler build, test and fixture work runs inside this container. The container runs as the `ubuntu` user (uid 1000), so files it writes into the project are owned by host uid 1000.

The bind mount `.:/work` keeps `target/` readable on the host.

```bash
# Start the dev container (background)
docker compose up -d dev

# Or run one-shot commands (container exits after each)
docker compose run --rm dev cargo build --workspace
docker compose run --rm dev cargo build -p ynz-driver
docker compose run --rm dev cargo build -p ynz-driver --release
docker compose run --rm dev cargo build -p ynz-lsp --release
docker compose run --rm dev cargo test --workspace
docker compose run --rm dev cargo clippy --workspace -- -D warnings
docker compose run --rm dev cargo fmt --all
docker compose run --rm dev cargo run -p ynz-pascalcase-codemod -- --help   # rename old lowercase built-in types (int, map, ...) to PascalCase, in type positions only, across any tree

# Run the compiler directly inside a running dev container
docker compose exec dev ./target/debug/ynz run crates/ynz-driver/tests/fixtures/hello.ynz
# → hello, yinz

# VSCode extension build
docker compose run --rm dev bash -c "cd tooling/vscode-ynz && npm install && npm run build && npx vsce package --no-yarn"
```

---

## Writing and running Yinz programs

Once the compiler binary exists, write any `.ynz` file and run it.

**Example** — save this as `hello.ynz`:

```
function entrypoint() -> Nothing {
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

- **GitHub Release**: download [`yinz-latest.vsix`](https://github.com/yinzers/yinz-lang/releases/latest/download/yinz-latest.vsix) (a stable URL that always points at the newest release) and run `code --install-extension yinz-latest.vsix`
- **Manual install**: see [`tooling/vscode-ynz/README.md`](tooling/vscode-ynz/README.md) for local build + install instructions

The extension requires the `ynz-lsp` binary on your PATH: `cargo build -p ynz-lsp --release && cp target/release/ynz-lsp ~/.local/bin/`

---

## Status

The compiler is in active development. The current version is **0.3.3** (workspace [`Cargo.toml`](Cargo.toml)). See [`CHANGELOG.md`](CHANGELOG.md) for what each release ships and [`docs/reference/REF-mvp-scope.md`](docs/reference/REF-mvp-scope.md) for the roadmap.

**Shipped so far** (per the changelog):
- **v0.1** — the core language (M1–M8 language features).
- **v0.2** — the feature registry, the language server and VSCode extension, `ynz fmt`, and the `ynz watch` rebuild-on-save daemon.
- **v0.3** — `background` and `wait` concurrency, channels, auto-parallelization, and an LLVM `default<O2>` optimizer pipeline.
