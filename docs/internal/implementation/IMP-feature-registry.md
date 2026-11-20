---
name: "IMP-feature-registry"
description: "Design rationale and schema reference for the registry/features.toml single source of truth that replaced scattered language-feature inventories across the compiler crates."
tags:
  - "yinz-compiler"
created_at: "2026-05-19"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Feature Registry — SSOT for Language Feature Inventories

**Cross-references**: [`.claude/rules/feature-registry.md`](../../../.claude/rules/feature-registry.md) (project rule), `crates/ynz-registry/` (the crate), [`registry/features.toml`](../../../registry/features.toml) (the data file).

This document is the schema reference and design for the feature registry. For the project rule governing when to add entries, see [`.claude/rules/feature-registry.md`](../../../.claude/rules/feature-registry.md). For the data file itself, see [`registry/features.toml`](../../../registry/features.toml).

---

## Why This Registry Exists

The inventories of language features (keywords, banned words, built-in functions and methods, type-attached constants, deferred features, muted-hint domains, lint rules, diagnostic templates) live in one TOML data file, [`registry/features.toml`](../../../registry/features.toml). `crates/ynz-registry/build.rs` parses it and generates typed Rust constants into `OUT_DIR/registry.rs`. The lexer, type checker, code generator, diagnostics and LSP all read from that one file, so a list a user sees (autocomplete, hover, "did you mean", teaching errors) cannot drift from the list the compiler enforces. A new feature item is added to the registry first and code follows it.

---

## Schema Reference

[`registry/features.toml`](../../../registry/features.toml) uses TOML array-of-tables (`[[entry_kind]]`) for each entry type. Every entry kind has required fields; `build.rs` panics with a clear message if any required field is missing.

### `[[keyword]]`

A valid Yinz keyword that the lexer recognizes and emits as a typed token.

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The keyword text (e.g. `"function"`) |
| `token` | string | yes | The `Token::Xxx` variant emitted (e.g. `"Function"`) |
| `since` | string | yes | Milestone tag when this keyword shipped (e.g. `"M1"`, `"M4"`) |

**Example**:
```toml
[[keyword]]
name = "function"
token = "Function"
since = "M1"

[[keyword]]
name = "shape"
token = "Shape"
since = "M4"
```

### `[[banned_declaration_keyword]]`

A keyword from another language that Yinz's lexer intercepts and redirects with a teaching error. Distinct from `[[banned_jargon]]` (which applies to user-prose diagnostics) — these fire at lex time and produce compiler errors.

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The banned keyword text (e.g. `"class"`) |
| `what_instead` | string | yes | Concrete replacement for the user to write |
| `why` | string | yes | Why Yinz uses a different approach |
| `since` | string | yes | Milestone when this ban was added |

**Example**:
```toml
[[banned_declaration_keyword]]
name = "class"
what_instead = "Use `shape` for data and standalone functions for behavior: `shape Player { name: String }` then `function greet(share self: Player) -> String`"
why = "Yinz is not object-oriented — there are no classes. Data lives in shapes; behavior lives in standalone functions called via dot-call syntax."
since = "M4"
```

### `[[banned_jargon]]`

A word that must not appear in user-facing compiler diagnostic prose. Enforced by `crates/ynz-diagnostics/src/banned_jargon.rs` and audited by `tests/jargon_audit.rs`.

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The banned word or acronym |
| `replacement` | string | yes | The Yinz-idiomatic term to use instead |
| `reason` | string | yes | Why this word is jargon (what audience it excludes) |
| `is_acronym` | bool | no | `true` for multi-character acronyms (`ADT`, `AST`, `UTF-16`). Default `false`. |

**Example**:
```toml
[[banned_jargon]]
name = "infer"
replacement = "figure out automatically"
reason = "compiler/type-theory jargon not known to junior developers"

[[banned_jargon]]
name = "ADT"
replacement = "shape or options type"
reason = "CS-degree acronym"
is_acronym = true
```

### `[[primitive_intrinsic]]`

