---
name: "IMP-ownership"
description: "Design decisions for Yinz's ownership system (share/lend/give signature keywords plus .copy()/.freeze() operations) and how the compiler infers call-site ownership instead of requiring Rust-style borrow syntax."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-16"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Ownership — Design Decisions

User spec: [`docs/reference/REF-ownership.md`](../../reference/REF-ownership.md)

> **Read first**: Yinz is NOT object-oriented — see [`.claude/rules/non-oop.md`](../../../.claude/rules/non-oop.md). Ownership modifiers are SIGNATURE-level keywords (declared at function definitions); body-level `.share()/.lend()/.give()` syntax does NOT exist. The compiler infers ownership at call sites from the callee's signature and renders the result as IDE muted hints. Only `.copy()` and `.freeze()` are body-level dot-postfix operations (with parens per [`.claude/rules/dot-postfix.md`](../../../.claude/rules/dot-postfix.md)). Locked r4, r10, r11.

---

## Ownership Concepts

Rust ownership semantics, Yinz surface syntax. Three modes for passing a value into a function:

- **`share`** — read-only access. Caller keeps ownership. Multiple shares allowed simultaneously.
- **`lend`** — mutable access. Caller keeps ownership. Only one outstanding lend at a time (no other share or lend during the borrow).
- **`give`** — ownership transfer. Caller loses the value; receiver gains it.

Plus two body-level operations on values:
- **`.copy()`** — produce a new owned value: a genuinely independent one for every type where independence is meaningful, and a compile error for the types where it is not.
- **`.freeze()`** — lock a binding from further mutation for the rest of its scope.

**Why English keywords instead of `&T`/`&mut T`/`move`**: Rust's `&'a mut T` syntax is the single biggest usability barrier in Rust adoption. Yinz uses plain English signature keywords (`share`/`lend`/`give`) declared at the function definition; the compiler infers them at every call site so the developer rarely types ownership at all. `.copy()` and `.freeze()` use dot-postfix-with-parens per [`.claude/rules/dot-postfix.md`](../../../.claude/rules/dot-postfix.md).

---

## Signature-Level Declaration (Explicit)

Function signatures declare ownership intent as keyword prefixes on parameters:

```ynz
function greet(share name: string) -> nothing { ... }      // function will only read name
function rename(lend player: Player) -> nothing { ... }    // function will modify player
function consume(give data: Data) -> nothing { ... }       // function takes ownership of data
```

The signature is the contract. Anyone reading the function knows exactly what it does with its inputs.

**OPTIONAL everywhere a parameter is declared** — on free functions with bodies and on contract method signatures inside `shape` declarations (`shape Foo { method(share self, ...) -> X }` — the parser accepts a bare position, `parser.rs:3996–4012`). Yinz has no function-type annotation syntax to consider as a third position — `ynz-ast`'s `Type` enum has no function variant and `parse_type_with_depth` (`parser.rs:900`) has no `function` arm (parked item 23, `.claude/plans/parked.md`). A bare position means the compiler figures out read-only vs. mutating from use (the `effective_ownership` fixpoint; for a contract, from the implementing function). **`give` is the one word that is never figured out** — a position is a give position only when the word is written, on a function or a contract alike ("Transfer — Who Else Holds This Value" below). The author may still type `share`/`lend` explicitly for emphasis; the compiler verifies it matches the body.

---

## Call-Site Inference (Implicit — No `.share()`/`.lend()`/`.give()` Body Syntax)

Call sites in Yinz follow the uniform inference rule ([`.claude/rules/inference.md`](../../../.claude/rules/inference.md)): the compiler reads the callee's signature and inserts the right modifier automatically. **There is no body-level syntax for `.share()`/`.lend()`/`.give()`** — those modifiers exist only in signature position; call sites get them via compiler inference and IDE rendering.

```ynz
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

```ynz
const backup = original.copy()       // a value nobody else reaches
saveForever(backup)                   // backup consumed (give inferred), original unchanged
```

**The ruling (Patrick, 2026-09-06; superseded the r4 "strict cheap-only" rule this section used to state).** `.copy()` returns a genuinely independent value for every type where independence is meaningful, and is a COMPILE ERROR where it is not. Nothing silently aliases, ever. There are exactly two buckets, no third.

The r4 text this replaced — "`.copy()` is only legal when every field is transitively trivially copyable; write a standalone `copy()` function for anything deeper" — described a compiler that never shipped. What shipped instead was an alias: `maybe`, `fixed`, union and `dynamic` all fell into a `.copy()` arm that handed back the receiver's own pointer while the type checker admitted the call, which produced one use-after-free and one silent wrong answer (v0.3 concurrency hardening, cluster C2). Deep-and-refuse is what removes that; a shallow copy would have re-introduced the same defect one level down and called it a design. Golden Rule 2 decides the tie against Golden Rule 10: "sometimes independent, sometimes not, depending on how nested your value is" is not something a junior developer can predict without documentation. If deep copying ever proves too slow on a real workload, the answer is an explicit cheaper operation with its own name — never a silent reinterpretation of `.copy()`.

**The one table.** `ynz_typeck::owned_copy::owned_copy_plan` is exhaustive over `Type` with no `_` arm, and both consumers read it: `.copy()`'s lowering and the `background`-argument path (`emit_owned_copy`, also exhaustive, no `_` arm). Neither classifies a type itself, so the two can never again disagree about what an owned copy is (`.claude/rules/authoritative-derivation.md`).

| Type | What `.copy()` does |
|---|---|
| `int`, `float`, `bool`, `string`, an `options` value, `sensitive string` | The receiver's bits ARE the copy — nothing can change what is behind them |
| `number` (≤ 34 digits), `range` | Immutable contents in storage the producing frame owns: the receiver, inside a body. A `background` argument of one is re-homed onto the heap first (`number`) or refused (`range` — no re-homing path exists) |
| `shape` | Its own bytes into fresh storage. A pointer-valued FIELD (a nested shape, an `array`, a `map`, a `maybe`) is still copied as a pointer — a named residual, not a claim |
| `fixed<T>` | Its N inline cells into a fresh slot, and for a `string` or `shape` item a copy of each (a string copy, or a fresh heap cell holding a deep shape copy). `fixed` of any other pointer-item type is refused: a `fixed` has no per-item pointer to follow |
| `array<T>` | A fresh header and buffer, and — when the cells hold pointers — every item copied through the same table, all the way down |
| `map<K, V>` | A fresh header and its four buffers. A value cell holding a pointer is copied as a pointer, the same residual as a shape field |
| `maybe<T>` | A fresh envelope cell, for the inner types the shared maybe-ownership core can follow: a simple value, a `number`/`range`, or a `shape`. `maybe<array<T>>`, `maybe<map<K, V>>`, `maybe<fixed<T>>` and `maybe<maybe<T>>` are refused |
| `sensitive<T>` | Copies and KEEPS the label, so the copy is still redacted wherever it is printed. Refusing would push people to call `.reveal()` just to get a copyable value, which is strictly worse for the secret. Only inner kinds whose copy IS the receiver's bits qualify, and `sensitive` wraps only `string` today |
| `channel<T>`, a task handle, a union, `dynamic C`, a loop's `MapEntry`, an unchecked `errors` value, a bignum, `nothing`, a type placeholder, a generic shape | **Compile error**, with WHAT / WHAT-INSTEAD / WHY naming what to do instead |

A refusal is a real answer and it is loud: a user who can see the refusal is better off than a user handed an alias they cannot see. The refusal text lives with the plan (`CopyRefusal`, three slots) and renders through the `CopyNotIndependent` diagnostic template; the `background` face of the same ruling renders through `SpawnArgNotIndependent`.

### `.freeze()` — lock a binding from further mutation

```ynz
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

Failure to emit these attributes is a perf regression even if the program is correct. The M4 plan's `### Performance` invariant subsection explicitly asserts this, and [`.claude/graveyard.md`](../../../.claude/graveyard.md) Entry 1 catches M4 plans that omit it.

### Forward-compatibility for shapes (M4)

When `shape` declarations land in M4, field assignment (`player.field = x`) becomes parseable. The typeck must reject field assignment when the receiver is a `const` binding, with a teaching error pointing the user to declare `let` if they need mutation. This is item #2 above and is the single biggest gap the design-lockdown plan was created to prevent.

Cross-references:
- [`docs/reference/REF-variables.md`](../../reference/REF-variables.md) "What const blocks" section (user-facing)
- [`.claude/planning/done/2026-05-12-v0-1-compiler/roadmap.md`](../../../.claude/planning/done/2026-05-12-v0-1-compiler/roadmap.md) Forward-Compatibility Constraints (locked decisions for M4+)
- [`.claude/graveyard.md`](../../../.claude/graveyard.md) Entry 1 (mechanical enforcement for M4+ plans)

---

## No Direct Array Indexing — `.get(index)` Returns `maybe<T>`

`items[5]` is a compile error. All collection access by index uses `.get(index)` which returns `maybe<T>`. Collections include `.first()` and `.last()` returning `maybe<T>`.

**Why**: Out-of-bounds array access is one of the most common runtime crashes and security vulnerabilities (buffer overflows) in systems languages. If the compiler can enforce safe access universally, there's no reason not to. The cost is slightly more verbose access; the benefit is the elimination of an entire crash category.

**Performance**: In release mode, the compiler eliminates bounds checks it can statically prove are safe (e.g., a `fixed<3>` accessed at index 1 — provably in bounds). Debug mode always bounds-checks. Performance impact is negligible in practice.

**Consistency**: Maps already use `.get(key)` returning `maybe<V>`. The same pattern applies everywhere. No special case for arrays — one rule, all collections.

---

## Transfer — Who Else Holds This Value (v0.3-M8 Phase 2; signed off 2026-09-03 — the v0.3-M8 plan's `audit.md` (`.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/audit.md`) SIGN-OFF record; Phase 4 implements)

**Status**: designed in v0.3-M8 Phase 2, signed off 2026-09-03 (`audit.md`'s SIGN-OFF record; `git log --grep=m8-p2` for the design rounds); **SHIPPED in v0.3-M8 Phase 4** (`crates/ynz-typeck/src/effective_ownership.rs` `provenance`/`consumed`/`returns_fresh`, `scope.rs` `Origin`/`ConsumedBy`/alias classes, `check.rs` `check_transfer` — the one emit site). This section is the ONE home for the question *"may this value leave this frame for a sink that will free it?"* — a channel `send`, a `give` parameter in any call form, a `background` argument the spawn gives away. [`IMP-concurrency.md`](IMP-concurrency.md)'s channel-close section cites it for the channel instance; it does not restate the rule.

### The producer this section retires

A list of argument shapes at call sites is unbounded — every expression form the list omits is a hole (eight probes were run against the tree; seven found a live hole, and the eighth — `dynamic Contract` — ICEs in codegen before reaching one, so it confirms zero runtime exposure rather than a hole: `.claude/corpses.md`, first entry, and `git log --grep=m8-p2`). The fix is not another shape on a list. It is one **provenance** function, defined once, with an exhaustive match the compiler enforces, whose four-valued answer every sink consumes; one **binding-event** rule, exhaustive over every statement form that can make a name denote a value; and the whole-program facts the existing `effective_ownership` fixpoint already computes, threaded to those sinks instead of re-derived.

**Where this diverges from the v0.3-M8 plan, surfaced in the required form.** The plan's Phase 2 block (FRAGO 008) says the phase answers the question "by THREADING `effective_ownership` and never enumerating `Expr::` variants"; this design does ONE exhaustive `Expr` match, inside the authoritative module, because "what does this expression denote, ownership-wise" is a syntactic question by nature — it is the input the fixpoint itself consumes (`arg_is_binding`, `root_binding_name` and `place_path` are already `Expr` matches inside that module). The corpse bans a partial list re-derived at each CALL SITE; a single match with no wildcard arm, in the one owning function, is closed by the compiler's exhaustiveness check rather than by a reviewer's memory. Recorded here and in the Phase 2 sign-off packet. **Ruled** (packet item (i), 2026-09-03 — `audit.md`'s SIGN-OFF record): a single exhaustive, wildcard-free match in the owning module is the remedy for the corpse's ban on open per-site lists, not an instance of it — the plan's Phase 2 block sentence is amended to this wording under FRAGO 010 (`audit.md`).

### Where the authority lives

