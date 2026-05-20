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

1. Download `yinz-0.2.0-m2.vsix` from the [latest release](https://github.com/patrickrizzardi/ynz/releases/tag/ynz-vscode-v0.2.0-m2)
2. Install: `code --install-extension yinz-0.2.0-m2.vsix`

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

Screenshots coming soon — see [`tooling/vscode-ynz/screenshots/`](https://github.com/patrickrizzardi/ynz/tree/main/tooling/vscode-ynz/screenshots) once the extension is verified.
