# Linting & Build — Design Decisions

User spec: `spec/linting.md`, `spec/tooling.md`. Full compiler design: `design/compiler.md`. Teaching mission alignment: `design/teaching-mission.md`.

---

## The Compiler IS the Linter

Yinz does NOT ship a separate linter tool. The compiler's third diagnostic tier (suggestions) IS the linter. There is no separate `ynz lint` command — `ynz build` and the LSP both emit suggestion-tier diagnostics during normal operation.

**Why this is the right design for Yinz:**
- One source of truth for code quality. No "lint passes on my machine, fails in CI" because the lint IS the compile.
- Zero configuration to get high-quality feedback — a v0.4+ Yinz project gets pedagogical suggestions out of the box.
- Suggestions can use the full compiler IR (type info, ownership analysis, control flow) for accuracy that external tools can't match.
- Built-in rules and custom plugin rules share the same single-pass walker — no performance penalty for adding rules.

**Customization:** The `[lint]` config in `yinz.toml` controls severity, disables specific rules, and loads plugin packages. See below for the full config shape.

**Layered opt-out:**
- Disable a specific built-in rule: `[lint.rules] unused-imports = "disabled"`
- Disable all built-in rules: `[lint] enabled = false` — plugin rules still run if installed
- Full replacement: `[lint] enabled = false` + your own `[lint.plugins]` — you own the rule set entirely

---

## Three-Tier Linting

