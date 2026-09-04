---
name: "IMP-ownership"
description: "Design decisions for Yinz's ownership system (share/lend/give signature keywords plus .copy()/.freeze() operations) and how the compiler infers call-site ownership instead of requiring Rust-style borrow syntax."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-09-03"
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
- **`.copy()`** — produce a new owned value (only on transitively-trivially-copyable types per r4).
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
const backup = original.copy()       // strict: only legal on transitively-trivially-copyable types
saveForever(backup)                   // backup consumed (give inferred), original unchanged
```

Strict cheap-only per r4: `.copy()` is only legal when every field of the value's type is transitively trivially copyable. For non-trivial deep copies, the user defines a standalone function (typically also named `copy`) and calls it via normal function/UFCS syntax — Yinz prefers explicit user-defined deep-copy semantics over silent expensive copies.

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

## No Direct Array Indexing — `.get(index)` Returns `maybe T`

`items[5]` is a compile error. All collection access by index uses `.get(index)` which returns `maybe T`. Collections include `.first()` and `.last()` returning `maybe T`.

**Why**: Out-of-bounds array access is one of the most common runtime crashes and security vulnerabilities (buffer overflows) in systems languages. If the compiler can enforce safe access universally, there's no reason not to. The cost is slightly more verbose access; the benefit is the elimination of an entire crash category.

**Performance**: In release mode, the compiler eliminates bounds checks it can statically prove are safe (e.g., a `fixed<3>` accessed at index 1 — provably in bounds). Debug mode always bounds-checks. Performance impact is negligible in practice.

**Consistency**: Maps already use `.get(key)` returning `maybe V`. The same pattern applies everywhere. No special case for arrays — one rule, all collections.

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
3. **`returns_fresh[fn]: Freshness`** — per function: `Fresh` iff every `return` yields a value nobody else reaches (a fresh expression, a `give` parameter, or a local whose initializer was fresh and that was not aliased); `MayAlias` otherwise (a `share`/`lend`/bare parameter or any piece of one; a call to a `MayAlias` or imported callee; anything unclassifiable). Bottom `Fresh`, raised in the same loop. Imported functions are `MayAlias`.
4. **`classify_binding_in_stmts(name, stmts) -> EffectiveOwnership`** — the existing per-name body classifier (`classify_param_in_block`) exposed over an arbitrary statement suffix. It never cared that the name was a parameter; the Auto-Arc caller-side proof below needs the identical question asked about a local over the statements after a spawn.

```rust
pub enum Provenance {
    /// This evaluation is the value's only holder. Transferable; nothing to consume.
    Fresh,
    /// Exactly the value a binding names (`Ident` / `self`). Transferable iff the
    /// binding's origin (below) permits; consumes the binding's whole alias class.
    Whole(String),
    /// A value someone still reaches through the named roots — a field, an index,
    /// a loop cell, a literal built from named values, a call that returns a piece
    /// of its argument. Never transferable; the fix is `.copy()` on the reached piece.
    Reaches(Vec<String>),
    /// Cannot classify (function-value call, dynamic-dispatch result, imported
    /// non-fresh callee, `.copy()` on a type whose copy is not yet independent).
    /// Never transferable.
    Unknown,
}
```

Classification (one function; listed here so a reader can check it, not so a call site can copy it):

| Expression | Provenance |
|---|---|
| int / number / bool / string / `none` literal; `BinOp`; `UnaryOp`; interpolated string; `is` | `Fresh` (value bits, or immortal string bytes) |
| `Ident`, `self` | `Whole(name)` — always; the binding's ORIGIN decides transferability |
| `FieldAccess` / `IndexAccess` | `Reaches(root_binding_name(receiver))`; a piece of a fresh temp (`makeBucket().rows`) is `Reaches([])` — still not transferable |
| array / map / shape literal | `Fresh` iff every heap-typed element/value is `Fresh`; else `Reaches(∪ roots of the non-fresh elements)` |
| `.copy()` | `Fresh` iff `copy_is_independent(type)` (ONE predicate in `types.rs`, parity-tested against the codegen `PostfixOpKind::Copy` arms — `array`, `map` after Phase 4 step 3a, inline `shape`); else `Unknown` (the FR#10 alias-no-op types) |
| `.freeze()` (the other `PostfixOpKind` arm) | `Fresh` — it is typed `nothing` (`check.rs:6947`, the P3c mutability flip never shipped; codegen lowers it as the bare receiver, `emit.rs:19297`), so no sink can ever accept it and there is no value to hold. Exhaustiveness is variant-level, so the arm is written; if `.freeze()` is ever retyped to return its receiver, this row becomes `provenance(receiver)` in the same commit — the non-wildcard match is what forces that reader here |
| constructor call — `channel<T>()` (the only constructor-call form the parser accepts; `array<T>()` / `map<K, V>()` are not forms the parser has, an empty container is the literal `[]` / `{}`, classified in the literal row); `.receive()` on a channel or handle | `Fresh` (the receiver's `maybe<T>` is the sole reference — [`IMP-concurrency.md`](IMP-concurrency.md) "What this makes sound") |
| builtin method call | from the ONE `builtins` table (`builtin_method_returns_fresh(name)`); **default `Reaches([receiver root])`** — `.get`/`.first`/`.last` return a cell; `.sort`/`.filter`/`.map`/`.concat` are widened one at a time with evidence, never by default (parked item 18 is this table, recorded as a deliberate conservative omission) |
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
| **reassignment** `name = value` | `Stmt::Assign` | the entry **leaves its class** (the class keeps its other members and their consumed state — they still hold the old value); the entry's consumed state is CLEARED (a consumed name that is reassigned names a new value; today's typeck gets this wrong — probe `eat(rows); rows = [4, 5]; rows.count()` is refused "already given away", a correct program rejected, fixed here by construction) | per the initializer table below, computed from the assigned value |
| `for` loop variable | `Stmt::For` | none (frame-scoped; `Assign` to it is already a compile error, `check.rs:2444`) | `Cell` (a cell of the iterated value; never transferable); joins the iterated root's class |
| every name a `for` destructure binds (`destructure_pattern`, `map_destructure_pattern`) | **not** a `Stmt::For` binding — the parser desugars each destructured name to its own `Stmt::Let { value: __shape.field }`, prepended ahead of the loop body (`parser.rs:2252–2274`; map form `:2417`) | none | a `Let` event, so it takes the initializer table below from `Reaches(__shape)` provenance — never transferable, joins the iterated root's class; unlike the loop variable, `Assign` to a destructured name is admitted (`check.rs:2436–2465`) and is covered by the `Assign` row above, not this one (parked item 25) |
| function parameter | `FunctionDecl.params` (not a `Stmt`) | none (`Assign` to a parameter is already a compile error, `check.rs:2436`) | `Param(declared modifier)`; its own class |

The other seven statement forms bind no name: `Expr`, `If`, `While`, `Return` (they only read); `Match` (arms carry a `MatchPattern`, not a binding — `is` narrowing refines a type, it does not rebind); `FieldAssign` and `IndexAssign` write THROUGH a name into a container — they never change what the name denotes, and as transfer sinks they are the deferred container-store class below. `Expr` has no binding form at all (no closure or arrow-function variant exists in `ynz-ast`). If a variant that binds a name is ever added to `Stmt`, it fails the pinned variant-count test and the `match` in the one function that implements this table — the compiler, not a reviewer, asks for its row.

The initializer table, applied at every `Let` and `Assign` event:

| Value bound has provenance | Origin | Alias class |
|---|---|---|
| `Fresh` | `Owned` | its own (a new class) |
| `Whole(b)` | b's origin | **joins b's class** — two names, one value |
| `Reaches(roots)` | `Reaches` (never transferable) | joins every root's class — if a root is given away, this name is consumed with it |
| `Unknown` | `Unknown` (never transferable) | its own |

Why "leave, then join" on a reassignment is the whole of the fix: `let rows = [1]; let other = [2]; rows = other; wire.send(rows); other.count()` — on today's tree the send consumes nothing and the program prints `2 2` (probe — `git log --grep=m8-p2`; the shadowing form `let rows = other` prints the same). Under this rule the `Assign` moves `rows` out of its own class and into `other`'s; the send consumes `{rows, other}`; `other.count()` is `ConsumedBySend` with the `{via}` slot naming `rows`. The `[1]` allocation `rows` used to name has no holder — an existing leak class (no scope-exit drop pass), not a use-after-free. The existing consumed-read mechanism's flow shape (a straight-line walk; no loop back-edge re-check) is unchanged by this section.

`consumed: bool` becomes `consumed: Option<ConsumedBy>` with `enum ConsumedBy { Given { callee }, Sent { channel } }` (the cause fills the diagnostic; the consumed-read site selects the template by it) — one field, no parallel `Option<String>`. `Scope::consume(name, cause)` consumes the whole class. The `Stmt::Assign` arm of typeck (`check.rs:2436–2465`, today a pure type check on a non-`const`, non-parameter, non-loop `let` local) becomes the second caller of the ONE binding-event function `Stmt::Let` calls — never a re-implementation beside it.

### The transfer decision — ONE emit site

`check_transfer(expr, sink)` is the only function that decides a transfer and the only place the three diagnostics below are emitted. Every sink calls it; no sink inspects syntax:

- `Fresh` → admitted, nothing to consume.
- `Whole(name)` → read the entry: `const` → the existing cannot-give-const refusal (extracted into a helper with a sink-supplied WHAT-INSTEAD — a sender wants `{channel}.send({name}.copy())`, not "declare it with `let`"); already consumed → the existing consumed-read site fires (`ConsumedBySend` or the existing `Consumed`, by cause); `Param(m)` with `m != give`, or a class containing such a parameter → **`ParamNeedsGive`**; `Reaches` / `Cell` / `Unknown` origin → **`TransferNeedsCopy`**; `Owned` or `Param(give)` → consume the class with the sink's cause.
- `Reaches(_)` / `Unknown` → **`TransferNeedsCopy`**.

**The sinks this milestone wires to it** (each is a give position; the list is of SINKS, which is closed and small, not of argument shapes, which is not):

1. `channel<T>.send(v)` when `channel_elem_drop(T)` is `Some(kind)` and `kind.transfers_source()` — the owned-heap element kinds `array`/`map` ([`IMP-concurrency.md`](IMP-concurrency.md) "Which element types"). `int`/`float`/`bool`/`string`/`number` channels are copy-through and never reach the helper.
2. Every declared-`give` parameter position of every call form — plain call, monomorphized generic call, UFCS dot-call (**receiver AND non-receiver arguments**), and `dynamic Contract` dispatch — through ONE argument-list normalization (`[receiver, args…]` — the same normalization `background_spawn_call_form` and `collect_aliasing_in_expr` already perform; Phase 4 makes the three call paths share it rather than adding a fourth loop). A position is a give position when it is declared `give` OR `consumed[callee][i]` is true — the second case is what reports a relay chain in one compile (below).
3. A `background` argument the spawn gives away — **and this sink is the liveness INFERENCE, not a `check_transfer` call.** The inference at `check.rs:1443–1515` chooses `Give` or `Copy` for a `Whole(name)` binding the user never annotated; nothing it chooses can be reported as an error against a `give` the user did not write. **A `channel<T>`-typed argument never reaches this rule at all**: the earlier `BgOwnership::Channel` branch (`check.rs:1461–1469`) claims it first and shares it (`emit.rs:16752–16761`, `ynz_channel_share`) — a `channel<T>()` constructor argument is `Fresh` → `Owned` under provenance, but that origin is never consulted here because the channel branch runs before this rule sees the argument (parked item 20). For every other type, the rule, restated class-aware: infer `Give` iff the binding's origin is `Owned` or `Param(give)` AND no member of its alias class is read in any remaining statement; otherwise infer `Copy`. A `Cell`, `Reaches`, `Unknown` or `Param(share|lend|bare)` origin therefore declines to `Copy` — **and "declines to `Copy`" means a `Copy` entry is still RECORDED for the argument, never nothing** (parked item 19): `for (row in matrix) { background producer(wire, row) }` is a correct program today (probe: prints `3`, `matrix` intact afterwards) and stays one because codegen's `is_heap_arg` gate (`emit.rs:16810–16828`) reads the PRESENCE of an entry in `background_arg_inferred_ownership`, not which variant it holds (`fr23_uaf_planned_red.rs:27` confirms the same reading) — an implementer who records nothing for a `Whole(name)` spawn argument hands the task a spawner-stack pointer instead of a heap copy (the fr23 use-after-free class). `bg_inferred` is not read only by the inlay-hint pass (`inlay_hint_passes.rs:765` is A reader, not the only one); codegen already copies by TYPE for a recorded `Copy`, so the decline changes no runtime behavior for that case — it only stops typeck from consuming a name the frame does not own, and makes the muted hint say "copied", which is what happens. The `Give` consume routes through the class, so a name that aliases the given binding is consumed with it — and because liveness is asked of the CLASS, `let other = rows; background f(rows); other.count()` infers `Copy` rather than consuming `other` under a program that runs correctly. The spawned call's own DECLARED `give` positions are sink 2 (`TransferNeedsCopy` on a loop cell there is correct: the callee's author wrote the word), because the `Expr::Background` arm runs `infer_expr` on the call.

**Not sinks in this milestone, decided, with the deferral named:** storing a heap value into a container — `list.add(v)`, `table.set(k, v)`, `x.field = v`, `x[i] = v` — and container LITERALS built from named heap values. Under the one-owner model these are transfers too. They are excluded here because (WHAT) admitting them means deciding, once and for every heap type, what putting a value into a container does to the source name — the same decision v0.3-M8 Future Requirement #9 and the drop-story work own for `background` arguments; (WHY deferred) no scope-exit drop pass exists, so an aliased container element is a leak today, never a free-under-the-reader, except through the one ladder door FR#9 RED-pins; deciding literals alone would make `[a]` consume `a` while `outer.add(a)` does not; (COST) medium — the sink list grows by four entries that call the same helper, plus the `.copy()`-or-consume ruling per type and the fixture corpus re-compiled for instances; (TRIGGER) the drop-insertion pass, or FR#9's rule for `background` arguments, whichever lands first. **Until then a literal built from a named heap value is `Reaches`**, so it cannot itself be transferred (`let outer = [a]; wire.send(outer)` is `TransferNeedsCopy` naming `a`) — the depth-two aliasing parked item 12 found is refused, not admitted.

### The parameter rule and how far it travels (FRAGO 007, re-derived)

A whole binding that is a parameter transfers only if declared `give`. With `give` declared, the EXISTING call-site path consumes the caller's binding (sink 2) at every call and every spawn — one mechanism. The obligation therefore transits the whole chain: A relays to B relays to C, C sends; C's parameter needs `give`; B passes its parameter to a give position, so B's needs `give`; up to the frame that owns the value as a local. `share` and `lend` parameters are refused by the same test — they are explicit statements that the caller keeps the value.

**Every frame of the chain is reported in ONE compile.** The earlier draft reported one frame per compile on the stated premise that typeck lacks a callee-before-caller ordering. The premise is false: the fixpoint IS that ordering. `consumed[B][i]` is true as soon as B's body sends its parameter, whether or not B declares `give`; A's call `B(x)` therefore sees a give position, and if `x` is A's own undeclared parameter, A gets its `ParamNeedsGive` in the same compile as B. For a LOCAL `x` at A, the call also consumes `x` in that compile (so A's later read of it is reported now, not after B is fixed) — the program is already rejected by B's error, so this consumption changes only what is reported, never what is accepted; the accept/reject decision stays signature-driven, which is why this is not the "infer `give` for bare parameters" alternative FRAGO 007 rejected (that alternative would have accepted a program whose caller's signature says nothing). Each error is independently actionable — one word in one signature — so N hops cost one compile, not N.

### `dynamic Contract` dispatch — covered by construction

A contract method's parameters carry ownership modifiers in the AST (`ContractSig.params: Vec<Param>`, each with `ownership`; the receiver's `ReceiverKind` too). **They are OPTIONAL there, exactly as on a function with a body** — the parser's contract-signature loop accepts a bare position (`parser.rs:3996–4012`, `_ => None`); the rule is the one the rest of the language already has: **a bare position is never a give position**, whether on a function (FRAGO 007 — `give` is written, never inferred) or on a contract (signed 2026-09-03, packet item (j) — `audit.md`'s SIGN-OFF record). A bare contract position is read-or-lend by the implementer's effective ownership, which is what the fixpoint computes for the implementing function; only a written `give` makes the dispatch site a transfer sink. `follows` parity is EXACT at the declared level — bare matches bare, `give` matches `give` — so an implementer cannot declare `give` where the contract is bare (the vtable would promise the caller keeps the value and the implementation would free it) nor the reverse. No parser enforcement is added. Typeck drops the modifiers at resolution (`ContractSigDef` in `shapes.rs` keeps only `param_tys`), so the dispatch site (`check.rs:5391–5421`) checks no ownership at all. Three consequences, one rule: `ContractSigDef` carries `param_ownerships` and the receiver kind; the dispatch site builds the same normalized argument list and calls `check_transfer` for every `give` position **using the contract's declared modifiers** (the only static truth for a runtime-resolved callee); and `follows` conformance checks that the implementing function's modifiers equal the contract's — otherwise the vtable would promise `give` and the implementation take `share`. Runtime exposure today is nil: codegen refuses every dynamic-dispatch call site ("dynamic dispatch call sites not yet lowered in M4 P4", confirmed by probe), so this closes a typeck hole before it can be reached, not a shipped crash. The `effective_ownership` fixpoint keys callees by name because the signature table does (`sig_table.fns` is one entry per name — no overloading is implemented); if overloading ships, both key by signature together.

### What this makes sound, and what stays outside

With every sink threaded through `check_transfer`, a transferred allocation has exactly one source-level holder at every moment: the owner (and its alias class) until the transfer, then the sink. The receiver of a channel is the sole reference; a `give` callee is the sole reference; a `background` task's ladder-owned clone is released by the shipped runtime protocol when the task sends it on ([`IMP-concurrency.md`](IMP-concurrency.md) "Two mechanisms, one rule" — unchanged by this section, and still not redundant with it: it answers allocation-level ladder ownership; this section answers source-level readability).

Outside the guarantee, named: FR#9's container door (`bucket.add(rows)` into an aliased container — the deferred sink class above; RED-pinned); `.copy()` on the FR#10 types (`maybe`, union, `fixed`, `dynamic`) which provenance classifies `Unknown` so they cannot be transferred at all until their copy is independent; and relaying a RECEIVED owned-heap value — `let got = wire.receive()` then `other.send(got.value)` — where `got.value` is a piece of `got` (a `maybe<T>` field access, `Provenance::Reaches`), so `TransferNeedsCopy` fires and the relay pays `.copy()` this milestone (packet item (g), signed 2026-09-03).

**Deferred (four fields, packet item (g)): a consuming move-out-of-`maybe<T>` form.**
- **WHAT**: a consuming accessor on `maybe<T>` — some form that yields the wrapped value as `Provenance::Fresh` (moving it out of the `maybe` rather than reaching through it), so a relay of a received owned-heap value would not need `.copy()`.
- **WHY deferred**: the alternative to accepting the `.copy()` cost this milestone is a language-wide `maybe` semantics addition, which is outside a concurrency milestone's charter — `maybe<T>` is used everywhere in the language, not only at channel receives, so a consuming accessor's design (naming, what happens to the `maybe` binding afterward, whether it generalizes to every `maybe`-producing site) is its own design pass, not a corollary of the transfer rule.
- **COST to add later**: a `maybe` design addendum (the accessor's name and semantics), one new `Provenance` arm for it, and one codegen path to move the wrapped value out instead of reading through the envelope.
- **TRIGGER**: a measured relay workload where the extra `.copy()` per hop is a real cost, or the general move/drop story (Track 3 / FR 7(2)) landing and settling what "consuming" means for `maybe<T>` alongside every other heap type.

### Teaching text (Golden Rule 11; three diagnostics, one emit site)

**`ConsumedBySend`** — unchanged from Phase 1's drafting except one slot: because consumption is class-wide, the read name may differ from the sent name. `{name}` is the binding read, `{channel}` the channel; when the name read is not the name sent, `{via}` renders ` — it shares its value with \`{sent}\`, which is what was sent`.

