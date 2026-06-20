# Capability Ledger: v0.2 Dev-Loop Tooling

Every capability this initiative delivers maps to an owning milestone. All five milestones shipped (v0.2.0 tag cut 2026-05-21); the Notes column records what each done milestone delivered (the done-child ownership record that stands in for retroactive per-phase Scope Boundary blocks).

| Capability | Owning milestone | Status | Notes |
|---|---|---|---|
| Single SSOT feature registry consolidating all scattered registries (banned jargon, intrinsics, reserved features, type-attached constants, diagnostic templates, muted-hint domains) + consistency tests + project-wide "must register" rule | v0-2-m1-feature-inventory-sync | done | Delivered: registry crate + features.toml + plan-invariant check + graveyard entry + CLAUDE.md rule. Compiler behavior unchanged (refactor + foundation). |
| LSP thin slice — autocomplete, inline errors, basic hover — installable VSCode extension | v0-2-m2-lsp-thin-slice | done | Delivered: ynz-lsp crate + tooling/vscode-ynz extension; first eyes-on LSP architecture validation. |
| `ynz fmt` formatter — single-file, `--all`, `--check` CI gate, library API for format-on-save | v0-2-m3-fmt | done | Delivered: ynz-fmt crate (separate, library-API-ready for M5 LSP format-on-save). 3 plan-review rounds. |
| `ynz watch` — recompile-on-save, sub-second turnaround, shared diagnostic rendering, `--json` structured events | v0-2-m4-watch | done | Delivered: ynz-watch crate + `--json` NDJSON events. Post-ship bug fixes applied. |
| LSP Full — 8 new capabilities (go-to-def, find-refs, rename, format-on-save, inlay hints, code actions, semantic tokens, doc hover) + 3 compiler correctness fixes + `ynz build --json` + VSCode v0.2.0 + v0.2.0 release tag | v0-2-m5-lsp-full-and-release | done | Delivered: full editor experience; cut v0.2.0 (first plain-version tag); GitHub release with vsix. |
