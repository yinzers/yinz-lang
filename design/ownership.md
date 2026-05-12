# Ownership — Design Decisions

User spec: `spec/ownership.md`

---

## Dot Modifiers over Rust Syntax

Rust ownership semantics, Yinz surface syntax. `name.share`, `name.lend`, `name.give`, `name.copy`, `name.freeze` as dot modifiers instead of `&T`, `&mut T`, `move`.

**Why**: Rust's `&'a mut T` syntax is the single biggest usability barrier in Rust adoption. Dot modifiers expose the same semantics through autocomplete-discoverable methods (Golden Rule 1). They appear when you type `.` on any variable — a developer can discover the entire ownership system without reading documentation.

**Smart defaults at call sites**: `.share` is inferred when a function signature declares `share`. Callers only annotate when escalating to `.lend` or `.give`. The safe path requires no typing; the unsafe path (mutation, transfer) requires explicit declaration.

**Explicit in signatures**: Function signatures always declare intent (`share`, `lend`, `give`). The contract is visible at the definition — no surprises for callers.

---

## No Direct Array Indexing — `.get(index)` Returns `maybe T`

`items[5]` is a compile error. All collection access by index uses `.get(index)` which returns `maybe T`. Collections include `.first()` and `.last()` returning `maybe T`.

**Why**: Out-of-bounds array access is one of the most common runtime crashes and security vulnerabilities (buffer overflows) in systems languages. If the compiler can enforce safe access universally, there's no reason not to. The cost is slightly more verbose access; the benefit is the elimination of an entire crash category.

**Performance**: In release mode, the compiler eliminates bounds checks it can statically prove are safe (e.g., a `fixed[3]` accessed at index 1 — provably in bounds). Debug mode always bounds-checks. Performance impact is negligible in practice.

**Consistency**: Maps already use `.get(key)` returning `maybe V`. The same pattern applies everywhere. No special case for arrays — one rule, all collections.