Compiler catches problems at three levels: **errors** (won't compile), **warnings** (compiles, flag the problem), **suggestions** (informational, IDE only by default).

**Configurable severity per rule** in `yinz.toml` (v1.x):

```toml
[lint]
enabled = true                  # set to false to disable all built-in rules

[lint.rules]
max-function-length = { severity = "warning", max-lines = 75 }
use-int-for-whole-numbers = { severity = "suggestion" }
unused-imports = { severity = "error" }     # promote to blocking (or "disabled" to turn off)

[lint.custom-rules.no-print-in-prod]        # simple pattern rules — no crate required
pattern = "print("
scope = "src/prod/**"
message = "Use log.info() instead in production code"
severity = "error"

[lint.plugins]                              # full AST-level plugin packages
no-internal-imports = { package = "ynz-lint-boundaries", version = "1.0" }
```

**Philosophy**: Catch real bugs and enforce code quality. Don't police style preferences. Every rule prevents an actual problem or teaches a pattern. The developer should feel helped, not harassed.

**Why suggestions are IDE-only by default**: Suggestions are the most subjective tier. Showing them in terminal output during CI would be too noisy and push developers to disable the whole system. IDE-only keeps them visible during development without friction in automated pipelines.

---

## Single-Walker Architecture — Performance Guarantee

All lint rules — built-in and plugin — share one AST walk. The walker never traverses the tree more than once per lint pass, regardless of how many rules are registered.

### How it works

Each rule declares upfront which node kinds it cares about. At lint startup, the compiler builds a dispatch table once:

```rust
// built at startup, not per-file
let mut dispatch: HashMap<NodeKind, Vec<&dyn LintPass>> = HashMap::new();
for pass in all_passes {   // built-ins + pattern rules + plugins — same list
    for kind in pass.interested_in() {
        dispatch[kind].push(pass);
    }
}
```

The walk is then O(1) per node regardless of rule count:

```rust
for node in ast.walk() {
    if let Some(passes) = dispatch.get(&node.kind()) {
        for pass in passes {
            pass.visit(&ctx, &node);
        }
    }
}
```

50 rules registered = same number of AST traversals as 1 rule. This is the oxlint approach and the primary reason it's 50–100× faster than ESLint at scale.

### Built-in rules have no special privileges

Built-in rules are `LintPass` implementors in the same dispatch table as plugin rules. They register via `interested_in()`, get called via `visit()`, and emit diagnostics via the same `LintContext`. There is no separate fast path or internal traversal for built-ins. This has two consequences:

1. The plugin API is automatically feature-complete — anything a built-in rule can check, a plugin can check.
2. Built-in rules can be tested the same way as plugin rules.

### Pattern-based custom rules

`[lint.custom-rules]` entries (simple text patterns in `yinz.toml`) are compiled at build time into `LintPass` text-matchers and inserted into the same dispatch table. They go through the same walker as everything else — no second traversal mechanism.

---

## Plugin API — LintPass

The public API for lint plugins. Versioned separately from compiler internals.

### The LintPass trait

```rust
pub trait LintPass: Send + Sync {
    fn interested_in(&self) -> Vec<NodeKind>;
    fn visit(&self, ctx: &LintContext, node: &LintNode);
}
```

Plugins implement `LintPass`, register interest upfront, and get called for matching nodes. The `LintContext` exposes type info, ownership info, and the diagnostic emitter. The `LintNode` types are concept-named public API — not internal compiler struct names.

### LintNode — public API vs internal AST

The compiler's internal AST can change freely. `LintNode` is the stable public representation, translated from internal nodes at call time. This decouples plugin authors from compiler internals.

```rust
// plugin author sees this — stable, concept-named
pub enum LintNode<'a> {
    Call(CallNode<'a>),
    LetBinding(LetBindingNode<'a>),
    FunctionDef(FunctionDefNode<'a>),
    // ...
}

pub struct CallNode<'a> {
    pub callee: &'a str,
    pub args: &'a [LintNode<'a>],
    pub span: Span,
    // ...type info, ownership info exposed via LintContext
}
```

Internal struct renamed → update the translation. Plugin author's code: unaffected.

### Diagnostic format — plugins must follow it too

Plugin diagnostics go through the same `LintContext::report()` and must follow the WHAT / WHAT-INSTEAD / WHY three-part format. The `Diagnostic` constructor enforces this — a diagnostic missing any part is a compile error in the plugin crate.

### Versioning

**Pre-v1.0**: `LintNode` API has no stability guarantee. Plugin authors pin to a specific Yinz version. Breaking changes are documented in the changelog.

**Post-v1.0**: `LintNode` API is stable. Internal AST changes must preserve the translation layer. A change that breaks the translation is a semver-major bump.

---

## Initial Rule Set (v0.4 launch)

The full curated initial set, organized by tier.

### Tier 1 — Errors (block compile)

These enforce Yinz language fundamentals. Violating them creates ambiguity that the language can't handle.

| Rule | Catches |
|------|---------|
| `type-naming` | Types must start with capital letter (Golden Rule 13). `type player` is an error. |
| `variable-naming` | Variables/functions/modules must start with lowercase. `function FetchUser` is an error. |
| `duplicate-import-name` | Same name imported twice without alias. Forces explicit disambiguation. |
| `out-of-bounds-literal` | `fixed<T>` of size N, accessed with literal index ≥ N. |
| `out-of-precision-literal` | `number<N>` with N > 4096 (the cap from `design/numeric-types.md`). |

### Tier 2 — Warnings (compile succeeds, message visible)

Real problems that don't break immediately but indicate bugs.

| Rule | Catches |
|------|---------|
| `unused-imports` | `import { foo } from "..."` where `foo` isn't used |
| `unused-variables` | `let x = ...` where `x` is never read |
| `unused-private-functions` | Functions defined but never called within the file |
| `unused-private-types` | Types defined but never referenced |
| `unused-exports` | Exported but never imported anywhere in the project |
| `unused-function-parameters` | Parameter declared but never used in function body |
| `dead-code` | Compiler-provable unreachable paths |
| `unreachable-after-return` | Code after a definite `return` |
| `shadowed-variables` | Re-declaring a name in nested scope |
| `mutable-when-const-suffices` | `let x = ...` where `x` is never reassigned/mutated/lent — compiler already treats the binding as read-only for optimization purposes; lint suggests writing `const` explicitly for source clarity. Same hybrid model as `prefer-fixed-when-immutable` |
| `unused-follows` | Type declares `follows X` but uses no contract methods |
| `assignment-in-condition` | `if (x = 5)` — almost always a typo for `==` |
| `empty-error-handling` | Catching `.failed()` and doing nothing — silent failure |
| `identical-branches` | Both `if` arms return the same value — condition has no effect |
| `constant-condition` | `if (debug)` where `debug` is a known-`false` const — code never runs |
| `unnecessary-wait` | `wait` on a call whose result feeds the next operation anyway |
| `string-concat-in-loop` | `result = result + chunk` (or `result += chunk`) inside a `for`/`while` loop — O(n²) allocations. Suggests `strings.builder { ... }`. Per `lockin-cpu-bigo.md` Finding #2: Go benchmark shows 91× perf gap (1,994,038 ns/op with 1000 allocations vs 21,918 ns/op with 15 allocations). Compiler diagnostic: WHAT — "each `+` creates a new string, copying all previous bytes"; WHAT-INSTEAD — "use `strings.builder { ... }` to accumulate into a single buffer"; WHY — "`result` has N bytes at iteration N, so this loop copies 1+2+...+N = O(n²) total bytes." |

### Tier 3 — Suggestions (IDE-visible hints, lowest urgency)

The "compiler-as-teacher" tier. About code quality, performance, and pedagogy, not strict correctness.

| Rule | Suggests |
|------|----------|
| `use-int-for-whole-numbers` | `let count: number = 0` for a counter — suggest `int` (faster) |
| `avoid-float-for-finance` | `let price: float = 19.99` — suggest `number` (exact decimal) |
| `use-shape-for-static-keys` | `map<string, V>` literal with all-string-literal keys — suggest a `shape` |
| `prefer-fixed-when-immutable` | `array<T>` that never calls `.add()` or `.remove()` — compiler auto-promotes to `fixed<T>` codegen silently; lint suggests writing `fixed<T>` explicitly for source clarity. Hybrid model — see `design/collections.md` "Auto-promotion" section |
| `max-function-length` | Functions over 50 lines (default, configurable) — suggest splitting |
| `max-nesting-depth` | More than 4 levels of nesting (configurable) — suggest restructuring |
| `long-parameter-list` | 5+ params (configurable) — suggest options object pattern |
| `single-letter-variables` | `let x = ...` outside `for (i in ...)` — suggest descriptive name |
| `magic-numbers` | Numeric literals other than 0, 1, -1 — suggest named constant |
| `duplicate-code` | Two functions/blocks with identical logic — suggest extraction |
| `debug-prints-in-production` | `print()` calls in non-test code (configurable scope) — suggest `log` module |
| `large-background-copy` | `background fn(largeData)` where `data` is reused after — `.give` or restructure |

**Module-specific rules ship with each module's version** (v0.5 file system rules, v0.6 math rules, etc.). They're not in the v0.4 launch set; they expand the suggestion tier as the stdlib grows.

---

## Three-Part Diagnostic Format — Required

Every lint diagnostic — errors, warnings, suggestions — follows the WHAT / WHAT-INSTEAD / WHY format defined in `design/teaching-mission.md`:

```
[severity tier]: [WHAT — concise statement of the issue]

  [WHAT TO DO INSTEAD — corrected code, ready to copy]

  [WHY — performance, correctness, idiomatic Yinz, or convention]
```

**Example (suggestion tier):**

```
SUGGESTION: This map's keys are all compile-time string literals.

  Consider a type instead:
    shape Scores { alice: number, bob: number, charlie: number }
    let scores: Scores = { alice: 90, bob: 85, charlie: 78 }

  Why: Type field access compiles to a direct memory offset (~1 instruction).
       Map key access requires a hash lookup (~10-50 instructions). For
       static keys, types are ~10x faster AND give you dot-access syntax
       with autocomplete.
```

A diagnostic missing any of the three parts is not Yinz-compliant. This is enforced through code review when new rules are added.

---

## Compile Speed — Design Principle

Runtime performance is never sacrificed for compile speed. But compile speed is aggressively optimized through incremental compilation, caching, and smart dependency tracking.

**Not a golden rule**: The 13 golden rules describe the language from the user's perspective. Compile speed is a compiler implementation goal. Full design: `design/compiler.md`.

**Debug vs release builds**: `ynz build` = minimal LLVM optimization, fast compile. `ynz build --release` = full LLVM optimization, slow compile, maximum runtime speed. Development uses debug. Production uses release. These are different binaries — never deploy a debug build.

---

## Rule ID Stability

Every rule has a stable, addressable ID (`type-naming`, `use-int-for-whole-numbers`, etc.) so v1.x config files can reference them. Rule IDs never change once shipped — renaming a rule means shipping the new ID AND keeping the old as a deprecated alias.

Rule IDs are also how custom-rules.md cross-references work and how the IDE shows users which rule produced a diagnostic.
