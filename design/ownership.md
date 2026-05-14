# Ownership — Design Decisions

User spec: `spec/ownership.md`

---

## Dot Modifiers over Rust Syntax

Rust ownership semantics, Yinz surface syntax. `name.share`, `name.lend`, `name.give`, `name.copy`, `name.freeze` as dot modifiers instead of `&T`, `&mut T`, `move`.

**Why**: Rust's `&'a mut T` syntax is the single biggest usability barrier in Rust adoption. Dot modifiers expose the same semantics through autocomplete-discoverable methods (Golden Rule 1). They appear when you type `.` on any variable — a developer can discover the entire ownership system without reading documentation.

**Explicit in signatures**: Function signatures always declare intent (`share`, `lend`, `give`). The contract is visible at the definition — no surprises for callers.

---

## Uniform Inference + IDE Hints (call sites)

Call sites in Yinz follow the [uniform inference rule](../.claude/rules/inference.md): if the compiler can figure out the right dot modifier from the function signature and the binding type, the developer doesn't type it. The IDE shows the inferred modifier as muted text — neutral gray for `.share` (benign read-only), red-tinted for `.lend`/`.give` (cautionary: mutation or transfer happens here).

Hover-tooltips on the muted text follow the three-part WHAT / WHAT-INSTEAD / WHY format from Golden Rule 11. The text completes to valid Yinz syntax — click-to-make-explicit produces real code.

**Why inferred-with-hints instead of inferred-only OR required-explicit**:

- **Inferred-only** (compiler picks, no visibility): mutation becomes invisible to readers. `healPlayer(player)` looks identical whether the function mutates or not. Bad for jr devs learning the model.
- **Required-explicit** (every call types `.share`/`.lend`/`.give`): too much noise. Developers stop reading and the marker degrades to syntactic burden. Inverse anti-pattern — see graveyard Entry 2.
- **Inferred + muted hint** (Yinz's choice): the compiler does the work, the IDE shows what happened, hover teaches WHY. Devs learn ownership by reading their own code.

This is consistent with type inference, lifetime inference, wait-point inference, and allocator inference — one rule across all surfaces.

---

## `const` Deep Immutability — Safety + Performance Contract

`const` is the load-bearing immutability primitive in Yinz. It is exactly equivalent to Rust's non-`mut` binding (Rust calls it `let`; Yinz calls it `const`).

### What `const` blocks

A `const` binding rejects ALL paths to mutation:

1. **Reassignment** — `constVar = newValue` is a compile error (already enforced as of M2 in `crates/ynz-typeck/src/check.rs` `check_assign`)
2. **Field mutation** — `constVar.field = x` is a compile error (enforced when field assignment lands in M4)
3. **Mutable borrows** — `constVar.lend` at a call site is rejected; cannot pass a `const` value where the function declares `lend` (enforced in M4)
4. **Ownership transfer** — `constVar.give` is rejected (enforced in M4)
5. **Mutable inference** — the compiler will NEVER infer `.lend` or `.give` for a `const` binding. If a function needs `.lend` and the caller passes a `const`, that's a compile error pointing the user toward declaring with `let`.

### Why this is load-bearing — Safety + Performance

**Safety**: aliasing rules. If `const` can be mutated through any path, then two `const` references to the same value could observe different state, breaking the language's promise that "two reads of the same `const` always return the same value." That promise is what makes `const` shareable across threads without locks — also what makes the borrow checker tractable.

**Performance — the LLVM contract**: a function parameter declared `share` (or inferred from a `const` binding) emits the LLVM **`readonly`** attribute. Combined with **`noalias`** (which Yinz can guarantee from the ownership system), the optimizer gets the same aliasing information that lets Rust beat C++ in benchmarks. Specifically:

- `readonly` → optimizer knows this pointer's pointee never changes during the call → enables loop-invariant code motion, common subexpression elimination, and skipping spurious reloads
- `noalias` → optimizer knows no other pointer aliases this one → enables vectorization and reordering that C/C++ can't do (because any pointer might alias any other)

C has `restrict` for `noalias` but it's a programmer's promise the compiler can't verify. Rust/Yinz make it enforced at compile time via the ownership system. That's the performance moat.

### M4 codegen contract

When M4 lands (types + ownership), codegen MUST emit:

- `readonly` on every LLVM function parameter declared as `share T`
- `readonly` on every parameter inferred from a `const` binding at the call site (even if the signature says `lend` — though that combo should error out before codegen)
- `noalias` on every parameter that the borrow checker has proven non-aliased (which, for `share` + ownership rules, should be most of them)

Failure to emit these attributes is a perf regression even if the program is correct. M4 plan's `### Performance` invariant subsection must explicitly assert this — and the graveyard Entry 1 catches M4 plans that omit it.

### Forward-compatibility for shapes (M4)

When `shape` declarations land in M4, field assignment (`player.field = x`) becomes parseable. The typeck must reject field assignment when the receiver is a `const` binding, with a teaching error pointing the user to declare `let` if they need mutation. This is item #2 above and is the single biggest gap the design-lockdown plan was created to prevent.

Cross-references:
- `spec/variables.md` "What const blocks" section (user-facing)
- `.claude/plans/active/v0-1-compiler.md` Forward-Compatibility Constraints (locked decisions for M4+)
- `.claude/graveyard.md` Entry 1 (mechanical enforcement for M4+ plans)

---

## No Direct Array Indexing — `.get(index)` Returns `maybe T`

`items[5]` is a compile error. All collection access by index uses `.get(index)` which returns `maybe T`. Collections include `.first()` and `.last()` returning `maybe T`.

**Why**: Out-of-bounds array access is one of the most common runtime crashes and security vulnerabilities (buffer overflows) in systems languages. If the compiler can enforce safe access universally, there's no reason not to. The cost is slightly more verbose access; the benefit is the elimination of an entire crash category.

**Performance**: In release mode, the compiler eliminates bounds checks it can statically prove are safe (e.g., a `fixed<3>` accessed at index 1 — provably in bounds). Debug mode always bounds-checks. Performance impact is negligible in practice.

**Consistency**: Maps already use `.get(key)` returning `maybe V`. The same pattern applies everywhere. No special case for arrays — one rule, all collections.
