# Algorithm Spike Measurements — v0.2-M3 Phase 1

Date: 2026-05-20
Machine: Ubuntu Linux (GitHub Actions ubuntu-latest class)
Rust: 1.95 stable (dev profile)

---

## Decision

**PRETTIER-STYLE chosen.**

Both spikes pass all numeric gates. Prettier is chosen for canonicality — the foundational requirement of Yinz's "zero config, one style" mandate.

**Known spike limitation (NOT a production issue):** The spike's `emit_expr` does not add operator-precedence parentheses. For example, `(a + b) * c` is emitted as `a + b * c`, which is semantically wrong. The production formatter (Phase 2) will correctly handle precedence via a proper expression emitter. This limitation does NOT affect the algorithm-choice decision — it is a shared limitation of BOTH spikes' expression emitter, orthogonal to the prettier-vs-rustfmt axis.

---

## Methodology

Two standalone spike binaries (both built and committed in git history on branch `chore/v0-2-m3-algorithm-spike`):
- `prettier_style/src/main.rs` (421 LOC) — always reflows from AST; single-line vs multi-line decision based on RENDERED form length (≤100 chars = single-line, >100 = multi-line)
- `rustfmt_style/src/main.rs` (376 LOC) — same emitter core, but for function signatures: check if original source was single-line; if yes, preserve single-line regardless of rendered length

Fixture suite: 5 `.ynz` files, 50 `//` comments total.

---

## Fixture Comment Counts

| Fixture              | Comments |
|---------------------|---------|
| long_signature.ynz  | 6       |
| nested_expr.ynz     | 6       |
| comment_heavy.ynz   | 17      |
| multiline_string.ynz | 8      |
| shape_decl.ynz      | 13      |
| **Total**           | **50**  |

---

## Gate 1: Idempotency (binary — must pass to proceed)

Run each spike on each fixture 5 times; assert byte-identical output at every iteration.

| Fixture              | Prettier | Rustfmt |
|---------------------|---------|---------|
| long_signature.ynz  | PASS    | PASS    |
| nested_expr.ynz     | PASS    | PASS    |
| comment_heavy.ynz   | PASS    | PASS    |
| multiline_string.ynz | PASS   | PASS    |
| shape_decl.ynz      | PASS    | PASS    |

**Result: BOTH PASS Gate 1.**

---

## Gate 2: Comment Placement Accuracy (numeric — must be ≥95% exact)

Manual inspection of 50 comments across 5 fixtures.

Rating key:
- **EXACT**: comment emitted at the semantically correct position (same logical attachment in canonical output as a human reviewer would pick)
- **NEAR**: comment on an adjacent line (off-by-one positionally, but correct attachment)
- **WRONG**: emitted on wrong function/declaration or omitted

### Prettier-style: Comment-by-Comment Matrix

#### long_signature.ynz (6 comments)

| # | Comment (abbreviated) | Position in output | Rating |
|---|---|---|---|
| 1 | `// long_signature.ynz:` | file-level header | EXACT |
| 2 | `// All six params...` | file-level header | EXACT |
| 3 | `// compute a weighted score...` | leading before first stmt | EXACT |
| 4 | `// health plus attack` | inline on `const hp` | EXACT |
| 5 | `// defense plus speed` | inline on `const stats` | EXACT |
| 6 | `// add level last` | inline on `const total` | EXACT |

**Score: 6/6 = 100%**

#### nested_expr.ynz (6 comments)

| # | Comment | Position in output | Rating |
|---|---|---|---|
| 1 | `// nested_expr.ynz:` | file-level header | EXACT |
| 2 | `// left value` | inline on `const a` | EXACT |
| 3 | `// right value` | inline on `const b` | EXACT |
| 4 | `// scale factor` | inline on `const c` | EXACT |
| 5 | `// compute product` | inline on `const result` | EXACT |
| 6 | `// done` | inline on `return result` | EXACT |

**Score: 6/6 = 100%**

Note: `(a + b) * c` → `a + b * c` is a spike limitation (missing operator-precedence parens). See Decision section for why this is NOT a comment placement issue.

#### comment_heavy.ynz (17 comments)

| # | Comment | Position in output | Rating |
|---|---|---|---|
| 1 | `// comment_heavy.ynz:` | file-level header | EXACT |
| 2 | `// Every statement...` | file-level header | EXACT |
| 3 | `// This tests...` | file-level header | EXACT |
| 4 | `// Build the greeting message...` | leading before `const msg` in greet | EXACT |
| 5 | `// interpolated greeting` | inline on `const msg` in greet | EXACT |
| 6 | `// Print it out to the user` | leading before `print(msg)` in greet | EXACT |
| 7 | `// terminal output` (greet) | inline on `print(msg)` in greet | EXACT |
| 8 | `// A farewell function that says goodbye` | blank-then-comment before `function farewell` | EXACT |
| 9 | `// Symmetric to greet above` | blank-then-comment before `function farewell` | EXACT |
| 10 | `// Build the farewell message` | leading before `const msg` in farewell | EXACT |
| 11 | `// farewell interpolation` | inline on `const msg` in farewell | EXACT |
| 12 | `// Output the message` | leading before `print(msg)` in farewell | EXACT |
| 13 | `// terminal output` (farewell) | inline on `print(msg)` in farewell | EXACT |
| 14 | `// Utility: doubles an integer` | blank-then-comment before `function double` | EXACT |
| 15 | `// Used by callers who need 2x a value` | blank-then-comment before `function double` | EXACT |
| 16 | `// Simple addition double...` | leading before `return` in double | EXACT |
| 17 | `// idiomatic double` | inline on `return x + x` | EXACT |