`crates/ynz-typeck/src/effective_ownership.rs` is the module. It already owns (a) a Kleene fixpoint over every parameter of every local function, converging under mutual recursion, seeded from declared `lend`/`give` and raised by body use; (b) the per-expression helpers `arg_is_binding`, `root_binding_name`, `place_path`. `root_binding_name` already has a pre-existing twin at `check.rs:11864` beside `effective_ownership.rs:658` — Phase 4 collapses it to the one definition this section's threading depends on, or this section's "one authoritative answer" claim rests on an un-collapsed twin ([`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md)). Two corrections to how the module has been described: the fixpoint runs in the same query as the body check but **after** it today (`check(...)` at `queries.rs:~423`, `effective_ownership::analyze` at `:503`), and it depends only on the parse and the signature tables, both in hand at `queries.rs:279–308` — so hoisting it above the body check is a reorder, not a redesign. Phase 4 performs that reorder; every consumer below assumes the report exists before any body is checked.

This design **extends that module** with three facts and one function, and forbids a second derivation of any of them anywhere else ([`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md)):

1. **`provenance(expr, bindings) -> Provenance`** — the syntactic answer to "what does this expression denote, ownership-wise". Exhaustive over `Expr` with **no wildcard arm**, so a new expression form is a compile error in exactly one function until someone classifies it; consumers never see syntax, only the four values below.
2. **`consumed[fn][i]: bool`** — a per-position fact in the same fixpoint loop: position `i` of `fn` is *consumed* when it is declared `give`, OR its body passes it (as a whole binding) to a consumed position, OR sends it (as a whole binding) on a channel. Monotone false→true; converges for the same reason `Writes` does. **The send case is gated on the parameter's DECLARED type**: the fixpoint runs before body checking and has no type environment (`effective_ownership.rs:475–491` classifies a `.send` receiver by method NAME and the `is_user_fn` test, never by receiver type), so "sends it on a channel" means: a builtin `send` (not a user function of that name) whose argument is the whole parameter, AND `FunctionDecl.params[i].ty` resolves to an owned-heap element kind (`channel_elem_drop(ty).is_some_and(transfers_source)` — `array`/`map`). A `number`/`int`/`string` parameter sent on a channel is never `consumed`; the only builtin `send` that takes an `array`/`map` is the conduit send (channel or handle, one send core), so the element type is the parameter's own declared type and needs no receiver lookup. It is a lower bound (a body that first rebinds the parameter with `let` and then sends the rebinding is caught by the emit site's alias classes, below, not by this fact) — it exists so a relay chain is reported in ONE compile, never to accept a program.
3. **`returns_fresh[fn]: Freshness`** — per function: `Fresh` iff every `return` yields a value nobody else reaches (a fresh expression, a `give` parameter, or a local whose initializer was fresh, or a local that a `let`/assignment of a fresh local handed its value over to: a store hands the value over, so the new name is fresh and the old one is gone); `MayAlias` otherwise (a `share`/`lend`/bare parameter or any piece of one; a call to a `MayAlias` or imported callee; anything unclassifiable). Bottom `Fresh`, raised in the same loop. Imported functions are `MayAlias`. **One exception, by declared return type**: a function whose return type is a `string`, `array`, `map`, shape, `channel`, `maybe` of a channel or a union with a shape variant is `Fresh` whatever it returns, because its `return` copies what is not a fresh value or an owned local (`owned_copy::copied_at_return`, the one predicate typeck and codegen's return seam share; see "Non-owned reads"). The same declared type makes an IMPORTED callee's result `Fresh` (its body is not visible, its signature says what its `return` does). A channel's copy is a counted reference (`ynz_channel_share`), taken by the callee's `return` for a read that does not move, so the caller's binding owns one reference and gives it back once. A call of a generic function is an owned value too: its declared return type is a type parameter, so the call's own instantiated type decides (codegen copies a heap-typed result at the generic `return`). Only a `maybe` of anything but a channel and a union with no shape variant still return a piece.
4. **`classify_binding_in_stmts(name, stmts) -> EffectiveOwnership`** — the existing per-name body classifier (`classify_param_in_block`) exposed over an arbitrary statement suffix. It never cared that the name was a parameter; the Auto-Arc caller-side proof below needs the identical question asked about a local over the statements after a spawn.

```rust
pub enum Provenance {
    /// This evaluation is the value's only holder. Transferable; nothing to consume.
    Fresh,
    /// Exactly the value a binding names (`Ident` / `self`). Transferable iff the
    /// binding's origin (below) permits; consumes the binding's whole alias class.
    Whole(String),
    /// A value someone still reaches through the named roots — a field, an index,
    /// a loop cell, a call that returns a piece
    /// of its argument. Never transferable; the fix is `.copy()` on the reached piece.
    Reaches(Vec<String>),
    /// Cannot classify (function-value call, dynamic-dispatch result, imported
    /// non-fresh callee). Never transferable.
    ///
    /// `.copy()` no longer lands here: since the v0.3 concurrency-hardening owned-copy
    /// fix, a `.copy()` that would not be independent does not compile at all.
    Unknown,
}
```

Classification (one function; listed here so a reader can check it, not so a call site can copy it):

| Expression | Provenance |
|---|---|
| int / number / bool / string / `none` literal; `BinOp`; `UnaryOp`; interpolated string; `is` | `Fresh` (value bits, or immortal string bytes) |
| `Ident`, `self` | `Whole(name)` — always; the binding's ORIGIN decides transferability |
| `FieldAccess` / `IndexAccess` | `Reaches(roots of the receiver's own provenance)` — a name chain roots in its binding, a call or method result in the bindings it may hold (`firstBag(bags).items` reaches `bags`); a piece of a fresh temp (`makeBucket().rows`) is `Reaches([])` — still not transferable; an untraceable receiver makes the piece `Unknown` |
| array / map / shape literal | `Fresh` — a literal takes over every named heap part (each part is a store, sink 4), so it is the sole holder of everything in it |
| `.copy()` | `Fresh` iff `copy_is_independent(type)` — since the v0.3 concurrency-hardening owned-copy fix that predicate is a RE-EXPORT of `owned_copy::copy_is_independent`, derived from `owned_copy_plan`, the ONE per-type table codegen's `.copy()` lowering and its `background`-argument path both consume (`ynz_typeck::owned_copy`). It is `false` for exactly the types the table REFUSES, and a refused `.copy()` is a compile error with teaching text, so `Unknown` is unreachable from this row in a program that builds |
| `.freeze()` (the other `PostfixOpKind` arm) | `Fresh` — it is typed `nothing` (`check.rs:6947`, the P3c mutability flip never shipped; codegen lowers it as the bare receiver, `emit.rs:19297`), so no sink can ever accept it and there is no value to hold. Exhaustiveness is variant-level, so the arm is written; if `.freeze()` is ever retyped to return its receiver, this row becomes `provenance(receiver)` in the same commit — the non-wildcard match is what forces that reader here |
| constructor call — `channel<T>()` (the only constructor-call form the parser accepts; `array<T>()` / `map<K, V>()` are not forms the parser has, an empty container is the literal `[]` / `{}`, classified in the literal row); `.receive()` on a channel or handle | `Fresh` (the receiver's `maybe<T>` is the sole reference — [`IMP-concurrency.md`](IMP-concurrency.md) "What this makes sound") |
| builtin method call | from the ONE `builtins` table (`builtin_method_returns_fresh(name)`: `receive` and `split`, the only built-in methods codegen lowers that return a fresh heap value; a `split` result nothing binds is also a temporary); **default `Reaches([receiver root ∪ the roots of every heap argument])`** (`maybe.or(d)` and `a.concat(b)` can hand back an argument) — `.get`/`.first`/`.last` return a cell; `.sort`/`.filter`/`.map`/`.concat` are widened one at a time with evidence, never by default (parked item 18 is this table, recorded as a deliberate conservative omission) |
| user function call, local | `Fresh` if `returns_fresh[callee] == Fresh`, else `Reaches(∪ roots of the argument expressions)` |
| user function call, imported; non-`Ident` callee; `dynamic Contract` dispatch result | `Unknown` |
| `wait e` | `provenance(e)` |
| `background …` (a handle) | `Fresh` |
| `Expr::Error` | `Unknown` |

### Binding events, origin and alias classes (the typeck half)

Origin and alias class are properties of a **binding event** — the moment a name is made to denote a value — never of the name. `ScopeEntry` (`crates/ynz-typeck/src/scope.rs`) gains an **origin** and an **alias-class id**; both are (re)computed at every binding event from the provenance of the value being bound. An **alias class** is the set of live entries that denote or reach one value, so that consuming any member consumes all of them (`let other = rows` and `rows = other` close by the same mechanism, not by a special case). Class membership is keyed by ENTRY, not by name: a shadowed outer entry keeps its membership while the inner frame is live and is the entry `lookup` finds again once the frame pops.

**The binding-event rule is exhaustive over the places a name can be bound, with no wildcard.** `Stmt` has ten variants (`ynz-ast/src/nodes.rs`, count pinned by test). Exactly three of them bind a name, and one non-statement form does:

| Binding event | What it is in the AST | Effect on the NAME's previous entry (if any) | Origin / class of the new entry |
|---|---|---|---|
| `let`/`const` — first declaration | `Stmt::Let` | none | per the initializer table below |
| `let`/`const` — **shadowing** a name already in scope (same frame: `Scope::insert` replaces; inner frame: hides — both are legal today, probe `let rows = [1,2,3]; let rows = [4]` prints `1`) | `Stmt::Let` | the previous entry keeps its own origin, class and consumed state (it still names its old value; a same-frame shadow makes that value unreachable by name — the never-drop-locals class, not this section's) | per the initializer table below — a NEW entry, never a mutation of the old one |
| **reassignment** `name = value` | `Stmt::Assign` | the entry **leaves its class** (the class keeps its other members and their consumed state — they still hold the old value); the entry's consumed state is REVIVED for the code the assignment dominates only (a consumed name that is reassigned names a new value: `eat(rows); rows = [4, 5]; rows.count()` is correct; `eat(rows); if (c) { rows = [4] }; rows.count()` is not, see "Revive is path-aware") | per the initializer table below, computed from the assigned value |
| `for` loop variable | `Stmt::For` | none (frame-scoped; `Assign` to it is already a compile error, `check.rs:2444`) | `Cell` (a cell of the iterated value; never transferable); joins the class of every binding the iterated value reaches |
| every name a `for` destructure binds (`destructure_pattern`, `map_destructure_pattern`) | **not** a `Stmt::For` binding — the parser desugars each destructured name to its own `Stmt::Let { value: __shape.field }`, prepended ahead of the loop body (`parser.rs:2252–2274`; map form `:2417`) | none | a `Let` event, so it takes the initializer table below from `Reaches(__shape)` provenance — never transferable, joins the iterated root's class; unlike the loop variable, `Assign` to a destructured name is admitted (`check.rs:2436–2465`) and is covered by the `Assign` row above, not this one (parked item 25) |
| function parameter | `FunctionDecl.params` (not a `Stmt`) | none (`Assign` to a parameter is already a compile error, `check.rs:2436`) | `Param(declared modifier)`; its own class |

The other seven statement forms bind no name: `Expr`, `If`, `While`, `Return` (they only read); `Match` (arms carry a `MatchPattern`, not a binding — `is` narrowing refines a type, it does not rebind); `FieldAssign` and `IndexAssign` write THROUGH a name into a container — they never change what the name denotes, and as stores they hand the value over (see "Stores are sinks" below). `Expr` has no binding form at all (no closure or arrow-function variant exists in `ynz-ast`). If a variant that binds a name is ever added to `Stmt`, it fails the pinned variant-count test and the `match` in the one function that implements this table — the compiler, not a reviewer, asks for its row.

The initializer table, applied at every `Let` and `Assign` event:

| Value bound has provenance | Origin | Alias class |
|---|---|---|
| `Fresh` | `Owned` | its own (a new class) |
| `Whole(b)`, heap-typed | `Owned` (or `Unknown` when the store is refused) | its own — **`b` is handed over** (`check_transfer`, sink 4), so `a` is consumed and the new name owns the value |
| `Whole(b)`, bits (`int`, `float`, `bool`, `number`, `range`, options, `string`, `sensitive string`; `effective_ownership::value_like` is the one table) | `Owned` | its own — the bits are copied, nothing is handed over |
| `Reaches(roots)` | `Reaches` (never transferable) | joins every root's class — if a root is given away, this name is consumed with it |
| `Unknown` | `Unknown` (never transferable) | its own in the live scope; the persisted record (below) joins it to every binding the expression mentions, since an untraceable result may hold any of them |

The reassignment rule in one line: `rows = other` is a store, so `other` is handed to `rows`, `rows` leaves its old class and owns a new one, and reading `other` afterwards is `ConsumedByStore`. The consumed-read mechanism itself is a straight-line walk; a give carried into the next pass of a loop is a separate check (see "A give inside a loop" below).

`consumed: bool` becomes `consumed: Option<ConsumedBy>` with `enum ConsumedBy { Given { callee }, Sent { channel } }` (the cause fills the diagnostic; the consumed-read site selects the template by it) — one field, no parallel `Option<String>`. `Scope::consume(name, cause)` consumes the whole class. The `Stmt::Assign` arm of typeck (`check.rs:2436–2465`, today a pure type check on a non-`const`, non-parameter, non-loop `let` local) becomes the second caller of the ONE binding-event function `Stmt::Let` calls — never a re-implementation beside it.

### The transfer decision — ONE emit site

`check_transfer(expr, sink)` is the only function that decides a transfer and the only place the three diagnostics below are emitted. Every sink calls it; no sink inspects syntax:

- `Fresh` → admitted, nothing to consume.
- `Whole(name)` → read the entry: `const` → the existing cannot-give-const refusal (extracted into a helper with a sink-supplied WHAT-INSTEAD — a sender wants `{channel}.send({name}.copy())`, not "declare it with `let`"); already consumed → `report_consumed_read` renders it (`Consumed`, `ConsumedBySend` or `ConsumedByStore`, by cause), once per read span: the read check (`resolve_ident`, which checks the consumed state BEFORE an `is` arm narrows the name, and the read of `self`) reports every read it sees, and this sink reports the read it could not see, an operand inferred live and consumed by another operand of the same statement (`eatBoth(rows, [rows])`); `Param(m)` with `m != give`, or a class containing such a parameter → **`ParamNeedsGive`**; `Reaches` / `Cell` / `Unknown` origin → **`TransferNeedsCopy`**; `Owned` or `Param(give)` → consume the class with the sink's cause.
- `Reaches(_)` / `Unknown` → **`TransferNeedsCopy`**.

**The sinks this milestone wires to it** (each is a give position; the list is of SINKS, which is closed and small, not of argument shapes, which is not):

1. `channel<T>.send(v)` when `channel_elem_drop(T)` is `Some(kind)` and `kind.transfers_source()` — the owned-heap element kinds `array`/`map` ([`IMP-concurrency.md`](IMP-concurrency.md) "Which element types"). `int`/`float`/`bool`/`string`/`number` channels are copy-through and never reach the helper.
2. Every declared-`give` parameter position of every call form — plain call, monomorphized generic call, UFCS dot-call (**receiver AND non-receiver arguments**), and `dynamic Contract` dispatch — through ONE argument-list normalization (`[receiver, args…]` — the same normalization `background_spawn_call_form` and `collect_aliasing_in_expr` already perform; Phase 4 makes the three call paths share it rather than adding a fourth loop). A position is a give position when it is declared `give` OR `consumed[callee][i]` is true — the second case is what reports a relay chain in one compile (below).
3. A `background` argument the spawn gives away — **and this sink is the liveness INFERENCE, not a `check_transfer` call.** The inference in `check.rs::check_stmts` (recorded through `record_spawn_arg_ownership`) chooses `Give` or `Copy` for a `Whole(name)` binding the user never annotated; nothing it chooses can be reported as an error against a `give` the user did not write. **A `channel<T>`-typed argument never reaches this rule at all**: the earlier `BgOwnership::Channel` branch (inside `check.rs::record_spawn_arg_ownership`) claims it first and shares it (`emit.rs::prepare_bg_arg_for_ctx`, `ynz_channel_share`) — a `channel<T>()` constructor argument is `Fresh` → `Owned` under provenance, but that origin is never consulted here because the channel branch runs before this rule sees the argument (parked item 20). For every other type, the rule, restated class-aware: infer `Give` iff the binding's origin is `Owned` or `Param(give)` AND no member of its alias class is read in any remaining statement; otherwise infer `Copy`. A `Cell`, `Reaches`, `Unknown` or `Param(share|lend|bare)` origin therefore declines to `Copy` — **and "declines to `Copy`" means a `Copy` entry is still RECORDED for the argument, never nothing** (parked item 19): `for (row in matrix) { background producer(wire, row) }` is a correct program today (probe: prints `3`, `matrix` intact afterwards) and stays one because codegen's `is_heap_arg` gate (`emit.rs::prepare_bg_arg_for_ctx`) reads the PRESENCE of an entry in `background_arg_inferred_ownership`, not which variant it holds (`fr23_uaf_planned_red.rs:27` confirms the same reading) — an implementer who records nothing for a `Whole(name)` spawn argument hands the task a spawner-stack pointer instead of a heap copy (the fr23 use-after-free class). `bg_inferred` is not read only by the inlay-hint pass (`inlay_hint_passes.rs::collect_background_ownership_hints_block` is A reader, not the only one); codegen already copies by TYPE for a recorded `Copy`, so the decline changes no runtime behavior for that case — it only stops typeck from consuming a name the frame does not own, and makes the muted hint say "copied", which is what happens. The `Give` consume routes through the class, so a name that aliases the given binding is consumed with it — and because liveness is asked of the CLASS, `let other = rows; background f(rows); other.count()` infers `Copy` rather than consuming `other` under a program that runs correctly. The spawned call's own DECLARED `give` positions are sink 2 (`TransferNeedsCopy` on a loop cell there is correct: the callee's author wrote the word), because the `Expr::Background` arm runs `infer_expr` on the call.
4. A store of a heap value into a place — `list.add(v)`, `table.set(k, v)`, `x.field = v`, `x[i] = v`, a part of a container literal, `let b = a`, `b = a` — through `Checker::store_value` ("Stores are sinks" below). Its cause is `ConsumedBy::Stored`, its diagnostic `ConsumedByStore`.

**Stores are sinks; storing hands the value over.** Decided (Patrick, on the Chief Architect's recommendation, "deny and teach"; `git log --grep="hands it over"`). Putting a heap value into a place — `list.add(v)`, `table.set(k, v)`, `x.field = v`, `x[i] = v`, a container literal built from named values, `let b = a`, `b = a` — is sink 4 of `check_transfer`: the place becomes the value's one owner, the source binding is consumed, and reading it afterwards is `ConsumedByStore` (WHAT / WHAT-INSTEAD / WHY below; it suggests `.copy()` or moving the line above). A value that is not an owner refuses the store, as at any sink: a parameter without `give` (`ParamNeedsGive`), a piece, a loop cell or an untraceable value (`TransferNeedsCopy`), a `const` binding. Values whose copy is their bits (`int`, `float`, `bool`, `number`, `range`, options, `string`, `sensitive string`) are copied and unaffected. `Checker::store_value` is the one entry; which built-in methods store is one table, `builtins::collection_store_positions`; a literal hands over every named heap part (`hand_literal_parts_over`) and is then `Fresh`. A callee that stores a parameter must declare it `give` (its own body is refused otherwise), so a call never stores one argument into another behind the caller's back: there is no call-site store analysis and no second walk of the module. The loop-carried check below covers a store of an outer binding in a loop like any other give.

**Revive is path-aware.** A reassignment revives a given binding only for the code it dominates. At the join after an `if`, a match or a loop, the binding is live only if EVERY path that reaches the join revived it: each arm that does not always return, plus the path that enters no arm (an `if` without else, a loop that may run zero times, a match with no `else` that is not an exhaustive options/union match). Otherwise it is maybe given, and reading it is the ordinary use-after-give error (the canonical `Consumed` text, its `{via}` slot naming the path that did not reassign it: `ConsumedBy::missed_revive`, set at the join, rendered by `missed_revive_clause`); a give made anywhere on an arm persists past the join as before. One mechanism: `Scope::revive` / `begin_branches` / `begin_arm` / `end_arm` / `end_branches` extend the consumed state, no second tracker. The loop-carried check below reads the same state at the end of the body. The transfer record stays flow-insensitive: a binding given on any path keeps its `Given` escape after a revive, so a release pass leaks the revived value and never frees early.

**A give inside a loop must not reach the next pass.** A give of a binding declared outside the loop that is still consumed when the loop body ends is rejected with the `Consumed` template (the loop named in `{via}`); a reassignment before the back-edge revives the binding, a block that always returns never reaches the back-edge. Only a binding the next pass reads (by name, in the loop condition or body) counts; a give the loop does not carry passes to the enclosing loop, which checks it the same way; a `while` condition is part of the loop, so a give made in it counts. The `background` liveness inference reads an outer binding in a loop as live, so it infers `Copy`, not a second `Give`.

### The persisted transfer record

`TypedModule::transfers` (`crates/ynz-typeck/src/transfer_record.rs`) is the ONE record of who holds what; the live scope's classes die with their scopes, this keeps the final partition for the scope-exit release pass (C5–C8). Per function: alias classes whose members are keyed by `DeclKey` and the unique codegen name (`BindingNames`, C4), each with

- **role** per member: `Owner` (only ever bound to fresh values or a `give` parameter), `BorrowedParam` (`share`/`lend`/bare parameter), `Piece` (ever bound to a piece, a loop cell or an untraceable value);
- **anchor**: `Founder` = the owner whose scope exits last, earliest declared on a tie; `CallerOwns` (a borrowed parameter is in the class); `NoOwner` (all pieces: the allocation belongs to code this function cannot see); `NoStaticFounder(OutlivedByNonOwner)` (a non-owner outlives every owner, so which value it holds at the owner's exit is known only at run time — reported, never guessed);
- **escapes** on at least one path: returned, given, sent, or stored (`Escape::Stored { place }`: the place owns the value from then on, a temporary included). A store never joins two classes; classes join only where a name keeps reading a piece of another value (a field, an item, a loop cell, a call result that is a piece of its argument) or an untraceable result.

**The two anchors with no static release point are C7's, decided** (Software Architect ruling, plan #17 C5a). The record reports them; it does not release them. Each is pinned by a typeck-fact test now:

| anchor | what C7 does | typeck-fact fixture (`ynz-typeck/tests/transfer_record.rs`) |
|---|---|---|
| `NoStaticFounder(OutlivedByNonOwner)` | nothing is released (a leak, never an early free). A read that would outlive its root is a copy (a copy owner), and a call result is an owned value (`copied_at_return`, a channel included), so no program that builds reaches it today: the `maybe<channel>` reproducer now yields two owners (`a_maybe_channel_an_imported_function_returns_is_an_owner_in_a_class_of_its_own`). The anchor stays as the honest answer for a non-owner that outlives every owner; no drop flag (decided: Chief Architect) | none reaches it; the analysis is covered by the class-anchor tests |
| `NoOwner` | the temporary is released at the end of the statement that made it (a piece of a temporary, a loop cell of a temporary); no binding has a release duty | `a_piece_of_a_temporary_is_recorded_as_no_owner`, `a_loop_cell_of_a_temporary_is_recorded_as_no_owner` (an imported function's result of a copied-at-return type is owned by its binding: `an_imported_functions_result_is_owned_by_the_binding_that_holds_it`) |

`NoOwner` is released at statement end since slice 2; `NoStaticFounder` is never released (a leak, never an early or double free).

A hole in the recorder's own bookkeeping (an event with no function being walked or naming an undeclared binding, a binding with no codegen name, a class with no anchor candidate) is never skipped: it is kept as a string in `TransferRecord::internal_errors`, and `TypedModule::with_codegen_names` fails on it with the other record holes, so `codegen_query` reports the compiler-bug diagnostic and builds nothing.

Releasing a class must free every distinct object in it exactly once, container and stored elements alike (C6/C7); leaks are acceptable meanwhile, an early or double free never.

### The persisted release plan

`TypedModule::release_plan` (`crates/ynz-typeck/src/release_plan.rs`) is the ONE record of what each scope exit releases; C6/C7 emit releases from it and derive no transfer fact of their own (`authoritative-derivation.md`). It holds one **total** list per exit edge, every binding of a heap-capable type that leaves scope there, last declared first:

| edge | key | lists |
|---|---|---|
| end of a block (function body, `if` body, loop body per iteration, match arm) | the `Block` span | that block's frame |
| `return` | the `Stmt::Return` span | every open frame, innermost first |
| errors auto-propagation | the span of the identifier read that fires it | every open frame, innermost first |

Entries are keyed by `DeclKey` and the unique codegen name (`BindingNames`), never the user's name. A binding with no heap in its type (`int`, `boolean`, `options`, a `maybe` of those) is not listed.

**Fates and where each comes from** (one source per fact):

| fate | meaning | source |
|---|---|---|
| `Release(kind)` | sole owner, not moved on any path to this edge | transfer-record anchor + live scope |
| `ReleaseIfLive(kind)` | moved on some paths, or on every path for a reassigned owner (an assignment may revive it): release if the slot is non-null; every move site nulls the founder's slot (`MoveSite`) | live scope (`ScopeEntry::consumed_on_every_path`), or `string_moves` for strings |
| `Omitted(Moved)` | given, sent or stored on every path | live scope |
| `Omitted(Returned)` | the returned value is or reaches its class | the returned value's holders |
| `Omitted(NotOwner / CallerOwns / NoStaticFounder)` | another member founds the class; a borrowed parameter is in it; a non-owner outlives every owner (an imported `maybe<channel>` result; nothing is released) | transfer-record anchor |
| `Omitted(TypeParameter)` | the value's type is, or holds, a type parameter of a generic function: the body alone cannot say whether it owns heap | `heap_class_of` |

**Every binding whose value owns heap is listed, whatever the kind** (`heap_class_of`, one type fact): `string`, `array`, `map`, `channel`, background task handle, a flat shape with heap fields, a shape holding a shape (each shape-typed field is a heap cell; a recursive or undescribed shape is assumed to own heap), `maybe` / union / `fixed` with a heap-owning content, `dynamic`, a map entry with a heap-owning key or value, and every errors-capable value (the error object is heap). A type that cannot own heap (`int`, `boolean`, `options`, a shape of ints, a `maybe` of bits) is not listed. How codegen releases each kind is C6/C7's job; the record only has to be complete, and the corpus parity test rejects an omission with no reason. Generic bodies are checked once, so their lists apply to every instance.

The crossing set reads only the kinds with a frame slot across a suspension today (`ReleaseKind::has_frame_backing`: string, array, map, channel, handle). A flat shape is left out as well: a shape local that crosses a `wait` inside an `is` arm of a state-machine match read freed frame bytes (`v0_3_m9_union_narrow_sm_host`, optimized). The others cannot cross a `wait` yet (Check 2/2b), so reading them would reject programs that compile now; C6 widens that one predicate together with the frame backing of the kind it starts releasing in a state machine.

**Strings.** A string is never statically moved: `string_moves` says a read MAY be taken by a store, and codegen decides at the store site. A `let`/`const` string with a record-listed last-use read that can precede the edge is `ReleaseIfLive`, otherwise `Release`; the listed reads are its move sites, so the plan and `string_moves` are one fact. A string given to a `give` parameter is handed over like any given value (the checker refuses a later read): the caller omits it as moved (`Moved`, or `ReleaseIfLive` when only some paths gave it) and the callee, which owns what it was given, releases it, so exactly one side does. A `share`, `lend` or bare string parameter stays the caller's. Only a string that founds its own class follows the give; a string joined to a piece's class owns a copy the class's give does not take, so its own scope releases it.

**Crossing set.** A release is a read at its edge. `crossing_local_names_with_provenance` scans the statements with those reads added (`ReleaseReads::with_reads`), so an owner crosses a suspension exactly when one separates its declaration from its release, by the one suspension detection; the typeck guards, the admission and M3d probes and codegen's frame layout pass the same reads (user names in typeck, codegen names in codegen). The user-name reads skip an owner whose user name the function declares for two or more bindings (sibling blocks reuse names): the crossing producer and its guards are keyed by the name and would judge it by another binding of that name, rejecting a program that compiled before the plan. The skip is deliberate and loses no guard verdict, because the skipped kinds (string, array, map, channel, handle) cross a `wait` legally; codegen's names are unique, so its reads and frame layout keep every owner (`resolve_function`, `ambiguous_user_names`). A move is a write to the founder's slot (`collect_crossing_writes`).

**Parity.** `ReleasePlan::parity_violations` checks, per edge and founder, the fate against `Scope::consumed_classes` at the edge and the transfer record's escapes (`typeck/tests/release_plan.rs`, over every fixture and example).

### The one release helper

`Cg::release_fn` (`ynz-codegen/src/emit.rs`) is the one place a heap value is freed: it emits one module function `ynz_release_<type>` per type, from `release_step`, an exhaustive match over `Type`. It frees exactly what a copy of the value allocates (`emit_owned_copy`, whose shape copy is the one seam `deepen_shape_fields`), so a value freed once per holder balances.

| value | what its release frees |
|---|---|
| `string` | its bytes (`ynz_string_release`; a static string is left alone) |
| `array<T>` | each item by `T`'s release, then the header and buffer (`ynz_array_release`) |
| `map<K, V>` | each occupied entry's key (a string key is a raw pointer the map owns) and value, then the buffers (`ynz_map_release`) |
| shape | each field by its type: strings, arrays, maps, a nested shape (its heap cell: contents, then the cell), a `maybe` (its heap envelope cell), a union slot, a channel (one counted reference) |
| `maybe<T>` | the payload a copy allocates (a string, a shape's heap cell, a `number`'s cell); in the cell flavor, the envelope cell. An array or map payload is not the envelope's (copy refuses it) |
| union | an envelope union is a slot: a shape variant's fields (inline) or its box cell (recursive union), a string, array or map variant; any other union is a `{tag, data}` view |
| `fixed<string>`, `fixed<Shape>` | each cell's item (a string, or a shape's heap cell and what it owns); any other heap item of a `fixed` is an internal error (the copy table refuses it) |
| channel, task handle | one counted reference (`ynz_channel_free`); the handle aborts its task (`ynz_handle_free`) |
| errors-capable | the error object, or the ok payload |
| `dynamic`, type parameter | internal error: no such value exists in generated code |

Replacing a stored value frees the one it replaces (`set`, index assignment, a field assignment, a `fixed` `set` or index assignment, a map `set` over a present key, whose handed-in key copy is freed too). `emit_errors_capable_release`'s ok payload and a plain union view release through the same `stored_release`: one dispatch decides what a stored value's release is. The `background` task ladder releases a task's own value with this helper (`BG_ARG_KIND_RELEASE_GLUE`, the descriptor's `size` is the glue's address); a channel's element drop glue is the same function. A callee with any `give` parameter keeps the flat free for every argument (deferred, see "Non-owned reads"). Calling the helper at scope exits is C7 (plain locals) and C8 (suspension frames).

### Scope-exit release on the plain path (C7)

A plain (non-suspending) function releases its heap locals at every exit edge of `TypedModule::release_plan`, through ONE emitter, `Cg::emit_release_edge` (`ynz-codegen/src/emit.rs`), which calls `Cg::release_fn` (the C6 helper) for each entry whose fate releases. Suspension frames stay C8's: a state-machine resume function emits no scope release.

**What is released** (`plain_releases_kind`, exhaustive): string, array, map, channel, task handle, flat shape, nested shape, `maybe`, envelope union, `fixed` list, errors-capable value and an admitted-SoA array (by field segment, `emit_soa_release`). Not released: `dynamic` and a map entry (never an owner of a slot). A union of heap variants with no shape variant (`string | array<int>`, `string | channel<int>`, and `string | int`) is a tagged union like any envelope union (`is_envelope_union`, `IMP-unions.md` "Heap-variant unions"), so it releases through the one helper by the variant its tag names (C7 slice 4); a union with one variant beside `nothing` (`map<..> | nothing`) is still that variant's raw pointer and is classified as that variant. There is no `UntaggedUnion` kind.

- **Edges.** Block end (function body, `if` body, loop body per iteration, match arm), `return`, auto-propagation, each read by span from the plan; the value of a `return` is fully built before its edge runs.
- **Fates.** `Release`: load the binding's slot, call the helper. `ReleaseIfLive`: the same behind a null check; every plan `MoveSite` nulls the founder's slot when the value is taken (a `give`, a send, a store, a string taken by the store seam). A last-use string passed to a call that only borrows is not a take: the slot stays and the scope releases it.
- **No drop flags.** A read that would outlive its root is a copy, so no class has a holder that outlives its owner (decided: Chief Architect, replacing the drop-flag ruling for `OutlivedByNonOwner`; a flag cannot make a view of a freed value sound). A call result is an owned value: a function copies at its `return` what does not move (`copied_at_return`), and for a channel that copy is a counted reference, `ynz_channel_share`, the count channels already have by design, as Rust's `Arc::clone` (no new counting mechanism, B4 "no garbage collector or reference counting for sequential code"). **The share is taken by the callee's `return`** (a parameter, an element or a field is shared; an owned local at its last use hands over the reference it holds), and the caller binds the result by move, so each reference has one owner and is given back once: by a binding's scope exit, a temporary's statement end, or the caller's own `return`. Decided: the Software Architect, within the Chief Architect's counted-reference rule (which first put the share at the holder of an untraceable result; that left an owned local a callee returned with an orphaned reference, and a `maybe<channel>` an imported function returned with no founder). A local `maybe` of a channel releases its payload (`ynz_channel_free`); a `maybe` in a shape field or a list does not, so its copy and release stay what C6 made them. An IMPORTED callee's result of a copied-at-return type is owned by the same reading of its declared return type. A task handle cannot be named in a signature, so no function returns one. A union that holds a channel (`string | channel<int>`) owns one counted reference like any channel holder: a wrap or a copy of its slot shares it (`ynz_channel_share`), its release gives it back once (C7 slice 4).
- **Unbound temporaries (`NoOwner`).** A fresh value no binding holds is released at the end of its statement.
- **Reassignment.** The old value is released at the assignment, behind the same null check. A reassigned binding copies at every place read (its `let` is a view only while nothing writes the binding, and the reassignment is that write; `non_owned_reads` never judges a reassignment a view), so it owns every value it holds and nothing it replaced leaks. An errors-capable local releases its old result through its original slot: the old words are snapshotted, the new result is copied into a region of the assignment's own (a call's result slot is per call site), and the original slot is rebound (`assign_errors_capable`).
- **Early exit from a loop.** A `return` or an auto-propagation inside a plain `for` body releases the copies of the loop variable (array, `fixed` and map-entry loops) that the iteration end would have released: `Cg::open_loop_copies` holds them while the body is lowered, `release_at_return` and `release_at_propagation` release them through `release_owned_read`, the helper the iteration end uses. A state-machine loop copy stays C8's.

**Move sites, temporaries, owners and returns.**

- **Move sites carry a cause and a kind.** `MoveSite { founder, founder_codegen_name, kind, cause }`. `cause` is `Handed` (a read typeck consumed: give, send, store, spawn give; the value always leaves the founder) or `LastUse` (a string last-use read: it leaves the founder only if the store seam takes it). Only a founder whose kind the plain emitter releases (string, array, map, channel, handle, flat shape) has sites.
- **Where the slot is nulled.** `Handed`: the identifier read itself (`lower_expr_node`, `Expr::Ident`), so every sink (call argument, send, spawn, store) is covered by one hook. `LastUse`: `own_string_for_store` and `own_read_for_store` when they take the string instead of copying it, through the slots remembered for the loaded value. A last-use string passed to a call that only borrows is not a take: its slot stays and the scope releases it. A state-machine function nulls nothing (C8).
- **`ReleaseIfLive` is "release if the slot is non-null"** for every kind. `StringTakeEvidence` is gone.
- **Reassignment.** `Stmt::Assign` releases the old value when its binding is a released owner. Order: evaluate the value, load the old slot value (a `RHS` that took it already nulled it), store the new value through the store seam, then release the old one; a flat shape snapshots its old bytes first because its per-site region is reused. A reassigned owner is released at its exits like any owner, and `Moved::EveryPath` becomes `ReleaseIfLive` for it (a later revive may restore a value). The plan lists the assignments that release (`ReleasePlan::assign_release`).
- **`give` parameters.** The callee releases a `give` parameter at its scope exits like an owner; the caller's slot is nulled at the give. The spawn ladder does not free an argument at a `give` position (the callee owns it); other positions release deep.
- **Temporaries.** `TypedModule::temporaries` (`ynz-typeck/src/temporaries.rs`) names each fresh heap value no binding holds (a Yinz call result, an array, map or shape literal, a fresh string, `.copy()`) and the release point it dies at: the statement, a `while` condition, or after an `if`, `match` or `for` as a whole. A value that enters a union slot, a `give` or store position (a map `table[key] = v` key included) and the piece of a `maybe` or union read are not temporaries. Codegen opens a frame per statement (`Cg::open_temporaries`): one entry-block slot per temporary, nulled at the statement's start; `lower_expr` stores the value into its slot where it is built (`bind_temporary`); the statement's end, a `return` and an auto-propagation release every non-null slot through `Cg::emit_slot_release`, the one emitter that `emit_release_edge` also calls. A state-machine function opens no frame (C8).
- **Take.** A string, array, map or nested-shape field read off a call result and kept by a `let` or assignment (`ReadVerdict::Take`) is taken without a copy: `lower_taken_field` loads the field, nulls it in the temporary's region and marks it moved; the temporary's release skips the null field. A nested shape is a heap cell, so its bytes move into the binding's region and the emptied cell is freed (`shape_bytes_to_owned`, as `maybe_to_owned_taking` does for a returned payload), and the cell flavor of a shape release returns on a null cell ONLY for a shape some `Take` of the module moves out (`NonOwnedReads::takes_nested_shape`); every other cell release frees unconditionally, as Rust's `Box` drop does, and `lower_taken_field` refuses to null a cell whose release does not check (pinned by `v03_m9_c7_slice4`). Under its own name only: a union or alias annotation wraps the value, which stays a copy. The binding owns the taken value: the checker's binding event and the `Take` verdict read the one predicate `non_owned_reads::is_taken_piece_of_a_temporary`. Pinned by `v03_m9_c7_slice2` (`binding_a_piece_of_a_fresh_temporary_takes_it_without_a_copy`: 2 allocations, 2 frees) and `v03_m9_c7_slice3b` (`a_nested_shape_taken_from_a_temporary_is_not_copied_and_balances`: 0 extra cells or strings).
- **Copy owners.** A binding whose every event is fresh, a `give` parameter or a read typeck judged `MustCopy` owns its value and is released like any owner. The record keeps the value span of each event and applies the joins when it is built (`TransferRecorder::finish` takes `non_owned_reads`): a `MustCopy` event joins no class and makes no piece. A `Take` is bound as a fresh value by the checker only where no name reaches the temporary. The `Take` verdict agrees: a call result that may be a piece of its arguments (a generic callee returning one of them) is not a temporary to take from, so a field read off it copies and the binding owns the copy (`ReadTracker::takes_a_piece_of_a_temporary`). The early release reads (suspension guards) are built before the verdicts and the final reads must cover them.
- **Every owner founds its own allocation.** `found.or(backup)` joins `backup` to the class of `found` (the result may reach either). `AliasClass::anchor_for(member)` makes every `Owner` of a class its own founder, with or without escapes; a `Piece` whose scope strictly encloses an owner's may still point into it, so that owner keeps the class anchor (nothing is freed early). A fate reads what happened to the binding's OWN value: the live scope's consumed state is per class, so it counts only where a read naming this binding gave, sent or stored it (`MemberFacts::moved_directly`), and a move site nulls only its own binding's slot. A dot-call reaches a user function only on a `Shape` or `dynamic` receiver (`provenance`, as `is_fresh_string_expr`), so a user function named `or` does not make the builtin `or` read as `Fresh`.
- **Returns.** A string, array, map, shape, channel, `maybe` of a channel or envelope union that a `return` does not move is copied at the `return` (a channel by a counted reference; a union by the return's own persist, deep unless the value moves), so it marks no binding returned; any other type keeps the class spread (`Checker::returned_holders_of`). The returns_fresh fixpoint reads a `let`/assignment of a fresh local as a hand-over (the new name is the fresh one), so a union built through an intermediate `let` and returned is `Fresh`: its caller binds it by move and nothing is copied twice or orphaned (`a_union_returned_from_a_local_or_a_parameter_is_owned_by_its_caller_once`). A call result of a union type is a temporary like any other (`temporaries.rs` releases it at the statement end).

**Slice 3 forms.** A local `maybe` is an envelope in an entry-block slot (its shape and number payloads sit in entry-block slots, its array and map payloads are the binding's own); a union binding is a VIEW of a slot it owns; both release through `local_release_fn` (`ReleaseForm::Local`). A binding takes the payload a returned `maybe` carries (`maybe_to_owned_taking`). `begin_assign_release` snapshots the old inline storage of a flat shape, `maybe`, union or `fixed` list before the new value reuses its per-site region.

**Parked, with the reason.**
- *`is` on a non-shape variant* (`is string =>`) does not parse or resolve (`check_is_arm_pattern` names shape variants only), so a heap-variant union is held, passed, returned, stored and released but its payload cannot be read back yet (#418).
- *Heap-variant unions as container elements or map values* (`array<string | array<int>>`) cannot be written: a union type does not parse inside a type argument.
- *A decimal key and a decimal value sub-cell of a spawned `MapEntry` leak* (the accounted persist-cell class of FRAGO 011; the key half is now copied, `value_to_stable_bits`). TRIGGER: the drop story for spawn arguments.

Pinned by `v03_m9_release_safety` (ASan, both optimization levels, counters) and `v03_m9_c7_*`.

**Hand-over to C8 (#385: suspension frames).** A state-machine function emits NO release today, so every item below is a leak on that path and never an early or double free:
- *State-machine functions.* A resume function emits no scope release, no statement-end temporary frame (`open_temporaries`), no slot nulling at a move site and no loop-copy release. C8 owns the frame slot of each kind it starts releasing; `ReleaseKind::has_frame_backing` (string, array, map, channel, handle) and the crossing set widen together with it, never apart (a flat shape that crosses a `wait` inside an `is` arm already read freed frame bytes once).
- *Suspending-call argument copies.* The copy of an argument staged into a call that suspends is released at the frame release (the deferral under "Non-owned reads"); no frame slot holds its pointer today.
- *Loop copies read before a `wait`.* The copy of a loop variable or map entry that a suspending body reads before its `wait` and not after it, is released at the frame release for the same reason; a write after the read is what makes the copy.
- *The state-machine union spawn string leak.* A union argument of a state-machine task is staged into a heap cell the task ladder frees flat: its string field is never freed (probe: 20 spawns of a `Circle | Tag` holding a string allocate 40 strings and free none, the spawner's and the task's).
- *Handle abort semantics.* A handle local's release is `ynz_handle_free`: it aborts the task (delivered at the child's next suspension), frees the conduit reference and the result buffer, each once. A plain function that holds a handle therefore aborts an unfinished task at its scope exit. Whether a scope exit should abort, join or detach a task that still runs is C8's decision; the typeck fact (the handle's class and escapes) is already recorded.

### Non-owned reads — view or copy

A read of a place someone else owns (`b.rows`, `rows[i].or(d)`, a loop's element, a call's argument) has two honest forms. A **view** points at the place: no allocation, as a Rust borrow. A **copy** owns a value of its own. Overwriting a place releases the value it held (the helper above), so a view that is still read after its place is written reads freed memory. Typeck therefore decides, per read, and codegen copies unless typeck proved a view. **Cost: no write while the reader lives means no copy; a write means one copy per affected `let`, loop iteration or call** (what Rust's `.clone()` costs where its borrow checker refuses a borrow), so Golden Rule 8 holds and the language gains no rule. Pinned by `v03_m9_c6_borrow_cost`.

**The record.** `TypedModule::non_owned_reads` (`crates/ynz-typeck/src/non_owned_reads.rs`) maps the span of the expression that reads the place to `View` or `MustCopy`. It is built in the ONE liveness walk of `string_ownership` (the same `after` sets as `string_moves`); nothing re-derives it. These sites are judged:

| site | reader | key | judged types |
|---|---|---|---|
| `let` value, reassignment value | the binding | the value expression | array, map, shape, union, `maybe` of those |
| `for` loop variable (array, `fixed`, map key and value) | the variable, live for the whole body | the iterated expression | string, array, map, shape, union |
| call argument at a position the callee only reads | the callee's parameter, live for the call | the argument | the same |
| `return` | none: it outlives its frame | no entry | string, array, map, shape, channel, `maybe` of a channel, envelope union (`copied_at_return`) |

**The copy sites, for C10's Golden Rule 8 check.** These are every place a non-owned read can allocate a copy. Each has a no-write case pinned at zero extra copies and a write case pinned at one, in `v03_m9_c6_borrow_cost` (`crates/ynz-driver/tests/`).

| copy site | codegen | no-write pin (0 extra copies) | write pin (1 copy) |
|---|---|---|---|
| `let` of a place | `own_read_for_store` | `a_let_of_a_field_nobody_overwrites_copies_nothing` | `a_let_of_a_field_that_is_overwritten_copies_it_once` |
| reassignment of a place (a reassigned binding owns every value it holds, so it copies even when nothing writes the place) | `own_read_for_store` | n/a: always one copy | `a_reassignment_from_a_field_nobody_writes_copies_it_once_and_balances` (1 copy, 6 allocations, 6 frees) |
| `let` or reassignment of a nested-shape field of a call result: **moves**, no copy (`ReadVerdict::Take`) | `lower_taken_field`, `shape_bytes_to_owned` (frees the emptied cell) | `a_nested_shape_taken_from_a_temporary_is_not_copied_and_balances` (0 extra cells, 0 extra strings) | n/a: a temporary is never written |
| `let` of a `maybe<array>` or `maybe<map>` payload | `copy_maybe_container_payload` | `a_let_of_a_maybe_array_element_nobody_writes_copies_no_payload`, `a_let_of_a_maybe_map_element_nobody_writes_copies_no_payload` | none: shares the `let` rule |
| loop variable (plain and state-machine) | `own_loop_element` | `a_loop_whose_body_writes_nothing_copies_nothing`, `a_state_machine_loop_whose_body_writes_nothing_copies_nothing` | `a_loop_that_overwrites_an_item_copies_one_item_per_iteration`, `a_state_machine_loop_that_overwrites_an_item_copies_and_releases_one_item_per_iteration` |
| map-entry and union loop variable | `own_map_entry`, `own_loop_element` | `a_map_loop_whose_body_writes_nothing_copies_no_entry`, `a_union_loop_whose_body_writes_nothing_copies_no_item` | none: shares the loop rule |
| call argument (a `share` or bare position) | `lower_call_argument` | `share_arguments_with_no_lend_of_their_collection_copy_nothing` | `a_share_argument_read_from_a_lent_collection_is_copied_once_and_released` |
| UFCS receiver (argument 0) | `lower_call_argument` | `a_ufcs_receiver_nobody_writes_is_not_copied` | `a_ufcs_receiver_read_from_a_lent_list_is_copied_once_and_released` |
| argument staged into a suspending callee's frame | `lower_call_argument` | `an_argument_of_a_suspending_call_nobody_writes_is_staged_without_a_copy` | released at C8's frame release (deferred below) |
| `return` of a non-owned place | `own_returned_value` | `a_returned_field_is_copied_once_and_the_caller_binds_it_without_a_second_copy` (the one copy is at the `return`) | the same |
| `let` or reassignment of a place whose root dies before the reader (the reader is a `let` declared outside the root's block) | `own_read_for_store` | `a_read_that_dies_with_its_roots_scope_copies_nothing` | `a_read_that_outlives_its_roots_scope_is_copied_once` |
| `let` or reassignment of a call result typeck cannot trace (`Provenance::Unknown`: an imported function that returns a `maybe` of anything but a channel, or a union, which may be a piece of an argument; a union with a shape variant is an owned return, below) | `own_read_for_store` (`copy_maybe_container_payload`, `maybe_to_owned`) | n/a: a `let` of an untraceable result always copies | `an_imported_maybe_result_is_a_copy_the_argument_owners_release_does_not_dangle` (1 copy per `let`) |
| `return` of a `channel` or a `maybe` of one that does not move (a parameter, an element, a field): **a counted reference**, not a copy; the caller moves the result | `own_returned_value` (`share_returned_channel`, `ynz_channel_share`, the one call C6's deep copy of a shape also makes) | n/a: always one reference per such `return` | `a_channel_handed_back_by_a_function_is_held_by_a_counted_reference` (one `ynz_channel_share` per function that returns a channel it does not move), `a_returned_channel_or_maybe_channel_is_one_counted_reference_given_back_once` |
| `return` of an envelope union that does not move (a `share` parameter, a field): the return's own deep persist | `union_persist_into` at the `return` (the store seam skips a union there) | a moved local is taken: `a_union_returned_from_a_local_or_a_parameter_is_owned_by_its_caller_once` | the same (one copy per non-moving `return`) |

A shape or `fixed` copy is deep at `deepen_unless_moved`; a value `heap_moves` marks as moved, and a `View`, cost nothing (`v03_m9_c6_shape_deep_copy`).

A read is a `View` when (a) the reader is never written, stored, returned, given or sent, and (b) no root of the place is written, lent, given or released anywhere in the reader's live range: for a loop variable the whole body, a `lend` callee included; for an argument, any other argument of the same call (`both(items[0].or(d), items)` copies the first argument); a parameter the function only reads satisfies (b) for the whole body. (c) A `let` or reassignment view never crosses a suspension (it may point at per-site staging); a loop variable or argument does not end at a `wait`, only at a write. A reassigned binding never points through another reader (a loop variable's staging). `.or(default)` reads the place or the default: both are roots. (d) A `let` or reassignment view never outlives its root: a root declared directly in a block dies at the block end, where the scope exit releases it, so a reader declared outside that block owns a copy (decided: Chief Architect, replacing the drop-flag ruling for `OutlivedByNonOwner`).

**The seam.** `own_read_for_store` (`ynz-codegen/src/emit.rs`) is the one store seam for a string, array, map, shape and envelope union: a value typeck says moves (`string_moves`, `heap_moves`) is taken, a `View` stays a pointer, anything else is copied by `emit_owned_copy` / `shape_bytes_to_owned` / `union_persist_into`. `let`, reassignment, loop variables (plain and state-machine, `own_loop_element`, `own_map_entry`), call arguments (`lower_call_argument`) and returns (`own_returned_value`) read the record through it. A `maybe` of an array or map copies its payload too (`copy_maybe_container_payload`). A loop variable's copy is released after its iteration and an argument's copy after the call, both through `release_fn`.

**Returns.** A function returns an owned value: a fresh value or an owned local at its last use moves, any other read (a parameter, a field, an element) is copied at the `return` (a channel by a counted reference). The caller binds a call result by move (`Provenance::Fresh`), so a returned copy is not copied again. The same copy closes a task's return of its own argument, which the release glue frees at retire.

**Deferred to C8** (none is a use-after-free: each is a leak the frame release closes; the two C7 deferrals that stood here, the flat free of a `give` callee's arguments and the overwrite release of a `fixed` list of unions, were closed by C7 slices 2 and 3):
- *Suspending calls and loop bodies.* WHAT: an argument copy of a call that suspends, and the copy of a loop variable (or map entry) that a suspending body reads before its `wait` and not after it, are released at C8's frame release. WHY: no frame slot holds their pointer and a slot is a frame-layout change C8 owns. COST: one copy leaked per such call or iteration, only when a write follows the read. TRIGGER: C8 (#385) frame release.

### The parameter rule and how far it travels (FRAGO 007, re-derived)

A whole binding that is a parameter transfers only if declared `give`. With `give` declared, the EXISTING call-site path consumes the caller's binding (sink 2) at every call and every spawn — one mechanism. The obligation therefore transits the whole chain: A relays to B relays to C, C sends; C's parameter needs `give`; B passes its parameter to a give position, so B's needs `give`; up to the frame that owns the value as a local. `share` and `lend` parameters are refused by the same test — they are explicit statements that the caller keeps the value.

**Every frame of the chain is reported in ONE compile.** The earlier draft reported one frame per compile on the stated premise that typeck lacks a callee-before-caller ordering. The premise is false: the fixpoint IS that ordering. `consumed[B][i]` is true as soon as B's body sends its parameter, whether or not B declares `give`; A's call `B(x)` therefore sees a give position, and if `x` is A's own undeclared parameter, A gets its `ParamNeedsGive` in the same compile as B. For a LOCAL `x` at A, the call also consumes `x` in that compile (so A's later read of it is reported now, not after B is fixed) — the program is already rejected by B's error, so this consumption changes only what is reported, never what is accepted; the accept/reject decision stays signature-driven, which is why this is not the "infer `give` for bare parameters" alternative FRAGO 007 rejected (that alternative would have accepted a program whose caller's signature says nothing). Each error is independently actionable — one word in one signature — so N hops cost one compile, not N.

### `dynamic Contract` dispatch — covered by construction

A contract method's parameters carry ownership modifiers in the AST (`ContractSig.params: Vec<Param>`, each with `ownership`; the receiver's `ReceiverKind` too). **They are OPTIONAL there, exactly as on a function with a body** — the parser's contract-signature loop accepts a bare position (`parser.rs:3996–4012`, `_ => None`); the rule is the one the rest of the language already has: **a bare position is never a give position**, whether on a function (FRAGO 007 — `give` is written, never inferred) or on a contract (signed 2026-09-03, packet item (j) — `audit.md`'s SIGN-OFF record). A bare contract position is read-or-lend by the implementer's effective ownership, which is what the fixpoint computes for the implementing function; only a written `give` makes the dispatch site a transfer sink. `follows` parity is EXACT at the declared level for every non-receiver position — bare matches bare, `give` matches `give` — so an implementer cannot declare `give` where the contract is bare (the vtable would promise the caller keeps the value and the implementation would free it) nor the reverse. The receiver is checked for the half the parser lets it express: an explicit contract receiver modifier must match the implementer's exactly, and a bare contract receiver is never a give position (the implementer may not declare `give self`); the parser folds a bare contract `self` and "no self" into the same `receiver: None`, so "bare matches bare" cannot be distinguished from "no receiver" there and only the safety-relevant direction is enforced (SIGN-OFF item (j), `audit.md`). No parser enforcement is added. `ContractSigDef` (`shapes.rs`) carries `param_ownerships` (the declared modifier of each non-`self` param) and `receiver` (the declared receiver kind); the dynamic-dispatch site (`check.rs:~6153-6209`) builds the same normalized `[receiver, args…]` list `check_call_transfers` (`check.rs:5320`) runs for every other call form, and calls it with the contract's declared modifiers — the only static truth for a runtime-resolved callee; `follows` conformance checks that the implementing function's modifiers equal the contract's — otherwise the vtable would promise `give` and the implementation take `share`. Runtime exposure today is nil: codegen refuses every dynamic-dispatch call site ("dynamic dispatch call sites not yet lowered in M4 P4", confirmed by probe), so this closes a typeck hole before it can be reached, not a shipped crash. The `effective_ownership` fixpoint keys callees by name because the signature table does (`sig_table.fns` is one entry per name — no overloading is implemented); if overloading ships, both key by signature together.

### What this makes sound, and what stays outside

With every sink threaded through `check_transfer`, a transferred allocation has exactly one source-level holder at every moment: the owner (and its alias class) until the transfer, then the sink. The receiver of a channel is the sole reference; a `give` callee is the sole reference; a `background` task's ladder-owned clone is released by the shipped runtime protocol when the task sends it on ([`IMP-concurrency.md`](IMP-concurrency.md) "Two mechanisms, one rule" — unchanged by this section, and still not redundant with it: it answers allocation-level ladder ownership; this section answers source-level readability).

Outside the guarantee, named: relaying a RECEIVED owned-heap value — `let got = wire.receive()` then `other.send(got.value)` — where `got.value` is a piece of `got` (a `maybe<T>` field access, `Provenance::Reaches`), so `TransferNeedsCopy` fires and the relay pays `.copy()` this milestone (packet item (g), signed 2026-09-03).

**Two doors this section used to name as outside the guarantee are closed** (v0.3 concurrency
hardening Phase 3, FRAGO 002 cluster C2 — M8 Future Requirements #9 and #10, one fix at one
producer). Both came from the same design gap: two per-type dispatches answering "give me an
independent copy of this heap value", agreeing only by comment, and both defaulting to handing
back the receiver's own pointer.

- **FR#9's container door.** `bucket.add(rows)` into an aliased container was reachable because
  an `array<pointer-elem>` `background` argument was passed through un-cloned. Such an argument
  is now copied — items included — so the task's container and its rows are its own, and the
  ladder can no longer free something the spawner still points at.
- **FR#10's `.copy()` types.** `maybe`, union, `fixed`, `dynamic` and the rest no longer sit in
  an alias arm. Each is either a real copy or a compile-time refusal (`ynz_typeck::owned_copy`),
  so there is no type whose `.copy()` compiles into an alias for provenance to hold at arm's
  length.

**Deferred (four fields, packet item (g)): a consuming move-out-of-`maybe<T>` form.**
- **WHAT**: a consuming accessor on `maybe<T>` — some form that yields the wrapped value as `Provenance::Fresh` (moving it out of the `maybe` rather than reaching through it), so a relay of a received owned-heap value would not need `.copy()`.
- **WHY deferred**: the alternative to accepting the `.copy()` cost this milestone is a language-wide `maybe` semantics addition, which is outside a concurrency milestone's charter — `maybe<T>` is used everywhere in the language, not only at channel receives, so a consuming accessor's design (naming, what happens to the `maybe` binding afterward, whether it generalizes to every `maybe`-producing site) is its own design pass, not a corollary of the transfer rule.
- **COST to add later**: a `maybe` design addendum (the accessor's name and semantics), one new `Provenance` arm for it, and one codegen path to move the wrapped value out instead of reading through the envelope.
- **TRIGGER**: a measured relay workload where the extra `.copy()` per hop is a real cost, or the general move/drop story (Track 3 / FR 7(2)) landing and settling what "consuming" means for `maybe<T>` alongside every other heap type.

### Teaching text (Golden Rule 11; four diagnostics, one emit site)

**`ConsumedBySend`** — unchanged from Phase 1's drafting except one slot: because consumption is class-wide, the read name may differ from the sent name. `{name}` is the binding read, `{channel}` the channel; when the name read is not the name sent, `{via}` renders ` — it shares its value with \`{sent}\`, which is what was sent`.

> **WHAT**: `{name}` was sent into `{channel}` and cannot be used here — `send()` gave it away{via}.
> **WHAT INSTEAD**: If you still need `{name}` after sending it, send a copy instead: `{channel}.send({sent}.copy())`. If you only need it before the send, put this line above the `send()`.
> **WHY**: A channel hands the value to whichever task receives it, and that task may already be reading or changing it by the time this line runs. If both sides kept the same value, two tasks could change it at once and neither would know. So `send()` gives `{sent}` away, and the compiler refuses to build any line that reads it afterward. Nothing happens at runtime — `{name}` is not cleared or set to `none`; the program simply does not compile until this read is placed above the `send()` or the send takes a copy.

**`ConsumedByStore`** — the read of a binding a store handed over. `{name}` is the binding read, `{place}` the place with its backticks (`` `list` ``, `` `x.field` ``, the literal `` `[a, c]` `` or `` `{ first: a, second: c }` ``, `` `b` ``), `{stored}` the binding named at the store, `{copy_form}` the same store written with `.copy()` (for a literal, the whole literal: `{ first: a.copy(), second: c }`), `{via}` as in `Consumed` (` — it shares its value with `` `{stored}` ``, which is what was stored`).

> **WHAT**: `{name}` was stored in {place} and cannot be used here — storing a value hands it over{via}.
> **WHAT INSTEAD**: If you still need `{name}` after the store, store a copy instead: `{copy_form}`. If you only need it before the store, put this line above the store.
> **WHY**: Every value has one owner. Storing `{stored}` in {place} makes {place} its owner, and when {place} is done with it, it frees it. If `{name}` could still read the same value, two names would reach one value and one of them would free it under the other. So a store hands the value over, and the compiler refuses any line that reads `{stored}` afterward. Nothing happens at runtime — `{name}` is not cleared or set to `none`; the program simply does not compile until this read is placed above the store or the store takes a copy.

A refused store (a parameter, a piece, a loop cell) is `ParamNeedsGive` / `TransferNeedsCopy` with `{act}` = `handed to {place}` and `{copy_form}` the store written with `.copy()`.

**`ParamNeedsGive`** — `{name}` the parameter, `{fn}` its function, `{type}` its declared type, `{modifier}` one of `has no ownership word` / `is declared \`share\`` / `is declared \`lend\``, `{act}` one of `sent into \`{channel}\`` / `given to \`{callee}\`` / `given to \`{callee}\`, whose \`{param}\` parameter gives it away`, `{copy_form}` the matching `{channel}.send({name}.copy())` / `{callee}({name}.copy(), …)`:

> **WHAT**: `{name}` is a parameter of `{fn}` that {modifier}, but this line gives it away — it is {act}. Only a `give` parameter can be given away.
> **WHAT INSTEAD**: Declare it `give` in `{fn}`'s signature — `function {fn}(give {name}: {type}, …)` — so every caller hands the value over for good. If `{fn}` should leave its caller's value usable, give away a copy instead: `{copy_form}`.
> **WHY**: A parameter without `give` still belongs to the caller — the caller's own binding stays usable after `{fn}` returns. If `{fn}` could hand that value onward anyway, the caller and the new holder would both have it, and when the new holder is done with it and frees it, the caller would be reading memory that is already gone. `give` in the signature makes the caller's binding unusable after the call, so the value has exactly one holder at every step. If the caller's value is itself a parameter, that function needs `give` too — every function on the chain that lacks the word is reported in this same build.

**`TransferNeedsCopy`** (replaces the Phase 1 draft's `SendPayloadNeedsCopy`, whose WHAT — "not a named binding, so the compiler cannot tell who else holds it" — was the enumeration speaking; the compiler now knows exactly who holds it, and the diagnostic fires at every sink, not only at a send). `{act}` as above; `{expr}` the source text; `{type}` the type; `{reason}` chosen by provenance — `a field of \`{root}\`` / `an item inside \`{root}\`` / `one cell of \`{root}\`, which this loop is walking` / `what \`{callee}\` returns, and \`{callee}\` returns a piece of its \`{param}\` argument` / `a value the compiler cannot trace to one owner`; `{fix}` is `{expr}.copy()`:

> **WHAT**: This line gives `{expr}` away — it is {act} — but `{expr}` is {reason}, so someone here still holds it.
> **WHAT INSTEAD**: Give away a copy instead: `{fix}`. If you meant to hand over the whole thing, give the whole thing: the binding that owns it.
> **WHY**: Whoever receives a `{type}` will eventually free it. If this side could still reach the same value afterward — through the field, the item, the loop, or the name it was built from — two places would hold one value and one of them would free it under the other. A `.copy()` belongs to nobody else, so it is safe to hand over.

Worked examples (the gallery renders these; `crates/ynz-driver/tests/error_galleries.rs` counts them):

```ynz
shape Bucket { rows: array<int> }
function eat(give rows: array<int>) -> nothing { print(rows.count().toString()) }
function pick(share b: maybe<Bucket>) -> maybe<Bucket> { return b }
function eatMaybe(give owner: maybe<Bucket>) -> nothing { print(owner.exists().toString()) }

function entrypoint() -> nothing {
  let bucket: Bucket = { rows: [1, 2, 3] }
  eat(bucket.rows)
  // COMPILE ERROR: This line gives `bucket.rows` away — it is given to `eat` — but `bucket.rows`
  //   is a field of `bucket`, so someone here still holds it.
  //   Give away a copy instead: `bucket.rows.copy()`. …
  let held: maybe<Bucket> = none
  let again = pick(held)
  eatMaybe(again)
  // COMPILE ERROR: … `again` is what `pick` returns, and `pick` returns a piece of its `b` argument …
  let matrix: array<array<int>> = [[1, 2], [3]]
  for (row in matrix) {
    eat(row)
    // COMPILE ERROR: … `row` is one cell of `matrix`, which this loop is walking …
  }
}
```

### Registry entries this section needs (Phase 4 adds them)

- `[[diagnostic_template]]` `ConsumedBySend` (slots `{name}`, `{channel}`, `{sent}`, `{via}`), `ParamNeedsGive` (as above, `{act}` gains the chain form), `TransferNeedsCopy` (new name; `SendPayloadNeedsCopy` was never registered or shipped — it appears only in Phase 1's draft and the v0.3-M8 plan text, both updated at sign-off). `HandleChannelArgNeedsBinding` is Phase 1's and unchanged.
- `[[deferred_language_feature]]` `name = "maybe-move-out"` — the consuming move-out-of-`maybe<T>` form deferred above ("What this makes sound, and what stays outside," packet item (g)). `substitute` = "Call `.copy()` on the accessed value at the relay site instead — e.g. `other.send(got.value.copy())`." `why` = the deferral's WHY (a consuming accessor is a language-wide `maybe` semantics addition outside a concurrency milestone's charter, not a corollary of the transfer rule) folded with its COST (a `maybe` design addendum, one new `Provenance` arm, one codegen path to move the wrapped value out instead of reading through the envelope). `ships_in` = "a later version" (no milestone tag). `design_doc` = this file, "Deferred (four fields, packet item (g))" above. `triggers` = "a measured relay workload where the extra `.copy()` per hop is a real cost, or the general move/drop story (Track 3 / FR 7(2)) landing and settling what 'consuming' means for `maybe<T>` alongside every other heap type."
- No new keyword, banned word, type constant, or muted-hint domain. The existing "ownership at call sites" hint domain renders `give` at every sink exactly as it does at a `give` parameter today — the sink list above is what it reads.

### Cross-references

- [`IMP-concurrency.md`](IMP-concurrency.md) "Channel Close — End-of-Stream Semantics" → "`send()` gives its payload" (the channel instance of sink 1; the `ChannelElemDrop` link; the runtime release protocol this section does not replace).
- `crates/ynz-typeck/src/effective_ownership.rs` — the module every fact above extends; [`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md).
- `.claude/corpses.md` "Enumerating syntactic sites instead of threading the whole-program ownership analysis" — the producer this section retires.
- v0.3-M8 plan Phase 2 (design), Phase 4 (implementation), Future Requirements #9 and #10.

---

## Auto-Arc — Sharing Topology Across `background` Boundaries (v0.3-M8 Phase 2; signed off 2026-09-03 — the v0.3-M8 plan's `audit.md` (`.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/audit.md`) SIGN-OFF record; Phase 5 SHIPPED it)

Code is cited by function name; line numbers drift.

**Status**: designed and signed off 2026-09-03 (`audit.md`'s SIGN-OFF record; `git log --grep=m8-p2` for the design rounds); **implemented in v0.3-M8 Phase 5** (the plan's Phase 5 block and `audit.md` entry `m8-p5-20260904-a1`; the caller-side range corrected in fix round 2, `audit.md` entry `m8-p5-fix2-20260904`). This is the section [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) §Runtime item 4 cites. Where it lives in the tree: the group admission is `admit_arc_group_for` in `crates/ynz-typeck/src/check.rs` (recording `BgOwnership::Arc { group, first, last }` through the ONE spawn-argument recording function, `record_spawn_arg_ownership`); the type floor is `arc_shareable` in `crates/ynz-typeck/src/types.rs`; the emission is the `Arc` arm of `prepare_bg_arg_for_ctx` plus `release_pending_arc_transients` in `crates/ynz-codegen/src/emit.rs` (the `ynz_arc_*` symbols are declared on first use so a program with no group emits byte-identical IR — Phase 5 declared them for the FIRST time, bare like every sibling runtime decl: the v0.3-M8 plan's premise that M7's Phase 1 attribute audit had already covered `ynz_arc_*` was false, M7's own FRAGO 001 recording that no `ynz_arc_*` symbol was declared or called from codegen); the task-side release is `BG_ARG_KIND_ARC_SHAPE` (`crates/ynz-abi`) in the drop ladder and the CPU-arm closure free; the hint is `auto_arc_hints` in `crates/ynz-typeck/src/inlay_hint_passes.rs`; the runtime substrate is `crates/ynz-runtime/src/arc.rs` (v0.3-M4), model-checked under loom by `loom_tests::loom_arc_group_*`. Fixtures: `crates/ynz-driver/tests/v03_m8_auto_arc.rs`. The registry entry `auto-arc-codegen-emission` is narrowed to the residual list at the end of this section.

### What `EffectiveOwnership::Reads` proves, and the one honest extension

`crates/ynz-typeck/src/effective_ownership.rs` answers, for every parameter of every local function, whether the body — across every path, transitively through every callee — only reads it (`Reads`), definitely writes or moves it (`Writes`), or flows it somewhere unanalyzable (`Unknown`). `Reads` is the lattice bottom and is raised by any non-read use; every unclassifiable path is `Unknown`, never `Reads`. That is exactly the **task-side** half of the sharing question: a value passed to a `background f(v)` is read-only inside the task iff `report.ownership_of("f", i) == Reads` at `v`'s position, including through everything `f` calls. Directly reusable; no restatement.

It is NOT the **caller-side** half. The report is per parameter; the caller's `v` is usually a local, and the question there is "does `v` denote the same, unwritten value at every spawn of the group?" — the same classification over a different region, PLUS one thing the walker does not answer today. The honest extension has two parts, both in `effective_ownership.rs`:

1. `classify_binding_in_stmts(name, stmts)` — the existing `classify_param_in_block` exposed over a statement suffix (it matches names; it never depended on the name being a parameter). Same lattice, same walker.
2. **A rebinding of the name is `Writes` in that walker.** Before Phase 5 it was not: the `Stmt::Assign` arm of `effective_ownership.rs::classify_param_in_stmt` classified by the assigned VALUE only, and the arm's own doc comment said reassigning the binding "does not escalate" — correct for the question the walker was written for (does the body write THROUGH the parameter's value; a parameter cannot be reassigned at all — the `Stmt::Assign` arm of `check.rs::check_stmts` refuses it), wrong for the caller-side question, where `v = other` between two spawns means spawn 2 must see a different value than the block minted at spawn 1 holds (probe — `git log --grep=m8-p2`: `background render(scene, out); scene = other; background render(scene, out)` prints `1 + 7 = 8` today, each spawn copying the current value; an Arc group admitted across that `Assign` would print `2`). ONE predicate, `stmt_rebinds(stmt, name) -> bool`, true for `Stmt::Assign { target == name }`, `Stmt::Let { name == name }` and `Stmt::For` binding `name` (the same three binding-event forms of "Binding events, origin and alias classes" above — the exhaustive list, reused, not re-listed), is consulted by the walker's statement arm and returns `Writes`. Cost on the task side: none in accepted programs for `Assign` (a compile error on a parameter) and a lost `readonly` attribute on a parameter shadowed by a same-name `let` in its own body — the conservative direction, and a form no fixture uses. That arm's doc comment now says this.

No new classifier is written for the caller side. The group condition below reads the same predicate to place group boundaries, so "is this statement a rebinding of `v`" has one definition for both consumers.

A declared `share` parameter on the spawned function stays the hard error it is today (the `Expr::Background` arm of `check.rs::infer_expr`, "Cannot use `background` with a function that borrows its arguments") — Auto-Arc's task-side proof is `Reads` on a BARE parameter, the modifier the compiler infers. This section reopens nothing.

### The topology, decided

Two shapes were on the table:

| | (A) repoint the caller | (B) one shared copy, N task references, caller keeps its original |
|---|---|---|
| Who holds a reference | caller + every task | every task; the caller holds ONE transient reference for the lexical extent of the spawn group and reads through its own original, never through the Arc |
| Caller's release | at scope exit — **needs the scope-exit drop pass that does not exist**: the compiler emits no release of any local at scope end (`ynz_handle_free` is declared in `crates/ynz-codegen/src/runtime_decls.rs` and has zero emit sites — [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) "Task Cancellation" implementation note; the roadmap's never-drop-locals row, M5 FR#6 / M6 FR#13–17, is the same gap; v0.3-M8 Phase 7 is where the pass question lives); without it the caller's reference is never released and the block leaks, or is not counted and the tasks free it under the caller | immediately after the last spawn of the group — a statically placed `ynz_arc_free` on a straight-line temporary; no drop pass needed |
| Frame layout | the caller's binding changes representation (a stack shape alloca becomes a pointer into an Arc block) — the crossing-local / frame-slot / `sm_crossing_*` machinery would have to learn a new shape of local; R2's hazard class | the caller's frame is untouched; the transient is an ordinary local that dies before any suspension |
| Benefit at N = 1 task | none (one copy either way, plus a header and two atomics) | none — and no group forms, so the shipped `.copy` path runs unchanged |
| Benefit at N ≥ 2 tasks | one copy instead of N | one copy instead of N |
| Composition with the shipped `.give`/`.copy` inference | a THIRD path the caller's own reads must know about | a refinement of `Copy` for a group of spawns: every member records `Arc` instead of `Copy`/`Give`; the caller's usage rules are unchanged |

**Decision: (B).** One `ynz_arc_new(struct_bytes)` + memcpy at the first spawn of a group, held in a caller-side transient; `ynz_arc_clone` at every member spawn (including the first — the transient's own reference is separate); `ynz_arc_free(transient)` immediately after the last member's spawn statement; each task's argument-drop ladder releases its reference through a new `BG_ARG_KIND_ARC_SHAPE` arm calling `ynz_arc_free(ptr, size)`. Count: `new`=1, N clones → N+1, transient released → N, tasks retire → 0. The transient is what makes the group safe across the region between spawns: without it, task 1 could finish and free the block before spawn 2 clones it.

### The beneficial-emission condition (the registry entry's own `why`)

Arc is emitted for binding `v` iff ALL hold, checked by `check.rs::admit_arc_group_for` (run from `admit_arc_groups_from` at the first `background` statement of a candidate group, inside the statement-form pass `check_stmts`) where the group is visible:

1. **≥ 2 spawn statements in one block pass `v` as a whole binding** (`Provenance::Whole`), by any spawn form (`background f(v)` or `let h = background f(v)`), with no suspension point between the first and the last (a suspending statement between them ends the group — the transient would have to cross a frame boundary, which is exactly the layout interaction R2 warns about; a later milestone may widen this with the crossing-local plumbing, recorded as the residual below). **The caller after spawn-return plus ONE task is NOT a sharing case under (B)**: the caller reads its own original, the task reads its copy — that is the shipped one-copy path, and an Arc would add a header and atomics for nothing. The plan's step-3 alternative ("caller + ≥1 task") is beneficial only under (A), which is infeasible without the drop pass.
2. **Task-side read-only proof**: for every member, `report.ownership_of(callee, position) == Reads`. `Unknown` or `Writes` on any member declines the whole group (a task that writes needs its own copy — today's semantics, unchanged).
3. **Caller-side same-value proof**: `classify_binding_in_stmts(v, stmts[first..=last]) == Reads` — the member `background` statements THEMSELVES and everything between them — where a rebinding of `v` is `Writes` (extension part 2 above). A write between the members means the later task must see the updated value, which a block minted at the first member cannot provide; decline to `Copy` (today's per-task copy sees the current value). **The members are inside the walked range, not just the statements between them**: a write can hide in a member's own argument list — `background render(scene, results, bump(scene))` with `bump` declared `lend` — and arguments are prepared in order, so the block would be minted from `scene` BEFORE `bump` ran and the next member's clone would read stale bytes (Phase 5 fix round 2, `red:code-reviewer`: the shipped range was `first + 1..last`, the group was admitted, and task 2 printed the stale `6` where the copy path prints `106`; fixture `m8_arc_write_in_member_arg_declines.ynz`). The walker's `Call` arm already returns `Writes` for a declared `lend`/`give` position, and the whole-binding member positions classify `Reads` from the same report the task-side proof read — no second classifier. **A rebinding of `v` at the TOP LEVEL of the spawns' own block — `v = …`, a shadowing `let v`, a `for (v in …)` — is a GROUP BOUNDARY, not a decline**: the group closes at the last spawn before it and a new group opens at the next spawn after it, each judged on its own member count (a one-member segment is the shipped copy path). Why a boundary and not `Writes`: a rebinding does not make sharing unsound, it changes WHICH value is shared — spawns before it read value A, spawns after it read value B, and each segment is a correct group in its own right; declining all of them would forfeit a sound sharing for no soundness reason, and `admit_arc_group_for` already walks the block by statement index to find the spawns, so placing a boundary is the same bookkeeping "no suspension between" already needs — one more delimiter of the same kind, read from the same `stmt_rebinds` predicate. A rebinding INSIDE a nested block between two spawns (`if (…) { v = other }`) is `Writes` and declines the enclosing group: which value spawn 2 sees is then path-dependent, and no statically placed block is right on both paths. A nested shadowing `let v` does not rebind the outer `v`, but the walker's same-name tracking classifies it `Writes` too — over-conservative, safe, and not worth a frame-aware twin of the walker.
4. **Arc-shareable type**: ONE predicate `arc_shareable(ty)` in `ynz_typeck::types`, the compile-time floor `arc.rs` documents — a `shape` whose fields are all `int`/`float`/`bool`/`string` (a `string` field is a pointer the shipped copy path — a byte `memcpy` of the struct — already shares between the caller and every task, and the runtime has no string release at all today, so sharing it by count changes nothing). Excluded: `number` fields (two reasons: 16-byte alignment, the block's data is 8-aligned; AND a shape with a `number` field SIGSEGVs on ANY `background` spawn today on the copy path, parked item 40 / FRAGO 012, hotfix pending — the floor must not widen to `number` until that lands), `array`/`map`/`maybe`/union/`channel` fields (pointer cells whose ownership the block cannot express — sharing the outer bytes would alias the inner allocation between tasks with no count on it), **nested `shape` fields** (the signed text said "inline `shape`"; Phase 5 found the tree stores a nested shape field as an opaque POINTER inside the struct — `ynz-codegen/src/shape_types.rs::llvm_field_type` — so it is a pointer cell exactly like an array field, and the implementation excludes it), and every non-shape type (`array<int>` bg args are cloned by `ynz_array_clone_primitive`; refcounting a header-plus-buffer pair is a different substrate, named in the residual).

**One boundary Phase 5 added beyond the signed list (item 1)**: a statement containing a `return` at any depth between two spawns is a group boundary. The transient's release is straight-line code placed after the last spawn; an early exit between the spawns would skip it and leak one count (never a free-early). The implementation's suspension test is deliberately conservative and syntactic (`stmt_may_suspend_conservative`: any `wait`, any call to a function the may-block fixpoint marked suspending or to a base suspension intrinsic, any call through a non-identifier callee, any `send`/`receive` method call on any receiver) because expression types of the statements ahead of the spawn are not yet inferred when the admission runs; over-approximating only declines to the shipped copy path.

Loops are out: `for (…) { background render(scene) }` is ONE spawn statement, so no group forms and the shipped per-iteration copy runs. A cross-iteration Arc would need a caller-held reference that outlives the statement — the drop-pass dependency again. Recorded in the residual.

When any condition fails, **nothing changes**: the shipped `Give`/`Copy` inference records exactly what it records today and `prepare_bg_arg_for_ctx` heap-copies per task. Phase 5 step 3 confirms that path byte-identical.

### What typeck records and what codegen reads (no re-derivation)

`BgOwnership` gains `Arc { group: GroupId, first: bool, last: bool }`, recorded for every member of an admitted group by the ONE spawn-argument recording function all three recording sites share (`check.rs::check_stmts` for the statement form, `check.rs::check_background_handle_spawn` for the handle-form pre-record, and the `Expr::Background` arm of `check.rs::infer_expr` as the backstop). That shared function also fixes parked item 16: the handle form records `Copy` unconditionally today even when the callee's `give` consumed the binding, so a hint reading `bg_inferred` would say "copied" for a value that was given. Codegen reads the recorded variant and emits `arc_new`+memcpy+`clone` (first), `clone` (member), `arc_free(transient)` after the last; it consults no ownership fact of its own.

The runtime's release protocol (`release_ladder_payload`, pointer identity) never meets an Arc'd shape: shapes are not channel elements (deferred) and are returned by value (v0.3-M7 R9). **Decided (Phase 5, packet item (h)): `bg_arg_kind_is_releasable_payload(BG_ARG_KIND_ARC_SHAPE)` is `false`.** The slot holds one COUNT on a block other tasks also count, not a payload the task owns outright; if a future hand-off ever matched it, skipping the ladder's release leaks one count (bounded, reported by the alloc/free parity gate) while releasing it under a receiver would be a use-after-free — `false` picks the leak side by construction. Pinned by the runtime's per-kind parity test (`channel::tests::every_bg_arg_kind_is_releasable_iff_the_ladder_frees_it_alloc_free_parity`), which stages the Arc kind with a live co-owner: the ladder's retire frees nothing, the co-owner's release frees the block once.

### Override directions ([`auto-promotion.md`](../../../.claude/rules/auto-promotion.md))

- **Force the OTHER pick (independent copies despite a group)**: write `background render(scene.copy())` at the spawn you want independent. `.copy()` is `Provenance::Fresh`, not `Whole`, so that spawn is not a group member; it takes the shipped per-task copy path (`prepare_bg_arg_for_ctx`'s explicit-`.copy()` arm). Existing, typeable syntax; no new API. **Correction to earlier text**: [`IMP-concurrency.md`](IMP-concurrency.md) "Ownership with Background Tasks" and the v0.3-M8 plan both named `.give` as a spawn-site override. No body-level `.give` exists — `PostfixOpKind` is `Copy | Freeze`, and this document's read-first note says so. The give direction is already what the liveness inference does for a binding unused after the spawn; there is nothing to force.
- **Force the AUTO pick (Arc when the compiler would not)**: deliberately no override. Every decline above is a soundness decline (a writer, an unprovable callee, a type the block cannot express) or a zero-benefit case (one task, a loop); forcing sharing through any of them either races or costs more than it saves.
- **`share` at a `background` boundary** stays a compile error. It is not reinterpreted as "Arc this": a `share` parameter is a borrow of the caller's frame, and the spawned task may outlive that frame; the Arc block is not the caller's value, it is a copy the tasks own.

### The muted hint (`auto_arc` domain, Informational category; live since Phase 5)

Inline: `// shared by reference count with {n} tasks — read-only` — `{n}` is the group's member count, read from the same `BgOwnership::Arc` record codegen emits from. The hover is static registry markdown (`ynz_registry::lsp_inlay_hint_hover_for` substitutes nothing), so the registry text says "each `background` task started here" where the draft below writes `{n}`; the count is in the label. The draft, tied to the actual group:

> **WHAT**: This value is copied once into a shared block, and each of the {n} `background` tasks started here holds a counted reference to that one copy instead of getting its own full copy. Your own binding is untouched — you keep reading your original.
> **WHAT INSTEAD**: Nothing to type — the compiler picks this when {n} tasks only read the value and nothing changes it between the `background` statements. If one task needs its own version to change, pass `{name}.copy()` at that `background` statement so it gets an independent copy.
> **WHY**: {n} tasks read this value, so one shared copy saves {n − 1} full copies. Each task pays a tiny bookkeeping step when it takes and drops its reference — cheaper than a copy here, which is why the compiler shares only when at least two tasks read it.

The red-tint styling stays under `auto-arc-cautionary-tint` (no per-hint tint path in `ynz-lsp`); the caution is in the words.

### Residual (`auto-arc-codegen-emission` is narrowed to exactly this)

- Spawn groups that straddle a suspension point (needs the transient to be a crossing local).
- Spawn groups inside a loop (needs a caller-held reference across iterations — the drop pass).
- `array`/`map` values and shapes with pointer-cell (`array`/`map`/`maybe`/union/`channel`/nested `shape`) or `number` fields (a different sharing substrate; `arc_shareable` is the floor — and `number` stays out for the second reason above, parked item 40, until its copy-path hotfix lands).

### Cross-references

- [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) §Runtime item 4 — the pointer this section resolves; "False Sharing Auto-Padding" — the transform whose throughput benefit arrives with this emission.
- [`IMP-concurrency.md`](IMP-concurrency.md) "Ownership with Background Tasks — Why `.share` Fails" — the inference table this refines.
- `registry/features.toml` `auto-arc-codegen-emission`, `auto-arc-cautionary-tint`, `[[muted_hint_domain]] auto_arc`.
- v0.3-M8 plan Phase 2 (design), Phase 5 (implementation), risk R2 (signed override), Future Requirement #5.
