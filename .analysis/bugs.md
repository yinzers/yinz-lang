# Bug Analysis Report — Unused-Import False Positives + LSP Teaching Surface Hunt

**Analyzed**: 2026-05-21
**Files Checked**: 6 (`queries.rs`, `check.rs`, `inlay_hint_passes.rs`, `hover.rs`, `inlay_hint.rs`, `shapes.rs`)
**Critical Bugs Found**: 1 (Bug 1) + 6 critical/high in Bug 2 sweep
**Scope**: Bug 1 = `Timeframe` unused-import false positive. Bug 2 = LSP teaching surfaces (diagnostics, hovers, inlay hints).

---

# BUG 1 — `Timeframe` reported "imported but never used" while in active use

## Symptom

Source compiles cleanly when `Timeframe` is used as `Timeframe.fiveMinute` (options-variant access) inside a function body or struct literal, BUT the compiler emits:

```
warning: `Timeframe` is imported but never used.
  hint: Remove `Timeframe` from the import list, or use it somewhere in this file.
  why: Unused imports add noise — every import signals to readers that this file depends on that symbol.
```

The user knows the import IS used and learns to ignore the warning — Tier 1 teaching-surface regression.

## Reproduction (minimal Yinz, 6 lines)

```yinz
// File: src/timeframes.ynz
export options Timeframe { fiveMinute: `Five Minute`, daily: `Daily` }

// File: src/entrypoint.ynz
import { Timeframe } from "timeframes"
function entrypoint() -> nothing {
  print(Timeframe.fiveMinute.toString())
}
```

Compile this and `Timeframe` is incorrectly flagged as unused.

## Root Cause

**File**: `/workspaces/ynz/crates/ynz-typeck/src/check.rs:1051-1053`

```rust
} else if self.options_table.contains(type_name_str) {
    // M6: OptionsValue — `Status.active` where Status is an options type.
    self.check_options_value(type_name_str, field, field_span)
} else {
```

The `Expr::FieldAccess` handler dispatches to `check_options_value` (defined at `check.rs:3574`) when the receiver identifier names an options type. **`check_options_value` validates the variant exists but never calls `self.referenced_names.insert(type_name.to_string())`** (verified at `check.rs:3574-3590`).

The type-annotation path at `check.rs:2240-2243` does insert the options name correctly:

```rust
AstType::Named(n, _) if self.options_table.contains(n) => {
    self.referenced_names.insert(n.clone());
    Type::Options { name: n.clone() }
}
```

But `Timeframe.fiveMinute` never traverses `ast_type_to_type`. It goes through `Expr::FieldAccess` → `check_options_value`, which has the missing insert.

**Why no example caught this**: every existing test that uses `Timeframe` either (a) does NOT use a named import (uses inline definition), or (b) ALSO uses `Timeframe` as a type annotation in the same file (which masks the bug because the type-annotation path tracks it).

## Fix Recommendation

In `/workspaces/ynz/crates/ynz-typeck/src/check.rs:3574`, add the insert:

```rust
fn check_options_value(&mut self, type_name: &str, variant: &str, span: &SourceSpan) -> Type {
    let entry = self.options_table.get(type_name).unwrap();
    self.referenced_names.insert(type_name.to_string());   // ← ADD
    if entry.variants.contains(&variant.to_string()) {
        Type::Options { name: type_name.to_string() }
    } else {
        // … existing diagnostic …
    }
}
```

Add a regression test in `crates/ynz-typeck/tests/check.rs`:

```rust
#[test]
fn unused_import_not_flagged_when_options_variant_accessed() {
    let src = r#"
        import { Timeframe } from "timeframes"
        function entrypoint() -> nothing {
            print(Timeframe.fiveMinute.toString())
        }
    "#;
    // … typecheck and assert no UnusedImport diagnostic for "Timeframe"
}
```

Note: this is the FIRST of several related false-positive paths — see Bug 2.1, 2.2, 2.3 below for additional missing inserts that share the same fix pattern.

---

# BUG 2 — Broad LSP Teaching-Surface Hunt

## Severity Legend

- **CRITICAL**: teaching surface is silently wrong (warning emitted for valid code, or hint fires on a binding that IS mutated) — users learn to ignore the diagnostic
- **HIGH**: false-positive emits noise users learn to ignore (close to critical but narrower trigger)
- **MEDIUM**: false-negative — should teach but doesn't
- **LOW**: stale, edge-case, or fragile but unlikely in practice

---

## CRITICAL