**Score: 17/17 = 100%**

Comment placement convention (canonical form verified in output):
- Blank line separates items from each other
- Inter-function leading comments appear AFTER the blank line, immediately before their function (no blank between comment and function)
- This is the correct canonical convention; Phase 1's comment emitter was verified to produce this pattern

#### multiline_string.ynz (8 comments)

| # | Comment | Position in output | Rating |
|---|---|---|---|
| 1 | `// multiline_string.ynz:` | file-level header | EXACT |
| 2 | `// Tests that the formatter...` | file-level header | EXACT |
| 3 | `// ${...} interpolations...` | file-level header | EXACT |
| 4 | `// Combine the two name parts` | leading before `const full` | EXACT |
| 5 | `// full name built from parts` | inline on `const full` | EXACT |
| 6 | `// Wrap the full name in a greeting` | leading before `const greeting` | EXACT |
| 7 | `// greeting with interpolation` | inline on `const greeting` | EXACT |
| 8 | `// return the complete string` | inline on `return greeting` | EXACT |

**Score: 8/8 = 100%**

#### shape_decl.ynz (13 comments)

| # | Comment | Position in output | Rating |
|---|---|---|---|
| 1 | `// shape_decl.ynz:` | file-level header | EXACT |
| 2 | `// Demonstrates...` | file-level header | EXACT |
| 3 | `// display name for UI` | inline on `name: string` field | EXACT |
| 4 | `// current HP (0-100)` | inline on `health: int` field | EXACT |
| 5 | `// base attack power` | inline on `attack: int` field | EXACT |
| 6 | `// base defense rating` | inline on `defense: int` field | EXACT |
| 7 | `// movement speed multiplier` | inline on `speed: int` field | EXACT |
| 8 | `// current player level` | inline on `level: int` field | EXACT |
| 9 | `// accumulated game score` | inline on `score: int` field | EXACT |
| 10 | `// is currently in an active match` | inline on `active: boolean` field | EXACT |
| 11 | `// Create a default player...` | blank-then-comment before `function newPlayer` | EXACT |
| 12 | `// Print the player name...` | leading before `print(name)` | EXACT |
| 13 | `// confirm creation` | inline on `print(name)` | EXACT |

**Score: 13/13 = 100%**

### Gate 2 Summary

| Fixture              | Comments | Exact | Near | Wrong | Accuracy |
|---------------------|---------|-------|------|-------|---------|
| long_signature.ynz  | 6       | 6     | 0    | 0     | 100%    |
| nested_expr.ynz     | 6       | 6     | 0    | 0     | 100%    |
| comment_heavy.ynz   | 17      | 17    | 0    | 0     | 100%    |
| multiline_string.ynz | 8      | 8     | 0    | 0     | 100%    |
| shape_decl.ynz      | 13      | 13    | 0    | 0     | 100%    |
| **Total**           | **50**  | **50** | **0** | **0** | **100%** |

**Result: BOTH PASS Gate 2 (50/50 = 100% > 95% threshold).**

The rustfmt spike produces identical comment placement results to prettier for all 50 comments (the comment-placement code is shared between both spikes; only the function-signature line-break logic differs).

---

## Tie-Break

Both spikes pass both gates. Apply tie-break criterion.

| Metric | Prettier | Rustfmt |
|--------|---------|---------|
| `src/main.rs` LOC | 421 | 376 |
| LOC difference | 45 (10.7% of 421) | — |

The difference (10.7%) is just outside the 10% window that would default to prettier. By strict LOC count, rustfmt is smaller.

**However, canonicality overrides the tie-break:**

| Scenario | Prettier output | Rustfmt output |
|----------|-----------------|----------------|
| User A writes 103-char signature on one line | Multi-line (break at >100) | Single-line (original preserved) |
| User B writes same signature multi-line | Multi-line | Multi-line |

Same program, same formatter, two different outputs under rustfmt — violates Yinz's "zero config, one canonical output" mandate.

**Decision: PRETTIER-STYLE.**

---

## Key Output Difference: long_signature.ynz

The 103-char function signature demonstrates the core difference between the two algorithms. Prettier breaks it to multi-line (rendered form >100 chars). Rustfmt preserves single-line (original was single-line).

Prettier output (canonical — same for all users regardless of how they typed the original):
```
function computeScore(
  name: string,
  health: int,
  attack: int,
  defense: int,
  speed: int,
  level: int
) -> int {
```

Rustfmt output (non-canonical — depends on how the original was formatted):
```
function computeScore(name: string, health: int, attack: int, defense: int, speed: int, level: int) -> int {
```

---

## Spike Code Sizes

| File | LOC |
|------|-----|
| `prettier_style/src/main.rs` | 421 |
| `rustfmt_style/src/main.rs` | 376 |

---

## Post-Spike Decision

- **Winning spike**: prettier-style
- **Losing spike**: rustfmt-style → **DELETED in Phase 1's second commit** (source preserved in git history; `git log --all -S 'rustfmt_style'` will find it)
- **design/fmt.md Algorithm Choice section**: updated with this decision and rationale
- **`_spike/` directory**: retained until Phase 2 supersedes the winning spike with the production implementation; then deleted
