# Yinz Language Extension for VSCode

Syntax highlighting, inline diagnostics, autocomplete, and hover docs for `.ynz` files.

## Features

- **Syntax highlighting** — keywords, deferred features (shown as illegal), banned keywords (shown as deprecated), strings, comments, literals
- **Inline diagnostics** — WHAT/WHAT-INSTEAD/WHY teaching content from the Yinz compiler
- **Autocomplete** — keywords, primitive methods, type constants, deferred features (marked deprecated)
- **Hover docs** — registry-sourced WHY content for every keyword, intrinsic, and deferred feature

## Prerequisites

Build the `ynz-lsp` binary from the repo root:

```bash
cargo build -p ynz-lsp --release
# copy to PATH or set yinz.server.path in VSCode settings
cp target/release/ynz-lsp ~/.local/bin/  # or any directory on your PATH
```

## Install

### Option A — From GitHub Release (current method)

Always-current stable URL (updates automatically with each release):

```bash
curl -L https://github.com/yinzers/yinz-lang/releases/latest/download/yinz-latest.vsix -o yinz-latest.vsix
code --install-extension yinz-latest.vsix
```

Or download a specific version from the [releases page](https://github.com/yinzers/yinz-lang/releases).

### Option B — VSCode Marketplace (coming soon)

Once published: search **"Yinz Language"** in the VSCode Extensions panel, or:

```
ext install yinz-lang.yinz
```

## Configuration

| Setting | Default | Description |
|---|---|---|
| `yinz.server.path` | `ynz-lsp` | Path to the `ynz-lsp` binary |

## Screenshots

Screenshots are tracked for a follow-up commit once the extension is installed and verified locally. Install the extension and open `examples/basics/src/entrypoint.ynz` to see syntax highlighting, hover docs, autocomplete, and inline diagnostics in action.