### Bug 2.1 — `is TypeName` and `if (x is TypeName)` don't track imported variant names

**File**: `/workspaces/ynz/crates/ynz-typeck/src/check.rs:3472-3510` (`check_is_arm_pattern`)
**File**: `/workspaces/ynz/crates/ynz-typeck/src/check.rs:3522-3565` (`check_is_expr`)
**Category**: False-positive diagnostic

Both functions validate `type_path.name` against the union's known variants but never insert it into `referenced_names`. So:

```yinz
import { Circle } from "shapes"
function area(s: Shape) -> int {
    if (s is Circle) { return 3 }
    return 0
}
```

This emits a spurious `Circle` is imported but never used warning whenever `Circle` is referenced only via `is`-narrowing.

**Fix**: Add `self.referenced_names.insert(type_path.name.clone());` near the top of both functions (after the empty-name guard at `check_is_expr:3529`).

---

### Bug 2.2 — `shape … extends Parent` and `shape … follows Contract` don't track imported names

**File**: `/workspaces/ynz/crates/ynz-typeck/src/check.rs:2502-2570` (`check_follows_contracts`)
**File**: `/workspaces/ynz/crates/ynz-typeck/src/check.rs:174` (`Item::ShapeDecl(_) => {}` — skipped entirely in check_module)
**Category**: False-positive diagnostic

`extends`/`follows` references are resolved during the signature pre-pass (`shapes.rs:collect_shapes`) without access to the `referenced_names` set; `check_module` then skips `Item::ShapeDecl` entirely. Result: imported types used solely as `extends Parent` or `follows Contract` are flagged as unused.

```yinz
import { Damageable } from "contracts"
shape Player follows Damageable { ... }
// → "Damageable is imported but never used."
```

**Fix**: In `check_module` (`check.rs:171-182`), iterate `Item::ShapeDecl` and call `self.referenced_names.insert(extends_name)` + each `follows_name`. Alternatively, do this insertion at the beginning of `check_follows_contracts` (already collects the contract names at `check.rs:2509`).

---

### Bug 2.3 — Shape field type annotations (`tf: Timeframe`) don't track imported names

**File**: `/workspaces/ynz/crates/ynz-typeck/src/check.rs:174` (skip)
**File**: `/workspaces/ynz/crates/ynz-typeck/src/shapes.rs` (field-type resolution happens here, pre-check, no `referenced_names` access)
**Category**: False-positive diagnostic

```yinz
import { Timeframe } from "timeframes"
shape Bar { tf: Timeframe, value: int }
// → "Timeframe is imported but never used."  (false positive)
```

This is documented as a known shape — the M8 plan (`m8-typeck-cross-file-resolution.md:221`) called for tracking via this path, but the implementation only tracks references seen during the `check` pass (which excludes `ShapeDecl`).

**Fix**: In `check_module`, walk every `Item::ShapeDecl`'s field type annotations and call `self.ast_type_to_type(&field.ty)` for the side effect of `referenced_names` insertion. (Discard the returned `Type` — typeck already validated fields in `shapes.rs`.) The walk also needs `extends` parent name and every `follows` name (see Bug 2.2).

Same gap for **module-level `const` declarations** (`check.rs:180`) — `Item::ConstDecl(_) => {}` skips the body. `const DEFAULT: Status = Status.active` won't track `Status` at all.

---

### Bug 2.4 — `dynamic Contract` doesn't track imported contract name

**File**: `/workspaces/ynz/crates/ynz-typeck/src/check.rs:2294-2308` (`AstType::Dynamic` branch)
**Category**: False-positive diagnostic

```rust
AstType::Dynamic { contract, span } => {
    if self.shape_table.contains(contract) {
        Type::Dynamic { contract: contract.clone() }   // no insert
    } else { /* error */ }
}
```

```yinz
import { Damageable } from "contracts"
function attack(targets: array<dynamic Damageable>) -> nothing { ... }
// → "Damageable is imported but never used."
```

**Fix**: Add `self.referenced_names.insert(contract.clone());` inside the `if self.shape_table.contains(contract)` branch.

---

### Bug 2.5 — Generic shape names in `Container<T>` position don't track

**File**: `/workspaces/ynz/crates/ynz-typeck/src/check.rs:2379-2388` (`AstType::Generic` user-defined branch)
**Category**: False-positive diagnostic

```rust
_ => {
    if self.generic_shape_table.contains(name) {
        Type::Generic { name: name.clone(), args: resolved_args }   // no insert
    } else { Type::Error }
}
```

