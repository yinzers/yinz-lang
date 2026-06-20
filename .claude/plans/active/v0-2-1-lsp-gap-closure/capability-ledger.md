# Capability Ledger: v0.2.1 LSP Gap Closure

Every capability the initiative will deliver maps to a milestone. Milestone 10 shipped (its child plan `v0-2-1-m10-teaching-surface-bugfix` is `done`); the Notes column records what it delivered. Milestones 1–9 and 11 have no child execution plan yet — they are `NEEDS-PLANNED` (run `/plan` when each is picked up), owned by their roadmap-sequenced milestone slug.

| Capability | Owning milestone | Status | Notes |
|---|---|---|---|
| Rename-shadowing rejection — a rename can never produce silent-wrong-output via accidental shadow; rejected with use-site error | v0-2-1-m1-rename-shadowing | NEEDS-PLANNED | No child plan yet. |
| Signature help — parameter list, names, types, active-argument position shown inline inside a call | v0-2-1-m2-signature-help | NEEDS-PLANNED | No child plan yet. |
| Code lens — "N references" inline above every top-level function/shape/options block, click to jump | v0-2-1-m3-code-lens | NEEDS-PLANNED | No child plan yet. |
| Lint Tier 3 infrastructure + 2 starter rules (`array_to_fixed_promotion`, `let_to_const_promotion` as tier-3 lints with WHAT/WHAT-INSTEAD/WHY hover) | v0-2-1-m4-lint-tier3 | NEEDS-PLANNED | No child plan yet. Future lint rules ship via one registry entry + analysis pass. |
| Document highlight + selection range — occurrence highlight, AST-node smart-expand selection, UFCS-site co-highlight | v0-2-1-m5-doc-highlight-selection-range | NEEDS-PLANNED | No child plan yet. |
| Type-definition + implementation provider — jump-to-TYPE, "Go to Implementations" for contract followers | v0-2-1-m6-type-def-implementation | NEEDS-PLANNED | No child plan yet. Missing piece for contract-based design in non-OOP Yinz. |
| Type hierarchy + call hierarchy — tree navigation for extends/follows chains and incoming/outgoing calls | v0-2-1-m7-type-hierarchy-call-hierarchy | NEEDS-PLANNED | No child plan yet. |
| Diagnostic enrichment + semantic-token modifiers — struck-through deprecated features, dimmed unused, cross-file related-info, const/lend/intrinsic token styling | v0-2-1-m8-diagnostic-enrichment-semantic-tokens | NEEDS-PLANNED | No child plan yet. |
| Extension polish + release — completion-item resolve, 5 snippets, 2 commands, per-domain inlay toggles, 0.2.1 release tag | v0-2-1-m9-extension-polish-release | NEEDS-PLANNED | No child plan yet. |
| Teaching-surface correctness — fix every bug from the 2026-05-21 four-agent audit (false-positive unused-import, inlay-line position, click-to-explicit promotion hint, suppressed promotion hints, keyword-name hover, BannedJargon quick-fix, space-triggered completion popup, `booleanean`/`infers` jargon) | v0-2-1-m10-teaching-surface-bugfix | done | Delivered (child `v0-2-1-m10-teaching-surface-bugfix`, status: done — Phase 2 complete). |
| Teaching-content polish — Go-model `//` comment syntax, colored signature hover, field hover on dot-access, 96 intrinsic hover docs, 9 keyword hover entries, per-domain WHY templates, inlay context strings, 15 diagnostic WHY improvements | v0-2-1-m11-teaching-content-polish | NEEDS-PLANNED | No child plan yet. |
