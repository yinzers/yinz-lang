# Yinz Tooling

This directory contains editor distributions and build outputs for Yinz developer tooling. It is **not** part of `cargo build --workspace` — builds here are opt-in.

## Contents

### `tooling/vscode-ynz/` (added in v0.2-M2)

The Yinz VSCode extension. Spawns the `ynz-lsp` binary over stdio and registers the `.ynz` language association.

**Build requirements:** Node.js, npm, `vsce` CLI.

**Build flow:**
1. Build the LSP binary: `cargo build -p ynz-lsp --release`
2. Install extension deps: `cd tooling/vscode-ynz && npm install`
3. Package the extension: `npx vsce package`
4. Install locally: `code --install-extension yinz-*.vsix`

The extension is published to the VSCode Marketplace as a preview. See [`tooling/vscode-ynz/README.md`](vscode-ynz/README.md) for marketplace install instructions.

## Conventions

- Each subdirectory is one editor or tool distribution.
- CI does NOT build tooling by default. Extension builds run via separate GitHub Actions jobs triggered on `tooling/**` changes.
- Version numbers mirror the Yinz compiler version (e.g., `0.2.0-m2`).
- Secrets (marketplace tokens, API keys) are NEVER committed here. The CI job injects them via environment variables.