```yinz
import { Container } from "generics"
function wrap(x: int) -> Container<int> { ... }
// → "Container is imported but never used."
```

**Fix**: Add `self.referenced_names.insert(name.clone());` inside the `if self.generic_shape_table.contains(name)` branch.

---

## HIGH

### Bug 2.6 — `let_to_const_promotion_hints` fails to track mutations inside `else =>` arms (and `array_to_fixed_promotion_hints` has the same gap)

**File**: `/workspaces/ynz/crates/ynz-typeck/src/inlay_hint_passes.rs:142-147` (`collect_maybe_mutated_stmt`)
**Category**: Inlay-hint false positive (CRITICAL impact — hint fires on a binding that IS mutated)

```rust
Stmt::Match { scrutinee, arms, .. } => {
    collect_maybe_mutated_expr(scrutinee, out);
    for arm in arms { collect_maybe_mutated(&arm.body, out); }
    // ← `else_arm: Option<Block>` is NEVER visited
}
```

`Stmt::Match.else_arm: Option<Block>` (defined at `ynz-ast/src/nodes.rs:246`) is silently ignored. A `let` binding mutated inside an `else =>` catch-all arm will trigger the `let_to_const` hint suggesting "effectively const — never reassigned" even though the binding IS reassigned.

```yinz
function entrypoint() -> nothing {
    let count = 0
    if (someValue) {
        Status.active => print("active"),
        else => count = 99   // ← mutation NOT tracked
    }
    // IDE shows: "// effectively const — never reassigned" on `let count`
}
```

This is the canonical "hint fires on a binding that IS mutated through code the analysis doesn't track" example from the brief.

**Fix**: In every `Stmt::Match { arms, else_arm, .. }` walker — both in mutation collection and hint emission — also visit `else_arm.as_ref()`:

```rust
Stmt::Match { scrutinee, arms, else_arm, .. } => {
    collect_maybe_mutated_expr(scrutinee, out);
    for arm in arms { collect_maybe_mutated(&arm.body, out); }
    if let Some(eb) = else_arm { collect_maybe_mutated(eb, out); }
}
```

Apply the same fix to **all five** `collect_*_block` walkers in this file (lines 247-256, 250-254, 311-316, 402-407, 471-481, 521-531) — every one of them iterates `arms` but ignores `else_arm`.

---

### Bug 2.7 — Nested `FieldAssign` / `IndexAssign` targets don't mark root binding as mutated

**File**: `/workspaces/ynz/crates/ynz-typeck/src/inlay_hint_passes.rs:119-134`
**Category**: Inlay-hint false positive

```rust
Stmt::FieldAssign { target, value, .. } => {
    if let Expr::FieldAccess { receiver, .. } = target.as_ref() {
        if let Expr::Ident(name, _) = receiver.as_ref() {
            out.insert(name.clone());          // only handles ONE level of nesting
        }
    }
    ...
}
```

```yinz
let player = makePlayer()
player.address.street = "x"   // root receiver is `player.address` (FieldAccess), not Ident
// → `player` NOT marked mutated → let_to_const hint fires (wrong)
```

Same gap in `Stmt::IndexAssign` at line 129: `arr[0][1] = x` won't mark `arr`.

**Fix**: Walk down the chain to find the root identifier:

```rust
fn root_ident<'a>(mut e: &'a Expr) -> Option<&'a str> {
    loop {
        match e {
            Expr::Ident(name, _) => return Some(name),
            Expr::FieldAccess { receiver, .. } | Expr::IndexAccess { receiver, .. } => e = receiver,
            _ => return None,
        }
    }
}
```

Use it for both `FieldAssign.target` and `IndexAssign.receiver`.

---

### Bug 2.8 — Hover keyword lookup wins over user-defined identifiers with the same name

**File**: `/workspaces/ynz/crates/ynz-lsp/src/hover.rs:112-121`
**Category**: Hover content for wrong symbol

`share`, `lend`, `give`, `errors`, `wait`, `is`, `background` are CONTEXTUAL identifiers (not lexer tokens — verified via `parser.rs:1450-1458`). A user-defined variable, function, or shape named `share` is legal Yinz. But the hover handler tries registry first:

```rust
if let Some(content) = lsp_hover_for_token(&token_name) {
    return Some(Hover { contents: ... });    // wins unconditionally
}
// Typeck fallback only runs if registry returned None.
```

Result: `let share = 5; share + 1` — hovering on the second `share` returns the **keyword** hover, not the variable type. This is the "hover content emitted for wrong symbol" bug class from the brief.

