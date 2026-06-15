---
slug: m8-typeck-cross-file-resolution
type: execution
owner: Patrick Rizzardi
status: done
roadmap: v0-1-compiler
milestone: m8-typeck-cross-file-resolution
files:
  - crates/ynz-typeck/src/**
  - crates/ynz-driver/src/build.rs
  - examples/pirates-roster/entrypoint.ynz
  - examples/primantis-orders/m8_errors.ynz
created: 2026-05-18
last_updated: 2026-05-18
roadmap: v0-1-compiler
---

# Plan: M8 P2 Typeck Cross-File Resolution

Created: 2026-05-18
Status: pending_approval

## Context & Why

**Bug (Paper-Trace)**:
- Input: two files — `src/timeframes.ynz` has `export options Timeframe { daily: \`Daily\` }`, `src/entrypoint.ynz` has `import { Timeframe } from "timeframes"` and `shape Bar { timeframe: Timeframe }`.
- Expected: compiles cleanly, `Bar` field resolves to `Type::Options { name: "Timeframe" }`.
- Observed: `Error: \`Timeframe\` is not a known type.` on the field declaration line.
- Residual: `Timeframe` is never registered in any type table visible to `collect_shapes` for `entrypoint.ynz`.
- Evidence path: `crates/ynz-typeck/src/check.rs:139` — `Item::ImportDecl(_) | Item::ConstDecl(_) | Item::ReExport(_) => {}` — all import/export declarations are silently skipped in `check_module`. Combined with `crates/ynz-typeck/src/queries.rs:92-96` — `module_signatures_query` calls `collect_shapes` and `collect_signatures` with no import context.

M8 Phase 2 shipped import/export **grammar** (AST nodes, parser, `is_exported` flags on FunctionDecl/ShapeDecl/OptionsDecl) but zero typeck resolution. The audit agent confirmed every cross-file typeck feature from the M8 P2 plan is missing.

**What the M8 P2 plan specified** (`m8-modules-doc-sensitive-concurrency-bignum-release.md` lines 430–437):
- Pass-0: collect `(file, exported_name) → ItemRef` into a global ExportTable
- Pass-1: per-file symbol resolution against ExportTable; reject missing exports
- Cross-file: duplicate names, unused imports, circular imports, self-import rejection, codegen mangling

## Research Findings

- `ImportDecl.source` holds the project-root-relative path string (no `.ynz` suffix). `crates/ynz-ast/src/nodes.rs:56`.
- `is_exported: bool` already set on `FunctionDecl`, `ShapeDecl`, `OptionsDecl`, `ConstDecl`. All correct.
- `crates/ynz-driver/src/load.rs:48` — `find_project_root` already exists. **Do not reimplement in typeck.**
- `crates/ynz-driver/src/build.rs:77-80` — current multi-file loop creates SourceFile AND runs `codegen_query` in the same iteration, so no other file's SourceFile exists in the salsa db when the first file is typechecked.
- **Design decision: whole-graph vs per-import.** The original M8 plan specified a whole-graph approach (Pass-0 sees all files). This plan uses that approach. The alternative (per-import temp-CompilerDb) was rejected because: (a) creates temporary dbs that salsa cannot track as dependencies, so a change to an imported file does NOT invalidate the importer's typeck — silent stale cache; (b) diagnostics from imported files have no propagation path back to the importer's diagnostic bucket; (c) circular imports require custom detection rather than falling out of the salsa query graph naturally. Cost to fix later if we ship per-import: the entire import resolution layer must be rewritten for v0.2 LSP. Whole-graph adds ~30 lines to the driver and zero duct tape.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Salsa cycle on circular imports | Med | Compile-time hang | Pre-detect cycles in the driver's project load; emit diagnostic before salsa query runs |
| Windows path separator differences | Low | CI-only bug | Use `PathBuf` throughout, never string manipulation |
| Imported file has parse errors | Med | Wrong diagnostic to user | Propagate imported file's parse diagnostics under that file's path before attempting export resolution |
| `as alias` shadows local declaration | Med | Silent wrong binding | Local declarations win; emit error if alias collides with a local name |
| ReExport chains (A re-exports from B) | Low | Unresolved symbols | Defer multi-hop re-exports to Phase 3; document explicitly |

## Invariants This Milestone Must Preserve

### Safety
- A symbol from file A can only be used in file B if A explicitly marks it `export`. No implicit visibility across files.
- A circular import (A imports B, B imports A) must produce a compile error, never a hang or stale result.
- Self-import (file imports from its own path) must produce a compile error.
- An imported name that collides with a local declaration must produce a compile error (no silent shadowing).
- `is_exported=false` symbols must never appear in ExportTable — typeck of importing file must not resolve them.

### Performance
- **Auto-promotion analysis**: no auto-promotion candidates in this milestone. ExportTable is a data structure that doesn't have a stricter/faster form — it stores exactly what was declared. Import resolution is I/O-dominated; codegen-promotion doesn't apply.
- The whole-graph pre-pass runs once per build (not per file). O(N) for N source files.
- Salsa memoizes `module_signatures_query` per SourceFile — an unchanged file's query result is reused across re-builds. Cross-file dependency edges are tracked by salsa: changing file A invalidates any file B that called `module_signatures_query(db, A)`.

### Teaching
- Missing-module diagnostic: WHAT names the path attempted; WHAT-INSTEAD says to check spelling and that paths are project-root-relative without `.ynz`; WHY explains the root-relative convention.
- Missing-export diagnostic: WHAT names the symbol and the file it was expected from; WHAT-INSTEAD lists what IS exported from that file (closest-match suggestion); WHY explains that only `export`-prefixed declarations are visible.
- Circular-import diagnostic: WHAT names the cycle (A → B → A); WHAT-INSTEAD says to restructure so neither imports the other, or extract the shared type to a third file; WHY explains that circular imports make compilation order undefined.
- Unused-import diagnostic: Warning severity; WHAT names the unused binding; WHAT-INSTEAD says to remove it; WHY notes that unused imports add noise without contributing to the program.
- All diagnostic strings pass `tests/jargon_audit.rs` — no banned words from `BANNED_JARGON`.

### Runtime Dependencies
- Import resolution reads source files from disk (`std::fs::read_to_string`) — requires filesystem access. Kernel-mode `--kernel` builds where filesystem is unavailable: imports cannot be resolved; compiler must emit COMPILE ERROR stating that multi-file projects require filesystem access, pointing to `design/future/no-runtime-mode.md`.
- ExportTable is compile-time only — no heap allocation in the produced binary.

### Kernel-Mode Behavior
- `import` declarations in kernel-mode builds: COMPILE ERROR (filesystem read required for import resolution; kernel targets have no filesystem). Diagnostic: "Import declarations are not supported in kernel-mode builds. Inline all types in a single file." 
- Single-file programs with no imports: unchanged — kernel-mode still works.

### Demo & Error Gallery
- `examples/pirates-roster/entrypoint.ynz`: add a multi-file demo showing `import { Timeframe } from "timeframes"` and a shape with a `Timeframe` field — the success path. Add as a comment block since the demo is a single-file project; alternatively restructure the basics example to be a two-file project.
- `examples/primantis-orders/m8_errors.ynz`: add intentional triggers for each new error class introduced:
  - `import { Missing } from "nonexistent"` — missing module // WHY: missing-module diagnostic class
  - `import { notExported } from "has_exports"` — missing export // WHY: missing-export diagnostic class
  - `import { Foo } from "file_a"` + `import { Foo } from "file_b"` — duplicate name // WHY: duplicate-import diagnostic class
  - Two mutually-importing files — circular import // WHY: circular-import diagnostic class
  - `import { UnusedType } from "types"` (never used in file) — unused import // WHY: unused-import warning class

## Phases

### Phase 1: ExportTable + whole-graph salsa pre-pass in the driver
**PR scope**: `ExportTable` type + `file_for_path` salsa query + driver creates all SourceFiles before any codegen runs.
**Branch**: `feat/m8-typeck-export-table`
**Flag**: N/A
**Est. lines**: ~150
**Ships via**: `/pr`
**Objective**: Wire the salsa infrastructure so any file's query can look up another file by path. ExportTable is a pure data struct; `collect_exports` is a pure function — both testable without I/O.
**Why this phase exists**: All cross-file resolution depends on (a) ExportTable data type and (b) being able to look up a SourceFile by path in the salsa db. Without these, Phases 2 and 3 have nowhere to put the result.
**Current-state anchors**:
- `crates/ynz-driver/src/build.rs:77-80` — current loop mixes SourceFile creation + codegen in same iteration
- `crates/ynz-typeck/src/shapes.rs:36` — ShapeDef (what exported shapes expose)
- `crates/ynz-typeck/src/options_table.rs:7` — OptionsEntry (what exported options expose)
- `crates/ynz-typeck/src/signatures.rs` — FunctionSig (what exported functions expose)
- `crates/ynz-driver/src/load.rs:161` — `load_project` already loads all source files
**Files (expected scope)**:
- `crates/ynz-typeck/src/exports.rs` (new)
- `crates/ynz-typeck/src/lib.rs` (add mod exports)
- `crates/ynz-parser/src/lib.rs` (add `file_for_path` salsa input or interned string input)
- `crates/ynz-driver/src/build.rs` (split loop: all SourceFiles first, then all codegen)
**Steps**:
1. Create `crates/ynz-typeck/src/exports.rs`:
   ```rust
   pub struct ExportTable {
       pub shapes: HashMap<String, ShapeDef>,    // key = exported name
       pub options: HashMap<String, OptionsEntry>,
       pub functions: HashMap<String, FunctionSig>,
   }
   impl ExportTable { pub fn empty() -> Self { ... } }
   pub fn collect_exports(module: &Module, shape_table: &ShapeTable,
       options_table: &OptionsTable, sig_table: &SignatureTable) -> ExportTable
   ```
   `collect_exports` filters items by `is_exported=true` and copies the corresponding entry from each table into ExportTable. No wrapper structs — the map keys ARE the names.
2. Add `PartialEq` for `ExportTable` (same-key-set coarse equality, matching queries.rs pattern).
3. In the driver, split `build_project`'s loop: (a) first loop creates all `SourceFile` inputs and stores `path → SourceFile` in a local `HashMap`; (b) second loop calls `codegen_query`. This ensures all SourceFiles exist in the salsa db before any query runs.
4. Add `file_for_path` as a salsa interned or tracked input: `HashMap<String, SourceFile>` registered on the db (or an accessor via a module-level `HashMap` built in step 3 and passed through).
**Acceptance criteria**:
- [ ] `ExportTable` struct in `exports.rs` with `shapes`, `options`, `functions` HashMap fields
- [ ] `collect_exports` returns ONLY `is_exported=true` symbols; non-exported shapes absent
- [ ] `ExportTable` has a `PartialEq` impl
- [ ] Driver loop split: all SourceFiles created before first `codegen_query` runs
- [ ] Unit test: export two shapes (one exported, one not) → ExportTable has exactly the exported one
- [ ] `cargo test` green

### Phase 2: Import resolution in `module_signatures_query`
**PR scope**: Imported shapes and options visible in shape field type annotations. Full diagnostics for missing module/symbol/duplicate/circular/self-import.
**Branch**: `feat/m8-typeck-import-sigs`
**Flag**: N/A
**Est. lines**: ~280
**Ships via**: `/pr`
**Objective**: When `module_signatures_query(db, B)` runs and B has `import { Timeframe } from "timeframes"`, resolve `timeframes.ynz` via `file_for_path`, call `module_signatures_query(db, A)` on the imported file (salsa memoizes + tracks the dep), build ExportTable from the result, merge imported shapes/options into B's local tables. Remove the same-file `options_names` duct tape.
**Why this phase exists**: Shape field type annotations are resolved in `module_signatures_query` → `collect_shapes`. Importing shapes/options types MUST be in scope by the time `collect_shapes` runs.
**Current-state anchors**:
- `crates/ynz-typeck/src/queries.rs:92` — `collect_shapes` called with no import context
- `crates/ynz-typeck/src/shapes.rs:210` — `collect_shapes` entry point
- `crates/ynz-typeck/src/shapes.rs:63-66` — `ShapeTable.options_names` (duct tape to DELETE)
- `crates/ynz-ast/src/nodes.rs:54-60` — `ImportDecl { kind, source, source_span, span }`
- `crates/ynz-driver/src/load.rs:161` — `find_project_root` — USE THIS, do not reimplement
**Path resolution rules (authoritative)**:
- `"module"` → `<project_root>/module.ynz` where project_root is nearest ancestor directory containing `yinz.toml`
- If no `yinz.toml` found walking up from importer's directory: COMPILE ERROR — "Import requires a `yinz.toml` project file. Add one at the project root."
- Relative paths (containing `/`), absolute paths, paths starting with `.`: COMPILE ERROR at parse time (already handled by M8 P2 parser; confirm)
- Self-import (resolved path == importing file's path): COMPILE ERROR
- Path on Windows: use `PathBuf` only, no string path separator assumptions
**Alias semantics (authoritative)**:
- `import { Foo as Bar } from "x"` → registers `Bar` in local scope; `Foo` is NOT in local scope
- If `Bar` already declared locally (as shape, options, function): COMPILE ERROR — "Import alias `Bar` collides with a local declaration at [span]."
- If `Bar` imported from another module in the same file: COMPILE ERROR — "Import name `Bar` is already bound by a previous import."
- Local declarations WIN: typeck's name resolution checks local scope first, then imported scope.
**Files (expected scope)**:
- `crates/ynz-typeck/src/exports.rs` (add `resolve_imports` function)
- `crates/ynz-typeck/src/queries.rs` (extend `module_signatures_query`)
- `crates/ynz-typeck/src/shapes.rs` (extend `collect_shapes` params; delete `options_names` field)
- `crates/ynz-typeck/src/options_table.rs` (extend `collect_options` params)
**Steps**:
1. **Circular import detection**: before calling `module_signatures_query(db, imported_sf)`, check a thread-local or passed-in `visiting: &mut HashSet<String>` (by canonical path). If `imported_path` already in `visiting`: emit circular-import diagnostic, return empty ExportTable. This prevents stack overflow.
2. In `module_signatures_query`: scan `ImportDecl` items, for each call `resolve_import_path` (wrapping `find_project_root`) → `file_for_path(db, path)` → `module_signatures_query(db, that_sf)` → `collect_exports(...)`. Handle diagnostics:
   - File not found → `"Module \"foo\" not found at <path>. Check the module name and ensure the file exists. Import paths are project-root-relative without the .ynz suffix."`
   - Symbol not exported → `"\"Foo\" is not exported from \"foo\". Exported names: [list]. Add \`export\` before the declaration in foo.ynz."`
   - Self-import → `"A file cannot import from itself."`
   - Duplicate binding → see alias semantics above
3. Build `imported_shapes: HashMap<String, ShapeDef>` and `imported_options: HashMap<String, OptionsEntry>` (keyed by local name).
4. Add `imported_shapes` and `imported_options` params to `collect_shapes`; merge into the name table before field type resolution. **Delete** `ShapeTable.options_names` — it's fully superseded.
5. Add `imported_options` param to `collect_options`; merge imported options into the local table.
6. Propagate diagnostic strategy for imported-file errors: if `module_signatures_query(db, imported_sf)` returns diagnostics (the imported file has errors), emit a single summary diagnostic on the import statement: `"Module \"foo\" has errors — fix those first."` Do NOT re-emit the imported file's individual diagnostics under the importer's path; they will surface when that file is compiled on its own.
**Acceptance criteria**:
- [ ] `shape Bar { timeframe: Timeframe }` where `Timeframe` imported from another file compiles without "not a known type" error
- [ ] `ShapeTable.options_names` field is deleted
- [ ] Same-file options-in-shapes still works (covered by existing tests)
- [ ] Missing module → single clear diagnostic with path shown
- [ ] Missing export → diagnostic lists what IS exported (closest-match suggestion)
- [ ] Self-import → compile error, not hang
- [ ] Circular import → compile error naming the cycle, not hang
- [ ] Duplicate import binding → compile error
- [ ] Alias collision with local name → compile error
- [ ] Unused-import: deferred to Phase 3 (no regression — no warning today)
- [ ] `cargo test` green; jargon_audit passes on all new diagnostic strings
**Test definitions (Given/When/Then)**:
- **Happy path**: Given `a.ynz` exports `options Color { red, blue }`, B imports it and uses `Color` in a field; When `ynz run b.ynz`; Then exit 0, no stderr.
- **Missing module**: Given import from `"nonexistent"`; When compile; Then stderr contains `'nonexistent'` and `project-root-relative` and exits nonzero.
- **Missing export**: Given `a.ynz` exports `Foo`, B imports `Bar` from a; When compile; Then stderr contains `'Bar' is not exported` and `Exported names: Foo`.
- **Circular**: Given A imports B, B imports A; When compile; Then stderr contains the cycle names A and B, exits nonzero, returns in <2s (no hang).
- **Self-import**: Given `a.ynz` imports from `"a"`; When compile; Then stderr contains `cannot import from itself`.
- **Alias collision**: Given local `shape Bar` AND `import { Foo as Bar } from "x"`; When compile; Then stderr contains `Bar` and `collides`.

### Phase 3: Import resolution in `check_query` + unused-import warnings + ConstDecl/ReExport stub
**PR scope**: Imported functions callable in function bodies; unused-import warning; ConstDecl and ReExport documented deferral.
**Branch**: `feat/m8-typeck-import-bodies`
**Flag**: N/A
**Est. lines**: ~200
**Ships via**: `/pr`
**Objective**: After Phase 2, imported types work in annotations. This phase makes imported functions callable in function bodies, and adds the unused-import warning.
**Why this phase exists**: `module_signatures_query` resolves types for field annotations. `check_query` resolves function body references. Both need import context.
**ConstDecl typeck deferral (explicit)**:
- `Item::ConstDecl` is still `_ => {}` in check.rs. Cross-file ConstDecl resolution deferred to v0.2 with explicit trigger: "when a user writes `import { MY_CONST } from "constants"` and uses it in a function body." Currently const decls in the same file are also unimplemented (ConstDecl typeck is entirely unimplemented — see `check.rs:139`). This is tracked separately.
- @design-decision Same-file ConstDecl resolution deferred to v0.2. @rationale Constants require expression-level evaluation which needs a const-eval pass not yet designed. @cost-to-fix ~1 session: add const-eval pass, then cross-file is trivial. @trigger First user complaint about `const MAX = 100` not resolving.
**ReExport deferral (explicit)**:
- Multi-hop re-exports (`export { X } from "B"` in A, where B itself exports X) deferred to v0.2. @design-decision Single-hop re-exports deferred. @rationale Requires following the re-export chain; circular detection extends. @cost-to-fix ~0.5 sessions: extend the import resolution loop to follow `ReExport` items. @trigger User writes `export { X } from "Y"` and tries to import X from the re-exporting file.
**Current-state anchors**:
- `crates/ynz-typeck/src/queries.rs:111` — `check_query` calls `check()` with no import context
- `crates/ynz-typeck/src/check.rs:139` — the no-op catch-all
**Files (expected scope)**:
- `crates/ynz-typeck/src/queries.rs` (extend `check_query`)
- `crates/ynz-typeck/src/check.rs` (process ImportDecl in `check_module`; track usage for unused-import)
**Steps**:
1. In `check_query`: reuse Phase 2's import resolution to also build `imported_fns: HashMap<String, FunctionSig>`. Pass to `check()`.
2. In `check_module`: merge `imported_fns` into the local signature table before function body checking. When resolving a call `Foo.bar()` or `bar()`, look in imported scope if not found locally.
3. Track which imported bindings are referenced during body checking. After all bodies checked, emit `Severity::Warning` for unreferenced bindings: `"'Timeframe' is imported but never used. Remove the import or use the type."`
4. **Namespace imports** (`import ns from "x"`): bound as a prefix — `ns.Foo` resolves `Foo` in the exported scope of x. Namespace is considered "used" if ANY member is accessed via the namespace prefix.
5. Integration test fixture: two-file project where B imports and calls a function from A → `ynz run B.ynz` prints expected output.
**Acceptance criteria**:
- [ ] Imported function callable by name in importing file's function bodies
- [ ] Unused import produces Warning (not Error), with the binding name named
- [ ] Used import produces no warning
- [ ] ConstDecl and ReExport handling documented with explicit @design-decision comments in check.rs:139
- [ ] Integration test: two-file project with cross-file function call → correct output
- [ ] `cargo test` green including driver integration tests
- [ ] `examples/pirates-roster/entrypoint.ynz` extended with cross-file usage example
- [ ] `examples/primantis-orders/m8_errors.ynz` extended with all five new error trigger cases

## Quality Checklist
- [ ] All diagnostics follow three-part WHAT/WHAT-INSTEAD/WHY format
- [ ] `cargo test -p ynz-diagnostics -- jargon_audit` passes (no banned words in new diagnostic strings)
- [ ] Circular import cannot cause hang — each adversarial case returns in <2s
- [ ] File not found gives actionable message (path shown, root-relative convention explained)
- [ ] `ShapeTable.options_names` duct-tape field deleted in Phase 2
- [ ] ConstDecl and ReExport deferrals explicitly documented with @design-decision comments
- [ ] All path manipulation uses `PathBuf`, not string operations

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: Each phase is one PR via `/pr`; no mid-phase commits outside the phase's branch
- **Shadow main branches**: Three branches fan off main; each merges before the next starts — no long-lived shadow
- **Building the engine before shipping value**: Phase 2 is the user-visible fix (cross-file type annotations work); Phase 1 is a 150-line data struct + driver split that enables it directly — not "building the engine" standalone
- **Hotfix that isn't**: This IS the proper fix for what M8 P2 missed. The `options_names` duct tape is explicitly deleted in Phase 2.
- **Abandoned branches**: Each phase ships via `/pr` before next starts; three small PRs not one giant branch
- **Flag graveyards**: No feature flags — import resolution is always-on infrastructure with no A/B testing need