> **WHAT**: `{name}` was sent into `{channel}` and cannot be used here — `send()` gave it away{via}.
> **WHAT INSTEAD**: If you still need `{name}` after sending it, send a copy instead: `{channel}.send({sent}.copy())`. If you only need it before the send, put this line above the `send()`.
> **WHY**: A channel hands the value to whichever task receives it, and that task may already be reading or changing it by the time this line runs. If both sides kept the same value, two tasks could change it at once and neither would know. So `send()` gives `{sent}` away, and the compiler refuses to build any line that reads it afterward. Nothing happens at runtime — `{name}` is not cleared or set to `none`; the program simply does not compile until this read is placed above the `send()` or the send takes a copy.

**`ParamNeedsGive`** — `{name}` the parameter, `{fn}` its function, `{type}` its declared type, `{modifier}` one of `has no ownership word` / `is declared \`share\`` / `is declared \`lend\``, `{act}` one of `sent into \`{channel}\`` / `given to \`{callee}\`` / `given to \`{callee}\`, whose \`{param}\` parameter gives it away`, `{copy_form}` the matching `{channel}.send({name}.copy())` / `{callee}({name}.copy(), …)`:

> **WHAT**: `{name}` is a parameter of `{fn}` that {modifier}, but this line gives it away — it is {act}. Only a `give` parameter can be given away.
> **WHAT INSTEAD**: Declare it `give` in `{fn}`'s signature — `function {fn}(give {name}: {type}, …)` — so every caller hands the value over for good. If `{fn}` should leave its caller's value usable, give away a copy instead: `{copy_form}`.
> **WHY**: A parameter without `give` still belongs to the caller — the caller's own binding stays usable after `{fn}` returns. If `{fn}` could hand that value onward anyway, the caller and the new holder would both have it, and when the new holder is done with it and frees it, the caller would be reading memory that is already gone. `give` in the signature makes the caller's binding unusable after the call, so the value has exactly one holder at every step. If the caller's value is itself a parameter, that function needs `give` too — every function on the chain that lacks the word is reported in this same build.

