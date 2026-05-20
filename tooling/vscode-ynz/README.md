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

### Option A — From `.vsix` (current method)

1. Build: `cd tooling/vscode-ynz && npm install && npx vsce package`
2. Install: `code --install-extension yinz-0.2.0-m2.vsix`

### Option B — VSCode Marketplace (preview)

Search **"Yinz Language"** in the VSCode Extensions panel, or install directly:

```
ext install yinz-lang.yinz
```

## Configuration

| Setting | Default | Description |
|---|---|---|
| `yinz.server.path` | `ynz-lsp` | Path to the `ynz-lsp` binary |

## Screenshots

![Hover docs](screenshots/hover.png)

![Autocomplete](screenshots/autocomplete.png)

![Inline diagnostics](screenshots/diagnostic.png)