A built-in function or method available on primitive types. Covers `print`, `range`, zero-arg and one-arg methods on `Int`/`Float`/`Number`/`Boolean`/`String`. Type-checked in `crates/ynz-typeck/src/intrinsics.rs` and `builtins.rs`.

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Function or method name |
| `kind` | string | yes | `"print_type"` \| `"free_fn"` \| `"method"` \| `"method_1arg"` |
| `receiver_type` | string | kind=method | `"Int"` \| `"Float"` \| `"Number"` \| `"Boolean"` \| `"string"` |
| `param_types` | array of strings | free_fn / method_1arg | List of parameter types in order |
| `return_type` | string | yes (except print_type) | Return type. Use `"Nothing"` for no value, `"Maybe<Int>"` etc. for maybe |
| `since` | string | yes | Milestone tag |

**Example**:
```toml
[[primitive_intrinsic]]
name = "range"
kind = "free_fn"
param_types = ["Int"]
return_type = "Range<Int>"
since = "M3"

[[primitive_intrinsic]]
name = "range"
kind = "free_fn"
param_types = ["Int", "Int"]
return_type = "Range<Int>"
since = "M3"

[[primitive_intrinsic]]
name = "toFloat"
kind = "method"
receiver_type = "Int"
param_types = []
return_type = "Float"
since = "M3"

[[primitive_intrinsic]]
name = "wrappingAdd"
kind = "method_1arg"
receiver_type = "Int"
param_types = ["Int"]
return_type = "Int"
since = "M4"
```

### `[[type_attached_constant]]`

A constant accessible as `Type.name` (e.g. `Int.max`, `Number.epsilon`). Both the type-checker and codegen read this — the type-checker to assign a type, codegen to emit the LLVM constant value.

| Field | Type | Required | Description |
|---|---|---|---|
| `type_name` | string | yes | `"Int"` \| `"Float"` \| `"Number"` |
| `const_name` | string | yes | `"max"` \| `"min"` \| `"epsilon"` |
| `value_type` | string | yes | Yinz type of the constant (`"Int"`, `"Float"`, `"Number"`) |
| `value_literal` | string | yes | Numeric value as string (avoids precision loss in TOML). `"9223372036854775807"` for `Int.max`. |
| `since` | string | yes | Milestone tag |

**Example**:
```toml
[[type_attached_constant]]
type_name = "Int"
const_name = "max"
value_type = "Int"
value_literal = "9223372036854775807"
since = "M4"

[[type_attached_constant]]
type_name = "Number"
const_name = "epsilon"
value_type = "Number"
value_literal = "1e-33"
since = "M4"
```

### `[[deferred_language_feature]]`

A language feature that is reserved at the lexer/parser level but whose full implementation ships in a future version. The registry entry drives the compiler error shown when a user writes the reserved syntax.

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The token or syntax that triggers this error (e.g. `"f32"`, `"test"`, `"gpu"`) |
| `substitute` | string | yes | What to use instead right now. Empty string if no current substitute. |
| `why` | string | yes | Why this is deferred. Must be user-readable — no jargon. |
| `ships_in` | string | yes | Target version string (`"v2+"`, `"v0.13"`, `"v0.2"`, etc.) |
| `design_doc` | string | yes | Path to the design doc (from repo root). Use `"docs/reference/REF-mvp-scope.md"` for items covered there rather than a dedicated future doc. |
| `triggers` | string | yes | What user code makes this error fire. `"none — no token reserved yet"` if not yet reserved. |

**Example**:
```toml
[[deferred_language_feature]]
name = "f32"
substitute = "Use `Float` (f64) for floating-point math. Sized floats ship in v2+ for GPU and embedded work."
why = "Yinz keeps the numeric type system small in v0.1: `Int` (i64), `Float` (f64), `Number` (decimal128). Sized variants add cognitive load without paying off for typical application code."
ships_in = "v2+"
design_doc = "docs/reference/REF-mvp-scope.md"
triggers = "Writing `f32` or `f64` as a type annotation in Yinz source"

[[deferred_language_feature]]
name = "test"
substitute = ""
why = "`test` is reserved for the built-in test framework shipping in v0.13. Pre-reserving it means your existing code will not break when v0.13 ships."
ships_in = "v0.13"
design_doc = "docs/reference/REF-mvp-scope.md"
triggers = "Using `test` as an identifier"
```

### `[[deferred_tooling_feature]]`

Same schema as `[[deferred_language_feature]]` but for tooling-side deferrals (compiler flags, build system features). Separate kind so consumers can filter by category.

