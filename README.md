# Yinz

A compiled systems programming language. Rust-level performance, TypeScript-level readability. Memory-safe, zero-cost abstractions, approachable by developers at any experience level.

File extension: `.ynz`. Compiler target: LLVM native machine code.

For the language specification, see [`spec/overview.md`](spec/overview.md).

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

### Build

```
cargo build --workspace
```

### Run

```
cargo run -p ynz-driver -- --version
```

---

## Status

The compiler is in active development. See [`design/mvp-scope.md`](design/mvp-scope.md) for the milestone roadmap.