**Fix**: Reorder — try `sig_table.fns.get(&token_name)` and a scope lookup first; only fall back to registry if no user-defined symbol matches. (Alternatively: pass position context — if `share` is used as a function parameter modifier, keyword wins; if it's an identifier in expression position, user-defined wins. The parser already distinguishes these contexts; LSP needs the AST node, not just the token.)

---

### Bug 2.9 — `let_to_const_promotion` over-suppresses on benign calls (`print`, pure functions)

**File**: `/workspaces/ynz/crates/ynz-typeck/src/inlay_hint_passes.rs:160-198` (`collect_maybe_mutated_expr`)
**Category**: Inlay-hint false negative (teaching surface silently doesn't teach)

```rust
Expr::Call(c) => {
    for arg in &c.args {
        if let Expr::Ident(name, _) = arg {
            out.insert(name.clone());    // marks EVERY call arg as possibly mutated
        }
        ...
    }
}
```

Even `print(x)` marks `x` as mutated. So:

```yinz
function entrypoint() -> nothing {
    let count = 5
    print(count)
}
// IDE shows: NO `let → const` hint (it should fire)
```

This is the dominant case in real code — almost every `let` is passed to *some* call, so the teaching hint almost never fires. Tier 2 false-negative.

**Fix**: Look up the callee's signature and check the parameter's ownership modifier (it's already wired in `collect_ownership_hints_expr`). Only mark as mutated when the parameter is declared `lend` or `give`. For `share` (which is the implicit default for `const` bindings) the call doesn't mutate.

---

## MEDIUM

### Bug 2.10 — `collect_maybe_mutated_expr` doesn't visit struct/array/map literals

**File**: `/workspaces/ynz/crates/ynz-typeck/src/inlay_hint_passes.rs:160-198`
**Category**: Inlay-hint false positive (narrow trigger)

`collect_maybe_mutated_expr` covers `Call`, `MethodCall`, `BinOp`, `UnaryOp`, `FieldAccess`, `IndexAccess` — but NOT `StructLit`, `ArrayLit`, `MapLit`, `PostfixOp`. So mutations inside literal values are missed:

```yinz
let nums = [a, b.consume(), c]
// `b.consume()` is a MethodCall inside ArrayLit — walker stops at the ArrayLit and never sees it
```

**Fix**: Add the missing `Expr` variants to the walker. Fall-through `_ =>` should match all leaf nodes only; compound expressions need explicit recursion.

---

### Bug 2.11 — `ownership_call_site_hints` doesn't handle generic functions or UFCS

**File**: `/workspaces/ynz/crates/ynz-typeck/src/inlay_hint_passes.rs:336-364`
**Category**: Inlay-hint false negative (teaching gap)

Line 339: `sig_table.fns.get(name).or_else(|| imported.get(name))` — never queries `generic_fn_table`. So calls to user-defined generic functions get no ownership hint. Documented as a known limitation but worth tracking — the muted hint protocol is supposed to be informative across all call sites.

UFCS method-call form (`player.heal(20)`) also gets no hint — the handler only matches `Expr::Call`, not `Expr::MethodCall`. This means the user sees ownership hints when they write `heal(player, 20)` but NOT when they write `player.heal(20)` — two call-syntax forms producing inconsistent teaching surfaces.

**Fix**: Add generic-fn lookup fallback and an `Expr::MethodCall` arm that resolves the method via the same UFCS lookup the typeck uses (`check.rs:2049-2076`).

---

### Bug 2.12 — Hover misses end-of-token cursor positions

**File**: `/workspaces/ynz/crates/ynz-lsp/src/hover.rs:21-29`
**Category**: Hover stale/edge-case (LOW-MEDIUM)

```rust
if byte_offset >= tok.span.start && byte_offset < tok.span.end {
```

Many editors place the cursor at the byte-after-last-char of a word when the user double-clicks or uses keyword navigation. The strict `< tok.span.end` excludes that position. Users will silently get "no hover" at what feels like a valid spot.

**Fix**: Use `<= tok.span.end` for the upper bound, OR fall back to the previous token if the offset is exactly `tok.span.end` and the next token isn't an identifier. Rust-analyzer uses inclusive end semantics for this exact reason.

---

## LOW

### Bug 2.13 — `array_to_fixed_promotion` hint emits no `TextEdit`, so click does nothing

**File**: `/workspaces/ynz/crates/ynz-lsp/src/inlay_hint.rs:216-225`
**Category**: Inlay-hint inconsistency (click-to-make-explicit broken)

`let_to_const_promotion` provides a `TextEdit` that rewrites `let → const`. `array_to_fixed_promotion` does not — there's no edit attached. The protocol described in `.claude/rules/inference.md` ("Two Surfaces for the Same Decision") says both auto-promotions get click-to-make-explicit. The user-visible behavior is "this decoration is unclickable" vs "this one rewrites my source." Inconsistent teaching surface.

If the rewrite is intentionally deferred (because `array<int>` → `fixed<int>` also requires sizing the literal), state that explicitly in the hint tooltip ("Click action: not supported — requires manual size — see docs").

---

### Bug 2.14 — Inlay-hint salsa queries don't filter `Type::Error`/`Type::Nothing` for copy hints

**File**: `/workspaces/ynz/crates/ynz-typeck/src/inlay_hint_passes.rs:416-433` (`collect_copy_hints_expr`)
**Category**: Edge-case noise

`is_trivially_copyable` matches `Int | Float | Bool | Number`, so `Type::Error` is not flagged — good. But the walker doesn't recurse into argument sub-expressions (compare with `collect_ownership_hints_expr` which DOES recurse via `collect_ownership_hints_expr(arg, ...)`). So copy hints fire only at the outermost call, missing nested calls like `outer(inner(x))` — `x` never gets a copy hint.

**Fix**: Recurse into args the same way ownership-hints does.

---

## Summary by Category

- **Null/Undefined**: 0
- **Types**: 0
- **Async**: 0
- **Logic**: 5 (Bugs 1, 2.1-2.5 — missing `referenced_names.insert` at five distinct AST sites)
- **Leaks**: 0
- **Isolation**: 0
- **Flow/Ordering**: 1 (Bug 2.6 — `else_arm` blindspot is an AST-walker ordering bug)
- **Inlay-hint false positive**: 2 (Bugs 2.6, 2.7)
- **Inlay-hint false negative**: 3 (Bugs 2.9, 2.10, 2.11)
- **Hover wrong-symbol**: 1 (Bug 2.8)
- **Hover edge-case**: 1 (Bug 2.12)
- **Click-action inconsistency**: 1 (Bug 2.13)

---

## Prioritized Fix Order

### Must Fix Now (Production Risk — teaching breaks)

1. **Bug 1 / 2.1-2.5** — five missing `referenced_names.insert` sites. Net change: 5 one-line additions + add walks for `Item::ShapeDecl` and `Item::ConstDecl` in `check_module`. Ship together as one PR — they share root cause and a single regression test suite.
2. **Bug 2.6** — `else_arm` blindspot in five AST walkers. Touch each of the five `collect_*_block` functions in `inlay_hint_passes.rs`.
3. **Bug 2.7** — nested `FieldAssign`/`IndexAssign` mutation tracking. Replace the 1-level `if let Expr::Ident(name, _) = receiver.as_ref()` checks with a `root_ident` walker.

### Should Fix Soon (User Impact)

4. **Bug 2.8** — hover keyword shadowing identifier. Reorder lookup in `hover_response`.
5. **Bug 2.9** — `let_to_const_promotion` over-suppresses on `print`-style calls. Resolve callee ownership before marking args as mutated.
6. **Bug 2.10** — missing `StructLit`/`ArrayLit`/`MapLit`/`PostfixOp` recursion in mutation walker.

### Nice to Fix (Code Quality)

7. Bug 2.11 — generic + UFCS coverage for ownership hints.
8. Bug 2.12 — hover end-of-token cursor.
9. Bug 2.13 — array→fixed click action consistency.
10. Bug 2.14 — copy-hint recursion into nested calls.

---

## What's Working Well

- The `check.rs:2240` and `check.rs:2244` paths DO correctly insert options/shape names from type annotations — the pattern is right; it just needs to be applied at the four other AST positions identified above.
- `check.rs:1349` and `check.rs:2054` correctly insert function names from free-fn calls and UFCS method calls, so unused imports of FUNCTIONS work fine.
- `check_query` orchestration (`queries.rs:158-200`) cleanly separates reference tracking from import-diagnostic emission — the architecture is sound; only the upstream insertions are missing.
- Salsa caching in `inlay_hint_passes.rs` correctly memoizes per-source-file, so the cost of running these hint passes on every keystroke is bounded.
- Three-part WHAT/WHAT-INSTEAD/WHY diagnostic format is consistently applied everywhere I looked — no jargon-mode regressions in user-facing strings.