### `[[deferred_stdlib_api]]` *(RESERVED — no M1 entries)*

Schema accommodated for v0.5+ stdlib module migrations. Each v0.5+ milestone populates its own entries; M1 ships zero.

| Field | Type | Required | Description |
|---|---|---|---|
| `module` | string | yes | Module name (`"file"`, `"math"`, etc.) |
| `method` | string | yes | Method name |
| `ships_in` | string | yes | Target version |
| `design_doc` | string | yes | Path to stdlib design doc |

### `[[diagnostic_template]]`

Canonical WHAT/WHAT-INSTEAD/WHY text for `DiagnosticKind` variants that have a reusable message shape. Dynamic per-site construction stays in code; the registry owns the canonical text skeleton. Supports `{placeholder}` substitution (unknown placeholder = `build.rs` panic naming the template and the key).

| Field | Type | Required | Description |
|---|---|---|---|
| `kind_name` | string | yes | `DiagnosticKind` variant name (e.g. `"MutationOfConst"`) |
| `what_template` | string | yes | Parameterized WHAT. `{binding}` etc. substituted at render time. |
| `what_instead_template` | string | yes | Parameterized WHAT-INSTEAD. |
| `why_template` | string | yes | Parameterized WHY. |

**Example**:
```toml
[[diagnostic_template]]
kind_name = "MutationOfConst"
what_template = "`{binding}` is declared `const` and cannot be changed."
what_instead_template = "Change `const {binding}` to `let {binding}` if you need to reassign it."
why_template = "`const` bindings are immutable — the compiler can optimize them more aggressively and they signal intent to readers."
```

### `[[muted_hint_domain]]`

An IDE inference domain from [`.claude/rules/inference.md`](../../../.claude/rules/inference.md). Populated in M1; consumer (LSP) wired in v0.2-M2.

| Field | Type | Required | Description |
|---|---|---|---|
| `domain` | string | yes | Domain name matching the inference.md table row |
| `placement_category` | string | yes | `"Addition"` \| `"Replacement"` \| `"Informational"` |
| `description` | string | yes | One-sentence description of what the domain infers |
| `example_source` | string | yes | Example Yinz source code triggering this hint |
| `example_hint_rendered` | string | yes | Exact text the IDE renders inline |

**Example**:
```toml
[[muted_hint_domain]]
domain = "variable_type"
placement_category = "Addition"
description = "Infers the type of a variable from its initializer expression"
example_source = "let x = 42"
example_hint_rendered = ": Int (from 42)"
```

### `[[lint_rule]]`

A Tier 3 lint rule per [`docs/internal/implementation/IMP-linting.md`](IMP-linting.md). Built in v0.3-M4 Phase 4 (first carriers: `cross-thread-fields-not-padded`, `prefer-yielding-sleep`); v0.3-M5's `array-using-soa-layout` reuses this kind unchanged.

The rule's teaching text, severity, and LSP code all have exactly ONE home — the entry. The compiler firing site calls `ynz_registry::lint_rule_diagnostic_parts(name, vars)` (or typeck's `ynz_typeck::lints::lint_diagnostic`, which also builds the `Diagnostic` with the `LintRule` kind) to render the templates with per-site `{placeholder}` vars. The rule `name` becomes the LSP `Diagnostic.code` (via `DiagnosticKind::LintRule`), so editors can group/dismiss by rule; `lsp_lint_rule_hover_for(name)` renders a rule-level hover. **Adding a rule = one TOML entry + one firing site — zero mechanism edits.**

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Kebab-case rule id (e.g. `"prefer-yielding-sleep"`) — the LSP `Diagnostic.code` |
| `severity` | string | yes | `"suggestion"` (Tier 3 default, dismissable) \| `"warning"`. **`"error"` is rejected by `build.rs`** — a lint is a teaching surface, never a gate |
| `description` | string | yes | One-sentence description (LSP hover/documentation surface) |
| `what_template` | string | yes | Parameterized WHAT (`{placeholder}` substitution, same grammar as `diagnostic_template`) |
| `what_instead_template` | string | yes | Parameterized WHAT-INSTEAD |
| `why_template` | string | yes | Parameterized WHY — contextual, per Golden Rule 3 |
| `since` | string | yes | Milestone tag (e.g. `"v0.3-M4"`) |
| `design_doc` | string | yes | Repo-relative path of the design doc that locked the rule |

