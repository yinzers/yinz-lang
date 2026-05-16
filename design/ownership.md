# Ownership — Design Decisions

User spec: `spec/ownership.md`

> **Read first**: Yinz is NOT object-oriented — see `.claude/rules/non-oop.md`. Ownership modifiers are SIGNATURE-level keywords (declared at function definitions); body-level `.share()/.lend()/.give()` syntax does NOT exist. The compiler infers ownership at call sites from the callee's signature and renders the result as IDE muted hints. Only `.copy()` and `.freeze()` are body-level dot-postfix operations (with parens per `.claude/rules/dot-postfix.md`). Locked r4, r10, r11.

---

## Ownership Concepts

Rust ownership semantics, Yinz surface syntax. Three modes for passing a value into a function:

- **`share`** — read-only access. Caller keeps ownership. Multiple shares allowed simultaneously.
- **`lend`** — mutable access. Caller keeps ownership. Only one outstanding lend at a time (no other share or lend during the borrow).
- **`give`** — ownership transfer. Caller loses the value; receiver gains it.

Plus two body-level operations on values:
- **`.copy()`** — produce a new owned value (only on transitively-trivially-copyable types per r4).
- **`.freeze()`** — lock a binding from further mutation for the rest of its scope.

**Why English keywords instead of `&T`/`&mut T`/`move`**: Rust's `&'a mut T` syntax is the single biggest usability barrier in Rust adoption. Yinz uses plain English signature keywords (`share`/`lend`/`give`) declared at the function definition; the compiler infers them at every call site so the developer rarely types ownership at all. `.copy()` and `.freeze()` use dot-postfix-with-parens per `.claude/rules/dot-postfix.md`.

---

## Signature-Level Declaration (Explicit)

Function signatures declare ownership intent as keyword prefixes on parameters:

```yinz
function greet(share name: string) -> nothing { ... }      // function will only read name
function rename(lend player: Player) -> nothing { ... }    // function will modify player
function consume(give data: Data) -> nothing { ... }       // function takes ownership of data
```

The signature is the contract. Anyone reading the function knows exactly what it does with its inputs.

**REQUIRED on**:
- Contract method signatures inside `shape` declarations (`shape Foo { method(share self, ...) -> X }`)
- Function-type annotations (`let f: function(give Data) -> nothing`)

**OPTIONAL on free functions with bodies** — the compiler infers the modifier from how the body uses the parameter (`.give()` to consume, mutation to lend, only-read to share). The author may still type the modifier explicitly for emphasis; the compiler verifies it matches the body.

---

## Call-Site Inference (Implicit — No `.share()`/`.lend()`/`.give()` Body Syntax)

Call sites in Yinz follow the uniform inference rule (`.claude/rules/inference.md`): the compiler reads the callee's signature and inserts the right modifier automatically. **There is no body-level syntax for `.share()`/`.lend()`/`.give()`** — those modifiers exist only in signature position; call sites get them via compiler inference and IDE rendering.

```yinz
greet(name)           // compiler reads greet's signature → name is shared
rename(player)        // compiler reads rename's signature → player is lent (mutated)
consume(data)         // compiler reads consume's signature → data is given (consumed)
print(data)           // COMPILE ERROR: data was consumed above; use .copy() if you need to keep it
```

The IDE renders the inferred modifier as muted text:
- **Neutral gray** for `share` (benign read-only)
- **Red-tinted** for `lend`/`give` (cautionary — mutation or transfer)

Click-to-make-explicit on a muted hint converts the source to the explicit signature-level form for the function-decl side, NOT to a call-site syntax (which doesn't exist).

**Why inferred-with-hints instead of inferred-only OR required-explicit**:

- **Inferred-only** (compiler picks, no visibility): mutation becomes invisible to readers. `healPlayer(player)` looks identical whether the function mutates or not. Bad for jr devs.
- **Required-explicit** (every call types ownership): too much noise; developers stop reading; the marker degrades to syntactic burden. Inverse anti-pattern — graveyard Entry 2.
- **Inferred + muted hint** (Yinz's choice): compiler does the work; IDE shows what happened; hover teaches WHY.

Consistent with type inference, lifetime inference, wait-point inference, allocator inference — one rule across all surfaces.

---

## Body-Level Operations: `.copy()` and `.freeze()`

These two are the only body-level dot-postfix ownership operations. They use parens per the dot-postfix rule.

### `.copy()` — produce a new owned value

```yinz
const backup = original.copy()       // strict: only legal on transitively-trivially-copyable types
saveForever(backup)                   // backup consumed (give inferred), original unchanged
```

Strict cheap-only per r4: `.copy()` is only legal when every field of the value's type is transitively trivially copyable. For non-trivial deep copies, the user defines a standalone function (typically also named `copy`) and calls it via normal function/UFCS syntax — Yinz prefers explicit user-defined deep-copy semantics over silent expensive copies.

### `.freeze()` — lock a binding from further mutation

```yinz
let config: ConfigBuilder = { rules: [] }
config.addRule("a", 1)
config.addRule("b", 2)
config.freeze()                       // lock from this point forward
runApp(config)                         // config still usable for reads (share inferred); no further mutation
```

The freeze flag persists for the rest of the binding's scope. Useful for build-then-lock patterns where mutability is needed during construction but should be prevented afterward.

---

## `const` Deep Immutability — Safety + Performance Contract

`const` is the load-bearing immutability primitive in Yinz. It is exactly equivalent to Rust's non-`mut` binding (Rust calls it `let`; Yinz calls it `const`).

### What `const` blocks

A `const` binding rejects ALL paths to mutation:

1. **Reassignment** — `constVar = newValue` is a compile error (already enforced as of M2 in `crates/ynz-typeck/src/check.rs` `check_assign`)
2. **Field mutation** — `constVar.field = x` is a compile error (enforced when field assignment lands in M4)
3. **Passing to a `lend` parameter** — the compiler refuses to infer the mutable modifier at the call site if the binding is `const`; compile error pointing to "declare with `let` if you need mutation" (enforced in M4)
4. **Passing to a `give` parameter** — same: compiler refuses to infer the transfer; compile error (enforced in M4)
5. **`.freeze()` is redundant** — `const` bindings are already locked; calling `.freeze()` is a no-op but allowed for stylistic uniformity

### Why this is load-bearing — Safety + Performance

**Safety — aliasing rules**: if `const` can be mutated through any path, then two `const` references to the same value could observe different state, breaking the language's promise that "two reads of the same `const` always return the same value." That promise is what makes `const` shareable across threads without locks — also what makes the borrow checker tractable.

**Performance — the LLVM contract**: a function parameter declared `share` (or inferred from a `const` binding at the call site) emits the LLVM **`readonly`** attribute. Combined with **`noalias`** (which Yinz can guarantee from the ownership system), the optimizer gets the same aliasing information that lets Rust beat C++ in benchmarks:

- `readonly` → optimizer knows this pointer's pointee never changes during the call → enables loop-invariant code motion, common subexpression elimination, and skipping spurious reloads
- `noalias` → optimizer knows no other pointer aliases this one → enables vectorization and reordering that C/C++ can't do (because any pointer might alias any other)

C has `restrict` for `noalias` but it's a programmer's promise the compiler can't verify. Rust/Yinz make it enforced at compile time via the ownership system. That's the performance moat.

### M4 codegen contract

When M4 lands, codegen MUST emit:

- `readonly` on every LLVM function parameter declared as `share T`
- `readonly` on every parameter inferred from a `const` binding at the call site (even if the signature is bare and the modifier was inferred from body usage)
- `noalias` on every parameter the borrow checker has proven non-aliased (most of them for `share`/`lend`/`give` per ownership rules)

Failure to emit these attributes is a perf regression even if the program is correct. The M4 plan's `### Performance` invariant subsection explicitly asserts this, and `.claude/graveyard.md` Entry 1 catches M4 plans that omit it.

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