**`TransferNeedsCopy`** (replaces the Phase 1 draft's `SendPayloadNeedsCopy`, whose WHAT — "not a named binding, so the compiler cannot tell who else holds it" — was the enumeration speaking; the compiler now knows exactly who holds it, and the diagnostic fires at every sink, not only at a send). `{act}` as above; `{expr}` the source text; `{type}` the type; `{reason}` chosen by provenance — `a field of \`{root}\`` / `an item inside \`{root}\`` / `one cell of \`{root}\`, which this loop is walking` / `built from \`{roots}\`, which are still named here` / `what \`{callee}\` returns, and \`{callee}\` returns a piece of its \`{param}\` argument` / `a value the compiler cannot trace to one owner`; `{fix}` is `{expr}.copy()` for a piece, `[{root}.copy(), …]` for a literal:

> **WHAT**: This line gives `{expr}` away — it is {act} — but `{expr}` is {reason}, so someone here still holds it.
> **WHAT INSTEAD**: Give away a copy instead: `{fix}`. If you meant to hand over the whole thing, give the whole thing: the binding that owns it.
> **WHY**: Whoever receives a `{type}` will eventually free it. If this side could still reach the same value afterward — through the field, the item, the loop, or the name it was built from — two places would hold one value and one of them would free it under the other. A `.copy()` belongs to nobody else, so it is safe to hand over.

Worked examples (the gallery renders these; `crates/ynz-driver/tests/error_galleries.rs` counts them):

```ynz
shape Bucket { rows: array<int> }
function eat(give rows: array<int>) -> nothing { print(rows.count().toString()) }
function pick(share b: Bucket) -> array<int> { return b.rows }

function entrypoint() -> nothing {
  let bucket: Bucket = { rows: [1, 2, 3] }
  eat(bucket.rows)
  // COMPILE ERROR: This line gives `bucket.rows` away — it is given to `eat` — but `bucket.rows`
  //   is a field of `bucket`, so someone here still holds it.
  //   Give away a copy instead: `bucket.rows.copy()`. …
  let rows = pick(bucket)
  eat(rows)
  // COMPILE ERROR: … `rows` is what `pick` returns, and `pick` returns a piece of its `b` argument …
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

## Auto-Arc — Sharing Topology Across `background` Boundaries (v0.3-M8 Phase 2; signed off 2026-09-03 — the v0.3-M8 plan's `audit.md` (`.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/audit.md`) SIGN-OFF record; Phase 5 implements)

**Status**: designed 2026-09-03, signed off 2026-09-03 (`audit.md`'s SIGN-OFF record; `git log --grep=m8-p2` for the design rounds). This is the section [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) §Runtime item 4 has cited since v0.3-M4 and the registry entry `auto-arc-codegen-emission` names as the missing decision. Nothing below is in the tree yet — Phase 5 implements it; the runtime substrate (`crates/ynz-runtime/src/arc.rs` — `ynz_arc_new`/`clone`/`free`, acquire-release, counted through `ynz_alloc`) shipped in v0.3-M4 and is what Phase 5 calls.

### What `EffectiveOwnership::Reads` proves, and the one honest extension

`crates/ynz-typeck/src/effective_ownership.rs` answers, for every parameter of every local function, whether the body — across every path, transitively through every callee — only reads it (`Reads`), definitely writes or moves it (`Writes`), or flows it somewhere unanalyzable (`Unknown`). `Reads` is the lattice bottom and is raised by any non-read use; every unclassifiable path is `Unknown`, never `Reads`. That is exactly the **task-side** half of the sharing question: a value passed to a `background f(v)` is read-only inside the task iff `report.ownership_of("f", i) == Reads` at `v`'s position, including through everything `f` calls. Directly reusable; no restatement.

It is NOT the **caller-side** half. The report is per parameter; the caller's `v` is usually a local, and the question there is "does `v` denote the same, unwritten value at every spawn of the group?" — the same classification over a different region, PLUS one thing the walker does not answer today. The honest extension has two parts, both in `effective_ownership.rs`:

1. `classify_binding_in_stmts(name, stmts)` — the existing `classify_param_in_block` exposed over a statement suffix (it matches names; it never depended on the name being a parameter). Same lattice, same walker.
2. **A rebinding of the name is `Writes` in that walker.** Today it is not: the `Stmt::Assign` arm classifies by the assigned VALUE only (`:379`), and the arm's own doc comment says reassigning the binding "does not escalate" (`:330–333`) — correct for the question the walker was written for (does the body write THROUGH the parameter's value; a parameter cannot be reassigned at all, `check.rs:2436`), wrong for the caller-side question, where `v = other` between two spawns means spawn 2 must see a different value than the block minted at spawn 1 holds (probe — `git log --grep=m8-p2`: `background render(scene, out); scene = other; background render(scene, out)` prints `1 + 7 = 8` today, each spawn copying the current value; an Arc group admitted across that `Assign` would print `2`). ONE predicate, `stmt_rebinds(stmt, name) -> bool`, true for `Stmt::Assign { target == name }`, `Stmt::Let { name == name }` and `Stmt::For` binding `name` (the same three binding-event forms of "Binding events, origin and alias classes" above — the exhaustive list, reused, not re-listed), is consulted by the walker's statement arm and returns `Writes`. Cost on the task side: none in accepted programs for `Assign` (a compile error on a parameter) and a lost `readonly` attribute on a parameter shadowed by a same-name `let` in its own body — the conservative direction, and a form no fixture uses. The doc comment at `:330–333` is rewritten to say this.

No new classifier is written for the caller side. The group condition below reads the same predicate to place group boundaries, so "is this statement a rebinding of `v`" has one definition for both consumers.

A declared `share` parameter on the spawned function stays the hard error it is today (`check.rs:~3470`, "Cannot use `background` with a function that borrows its arguments") — Auto-Arc's task-side proof is `Reads` on a BARE parameter, the modifier the compiler infers. This section reopens nothing.

### The topology, decided

Two shapes were on the table:

| | (A) repoint the caller | (B) one shared copy, N task references, caller keeps its original |
|---|---|---|
| Who holds a reference | caller + every task | every task; the caller holds ONE transient reference for the lexical extent of the spawn group and reads through its own original, never through the Arc |
| Caller's release | at scope exit — **needs the scope-exit drop pass that does not exist**: the compiler emits no release of any local at scope end (`ynz_handle_free` is declared in `crates/ynz-codegen/src/runtime_decls.rs:126` and has zero emit sites — [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) "Task Cancellation" implementation note; the roadmap's never-drop-locals row, M5 FR#6 / M6 FR#13–17, is the same gap; v0.3-M8 Phase 7 is where the pass question lives); without it the caller's reference is never released and the block leaks, or is not counted and the tasks free it under the caller | immediately after the last spawn of the group — a statically placed `ynz_arc_free` on a straight-line temporary; no drop pass needed |
| Frame layout | the caller's binding changes representation (a stack shape alloca becomes a pointer into an Arc block) — the crossing-local / frame-slot / `sm_crossing_*` machinery would have to learn a new shape of local; R2's hazard class | the caller's frame is untouched; the transient is an ordinary local that dies before any suspension |
| Benefit at N = 1 task | none (one copy either way, plus a header and two atomics) | none — and no group forms, so the shipped `.copy` path runs unchanged |
| Benefit at N ≥ 2 tasks | one copy instead of N | one copy instead of N |
| Composition with the shipped `.give`/`.copy` inference | a THIRD path the caller's own reads must know about | a refinement of `Copy` for a group of spawns: every member records `Arc` instead of `Copy`/`Give`; the caller's usage rules are unchanged |

**Decision: (B).** One `ynz_arc_new(struct_bytes)` + memcpy at the first spawn of a group, held in a caller-side transient; `ynz_arc_clone` at every member spawn (including the first — the transient's own reference is separate); `ynz_arc_free(transient)` immediately after the last member's spawn statement; each task's argument-drop ladder releases its reference through a new `BG_ARG_KIND_ARC_SHAPE` arm calling `ynz_arc_free(ptr, size)`. Count: `new`=1, N clones → N+1, transient released → N, tasks retire → 0. The transient is what makes the group safe across the region between spawns: without it, task 1 could finish and free the block before spawn 2 clones it.

### The beneficial-emission condition (the registry entry's own `why`)

Arc is emitted for binding `v` iff ALL hold, checked at the statement-form liveness pass (`check.rs:1443–1515`) where the spawn group is visible:

1. **≥ 2 spawn statements in one block pass `v` as a whole binding** (`Provenance::Whole`), by any spawn form (`background f(v)` or `let h = background f(v)`), with no suspension point between the first and the last (a suspending statement between them ends the group — the transient would have to cross a frame boundary, which is exactly the layout interaction R2 warns about; a later milestone may widen this with the crossing-local plumbing, recorded as the residual below). **The caller after spawn-return plus ONE task is NOT a sharing case under (B)**: the caller reads its own original, the task reads its copy — that is the shipped one-copy path, and an Arc would add a header and atomics for nothing. The plan's step-3 alternative ("caller + ≥1 task") is beneficial only under (A), which is infeasible without the drop pass.
2. **Task-side read-only proof**: for every member, `report.ownership_of(callee, position) == Reads`. `Unknown` or `Writes` on any member declines the whole group (a task that writes needs its own copy — today's semantics, unchanged).
3. **Caller-side same-value proof**: `classify_binding_in_stmts(v, stmts between first and last spawn) == Reads`, where a rebinding of `v` is `Writes` (extension part 2 above). A write between spawns means spawn 2 must see the updated value, which a block minted at spawn 1 cannot provide; decline to `Copy` (today's per-spawn copy sees the current value). **A rebinding of `v` at the TOP LEVEL of the spawns' own block — `v = …`, a shadowing `let v`, a `for (v in …)` — is a GROUP BOUNDARY, not a decline**: the group closes at the last spawn before it and a new group opens at the next spawn after it, each judged on its own member count (a one-member segment is the shipped copy path). Why a boundary and not `Writes`: a rebinding does not make sharing unsound, it changes WHICH value is shared — spawns before it read value A, spawns after it read value B, and each segment is a correct group in its own right; declining all of them would forfeit a sound sharing for no soundness reason, and the statement-form pass (`check.rs:1443`) already walks the block by statement index to find the spawns, so placing a boundary is the same bookkeeping "no suspension between" already needs — one more delimiter of the same kind, read from the same `stmt_rebinds` predicate. A rebinding INSIDE a nested block between two spawns (`if (…) { v = other }`) is `Writes` and declines the enclosing group: which value spawn 2 sees is then path-dependent, and no statically placed block is right on both paths. A nested shadowing `let v` does not rebind the outer `v`, but the walker's same-name tracking classifies it `Writes` too — over-conservative, safe, and not worth a frame-aware twin of the walker.
4. **Arc-shareable type**: ONE predicate `arc_shareable(ty)` in `ynz_typeck::types`, the compile-time floor `arc.rs` documents but no code defines today — a `shape` whose fields are transitively `int`/`float`/`bool`/`string`/inline `shape`. Excluded: `number` fields (16-byte alignment; the block's data is 8-aligned), `array`/`map`/`maybe`/union fields (pointer cells whose ownership the block cannot express — sharing the outer bytes would alias the inner allocation between tasks with no count on it), and every non-shape type (`array<int>` bg args are cloned by `ynz_array_clone_primitive`; refcounting a header-plus-buffer pair is a different substrate, named in the residual).

Loops are out: `for (…) { background render(scene) }` is ONE spawn statement, so no group forms and the shipped per-iteration copy runs. A cross-iteration Arc would need a caller-held reference that outlives the statement — the drop-pass dependency again. Recorded in the residual.

When any condition fails, **nothing changes**: the shipped `Give`/`Copy` inference records exactly what it records today and `prepare_bg_arg_for_ctx` heap-copies per task. Phase 5 step 3 confirms that path byte-identical.

### What typeck records and what codegen reads (no re-derivation)

`BgOwnership` gains `Arc { group: GroupId, first: bool, last: bool }`, recorded for every member of an admitted group by the ONE spawn-argument recording function all three recording sites share (the statement-form pass, the handle-form pre-record at `check.rs:2321–2345`, and the `Expr::Background` backstop at `:3283`). That shared function also fixes parked item 16: the handle form records `Copy` unconditionally today even when the callee's `give` consumed the binding, so a hint reading `bg_inferred` would say "copied" for a value that was given. Codegen reads the recorded variant and emits `arc_new`+memcpy+`clone` (first), `clone` (member), `arc_free(transient)` after the last; it consults no ownership fact of its own.

The runtime's release protocol (`release_ladder_payload`, pointer identity) never meets an Arc'd shape: shapes are not channel elements (deferred) and are returned by value (v0.3-M7 R9). Phase 5 still decides `bg_arg_kind_is_releasable_payload(BG_ARG_KIND_ARC_SHAPE)` explicitly and adds the per-kind parity case Phase 4's `ALL_BG_ARG_KINDS` test requires — an Arc kind that were ever "released" would leak one count, never free early.

### Override directions ([`auto-promotion.md`](../../../.claude/rules/auto-promotion.md))

- **Force the OTHER pick (independent copies despite a group)**: write `background render(scene.copy())` at the spawn you want independent. `.copy()` is `Provenance::Fresh`, not `Whole`, so that spawn is not a group member; it takes the shipped per-task copy path (`prepare_bg_arg_for_ctx`'s explicit-`.copy()` arm). Existing, typeable syntax; no new API. **Correction to earlier text**: [`IMP-concurrency.md`](IMP-concurrency.md) "Ownership with Background Tasks" and the v0.3-M8 plan both named `.give` as a spawn-site override. No body-level `.give` exists — `PostfixOpKind` is `Copy | Freeze`, and this document's read-first note says so. The give direction is already what the liveness inference does for a binding unused after the spawn; there is nothing to force.
- **Force the AUTO pick (Arc when the compiler would not)**: deliberately no override. Every decline above is a soundness decline (a writer, an unprovable callee, a type the block cannot express) or a zero-benefit case (one task, a loop); forcing sharing through any of them either races or costs more than it saves.
- **`share` at a `background` boundary** stays a compile error. It is not reinterpreted as "Arc this": a `share` parameter is a borrow of the caller's frame, and the spawned task may outlive that frame; the Arc block is not the caller's value, it is a copy the tasks own.

### The muted hint (`auto_arc` domain, Informational category; Phase 5 wires it, registry text updated then)

Inline: `// shared by reference count with {n} tasks — read-only`. Hover, tied to the actual group:

> **WHAT**: This value is copied once into a shared block, and each of the {n} background tasks spawned here holds a counted reference to that one copy instead of getting its own full copy. Your own binding is untouched — you keep reading your original.
> **WHAT INSTEAD**: Nothing to type — the compiler picks this when {n} tasks only read the value and nothing changes it between the spawns. If one task needs its own version to change, pass `{name}.copy()` at that spawn so it gets an independent copy.
> **WHY**: {n} tasks read this value, so one shared copy saves {n − 1} full copies. Each task pays a small atomic bump when it takes and drops its reference — cheaper than a copy here, which is why the compiler shares only when at least two tasks read it.

The red-tint styling stays under `auto-arc-cautionary-tint` (no per-hint tint path in `ynz-lsp`); the caution is in the words.

### Residual (Phase 5 step 7 narrows `auto-arc-codegen-emission` to this, or retires it if it ships)

- Spawn groups that straddle a suspension point (needs the transient to be a crossing local).
- Spawn groups inside a loop (needs a caller-held reference across iterations — the drop pass).
- `array`/`map` values and shapes with pointer-cell or `number` fields (a different sharing substrate; `arc_shareable` is the floor).

### Cross-references

- [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) §Runtime item 4 — the pointer this section resolves; "False Sharing Auto-Padding" — the transform whose throughput benefit arrives with this emission.
- [`IMP-concurrency.md`](IMP-concurrency.md) "Ownership with Background Tasks — Why `.share` Fails" — the inference table this refines.
- `registry/features.toml` `auto-arc-codegen-emission`, `auto-arc-cautionary-tint`, `[[muted_hint_domain]] auto_arc`.
- v0.3-M8 plan Phase 2 (design), Phase 5 (implementation), risk R2 (signed override), Future Requirement #5.