**Example**:
```toml
[[lint_rule]]
name = "prefer-yielding-sleep"
severity = "suggestion"
description = "Suggests the yielding `wait sleep(ms)` over `sleepBlocking(ms)` in programs that have a scheduler — blocking sleep holds an OS thread idle."
what_template = "`sleepBlocking` parks the current thread — it sits idle and runs nothing else until the time is up."
what_instead_template = "Write `wait sleep({ms})` — your function pauses just the same, but the thread stays free to run other tasks."
why_template = "This program runs on a scheduler that can run other tasks while one waits."
since = "v0.3-M4"
design_doc = "docs/internal/implementation/IMP-no-function-coloring.md"
```

---

## Deferred-Feature Catalog — Phase 5b Population Targets

Every locked deferred feature that a user could try to use today gets a `[[deferred_language_feature]]` (language) or `[[deferred_tooling_feature]]` (CLI flag, tooling) entry, so the compiler answers it with a teaching error naming what to do instead. The entry's `design_doc` field points at the IMP or SCRATCH doc holding the design. Features that are compiler-internal (auto-SoA, the string pointer-and-length layout) or already shipped as keywords (`wait`, `background`, `errors`) get no entry. The `[[deferred_stdlib_api]]` kind is reserved and holds no entries. To see the current set, read `registry/features.toml`.

---

## Consumer API Contract

The `ynz-registry` crate exposes typed accessor functions following the project's `*Table` convention. These are the surfaces consumers import:

```rust
// All keywords — for lexer sync test + IDE autocomplete
pub fn keywords() -> impl Iterator<Item = &'static KeywordEntry>;

// All banned declaration keywords — for lexer sync test + error rendering
pub fn banned_declaration_keywords() -> impl Iterator<Item = &'static BannedDeclarationKeywordEntry>;

// Banned jargon lookup by word
pub fn banned_jargon_lookup(word: &str) -> Option<&'static BannedJargonEntry>;

// All banned jargon — for jargon_audit.rs iteration
pub fn banned_jargon() -> impl Iterator<Item = &'static BannedJargonEntry>;

// Primitive intrinsic lookup by kind + name + receiver_type
pub fn primitive_intrinsic_lookup(kind: IntrinsicKind, name: &str, receiver_type: Option<&str>) -> impl Iterator<Item = &'static PrimitiveIntrinsicEntry>;

// Type-attached constant lookup
pub fn type_attached_constant(type_name: &str, const_name: &str) -> Option<&'static TypeAttachedConstantEntry>;

// Deferred language feature lookup by name (for lexer error rendering)
pub fn deferred_language_feature(name: &str) -> Option<&'static DeferredLanguageFeatureEntry>;

// All deferred features — for bidirectional consistency test
pub fn deferred_language_features() -> impl Iterator<Item = &'static DeferredLanguageFeatureEntry>;

// Muted hint domain lookup
pub fn muted_hint_domain(domain: &str) -> Option<&'static MutedHintDomainEntry>;
pub fn muted_hint_domains() -> impl Iterator<Item = &'static MutedHintDomainEntry>;

// Lint rules (v0.3-M4): lookup + iteration, the generic firing-site renderer
// (WHAT/WHAT-INSTEAD/WHY with {placeholder} vars), and the LSP hover keyed by
// the rule name (which is the published Diagnostic.code).
pub fn lint_rule_lookup(name: &str) -> Option<&'static LintRuleEntry>;
pub fn lint_rules() -> impl Iterator<Item = &'static LintRuleEntry>;
pub fn lint_rule_diagnostic_parts(rule_name: &str, vars: &HashMap<&str, &str>) -> Option<(String, String, String)>;
pub fn lsp_lint_rule_hover_for(rule_name: &str) -> Option<String>;
```

The generated code (`OUT_DIR/registry.rs`) is `const` arrays with `&'static str` values — zero runtime allocation, O(1) or O(N) linear scan over small constants (same as pre-migration `match` statements).

---

## Carve-Outs — When a Parallel Rust Table Is Allowed

A diff check flags new `pub const`/`pub static` string arrays in `crates/ynz-{diagnostics,typeck,parser}/src/` without a CARVE-OUT comment. The following cases are allowed WITHOUT going through the registry:

### 1. Test-only intrinsics (`PrimitiveIntrinsicTable::with_test_intrinsic`)

`crates/ynz-typeck/src/intrinsics.rs` has a `#[cfg(test)]` method to inject test-only free functions. These are not user-facing features and should NOT be in the registry (the registry is for user-facing surface). The `#[cfg(test)]` attribute marks them unambiguously.

**Required annotation**: `#[cfg(test)]` on the definition. No additional comment needed — the attribute itself signals "test-only, not a registry entry."

### 2. Perf-critical hot-path lookups with proven overhead

If a lookup table is hit on every token/expression/statement in a hot loop AND profiling proves the `registry::` accessor is measurably slower (>5% compilation-time regression on a real workload), a local copy is allowed WITH:
- A `// CARVE-OUT: SSOT registry/features.toml#<entry_kind>.<name>` comment on the first line of the definition.
- A corresponding registry entry still exists (the registry remains the source of truth; the carve-out is a performance mirror).
- A consistency test in `crates/ynz-registry/tests/` asserting the carve-out list is a subset of the registry.

No carve-out of this kind exists today: every lookup is a compile-time constant over a small list.

### 3. Compiler-internal implementation details with no user-facing surface

If a list exists ONLY for internal compiler bookkeeping (no error messages reference it, no IDE uses it, no user-visible behavior depends on it), a hand-written Rust constant is fine. The list does NOT belong in the registry.

**Required annotation**: `// CARVE-OUT: internal-only, not user-facing` on the definition.

---

## When to Add a Registry Entry

Add an entry when ALL of the following are true:
1. The item is **user-facing** in at least one of these ways: appears in an error message, drives IDE autocomplete, drives IDE muted hints, is documented in `spec/`.
2. The item could **drift** if maintained in multiple places — it appears in two or more places in the compiler OR it will appear in a second place when the LSP is wired (v0.2-M2).
3. The item is **stable enough** to warrant TOML — it won't change every PR.

## When NOT to Add a Registry Entry

Do NOT add a registry entry when:
- The item is compiler-internal only (no user-facing surface). Use a Rust constant in the relevant crate.
- The item is test-only. Use `#[cfg(test)]` + a local list.
- The item is a one-off (appears in exactly one place and has no LSP consumer). Add it to the TOML when a second consumer is known.

---

## Self-Hosting Migration Plan

Yinz is planned to self-host (compiler written in Yinz) at v2+. The registry's implementation is deliberately split to survive this transition:

**Source of truth** ([`registry/features.toml`](../../../registry/features.toml)): pure TOML — language-agnostic. Survives unchanged through the Rust→Yinz transition. No migration required.

**Code generator** (`crates/ynz-registry/build.rs`): currently Rust. At self-hosting time, this becomes `build.ynz` with the same semantics. The TOML parsing uses Yinz's stdlib v0.22+ package-manager TOML parser (the same one used for `yinz.toml`). The generated Rust output becomes generated Yinz.

**Consumers** (e.g., `crates/ynz-diagnostics/src/banned_jargon.rs`): currently Rust adapters. At self-hosting time, they become Yinz modules.

The transition is incremental: TOML stays, build script is rewritten once, consumer adapters are rewritten per-crate. No "big bang" migration.

---

## Build Mechanics

`build.rs` in `crates/ynz-registry/`:
1. Reads [`registry/features.toml`](../../../registry/features.toml) (path relative to crate root: `../../registry/features.toml`).
2. Validates every entry against the schema (panics with: `registry/features.toml: [[<kind>]] entry '<name>': missing required field '<field>'`).
3. Validates no duplicate `name` within a kind (panics with both conflicting line numbers if TOML library provides them).
4. Emits `OUT_DIR/registry.rs` — a valid Rust source file containing `pub static` arrays of the typed structs.
5. Declares `cargo:rerun-if-changed=../../registry/features.toml` so incremental builds skip the script when the TOML hasn't changed.

**Incremental build guarantee**: touching any file other than [`registry/features.toml`](../../../registry/features.toml) does not re-run `build.rs`.
