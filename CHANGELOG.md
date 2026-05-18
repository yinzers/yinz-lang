# Changelog

## v0.1.0-m7 — Full Strings, errors Keyword, Iterables Protocol

Commit range: v0.1.0-m6..v0.1.0-m7

### What's new

M7 is the largest milestone yet — three interlocking pillars that make Yinz programs
feel like a real language: production-quality strings, a first-class failure-handling
system, and a uniform iteration protocol.

**Full Unicode strings.** Yinz now has one string form: backtick-quoted `` `...` ``
with built-in `${}` interpolation and escape sequences (`\n`, `\t`, `\\`, `` \` ``,
`\${`). The old double-quote form no longer exists — a diagnostic redirects any
`"..."` to the backtick form. String equality uses NFC canonical normalisation, so
`` `é` == `é` `` is always `true` regardless of byte representation. All 16 string
methods ship: `.contains`, `.indexOf`, `.startsWith`, `.endsWith`, `.toUpperCase`,
`.toLowerCase`, `.substring`, `.trim`, `.split`, `.replace`, `.byteAt`, `.get`,
`.graphemeAt`, `.count`, `.byteCount`, `.graphemeCount`. The runtime uses
`simdutf8` for UTF-8 validation and `memchr` for SIMD-accelerated search on
patterns ≥ 16 bytes.

**`errors` keyword.** Functions that can fail declare `-> T errors`. Calling an
`errors` function from another `errors` function auto-propagates on failure —
the happy path reads without any boilerplate. Calling from a non-`errors` function
requires explicit handling: `.or(default)` for a fallback or `.failed()` for an
explicit check. The error value carries `.message`, `.suggestions`, `.trace`
(a call chain as `array<Frame>`), and `.source` (where the failure originated).
The flow-sensitive narrowing engine (M6) is extended to `errors`-capable bindings:
`.message` is only valid inside the `if (x.failed())` true-branch.

**Iterable protocol.** `for` loops now dispatch uniformly through the `Iterable<T>`
and `FallibleIterable<T>` contracts. Built-in collections (`array<T>`, `fixed<T>`,
`map<K,V>`, `string`, `Range`) all follow `Iterable<T>` via synthesized wrappers.
User shapes can implement `Iterable<T>` by writing a standalone `next(lend self: Foo)
-> maybe T` function. `range()` is now first-class — storable, passable, returnable.
The four REPLACE-AT M7 markers left by M5 are all gone. For-loop iteration over
strings walks code points.

### Language surface (M7)

- Backtick-only strings with `${}` interpolation, escape sequences, multi-line
- 16 string methods on the built-in `string` type
- String equality: NFC canonical (`` `é` == `é` `` is `true`)
- `for c in someString` — walks code points
- `-> T errors` function return type — fallible functions
- Auto-propagation: first use of an errors-capable value in an `errors` function
  triggers implicit early-return-on-failure
- `.failed()`, `.or(default)`, `.message`, `.suggestions`, `.trace`, `.source`
- `range()` first-class — `let r = range(0, 10); for i in r { ... }`
- User shapes follow `Iterable<T>` via standalone `next()` function
- `for ((k, v) in m)` tuple-destructure for map iteration

### Design decisions locked

M7 P0 locked 24 design questions before a line of code landed:
- SSO 24-byte string struct layout (ABI locked; tag byte at offset 23)
- SIMD crates pinned: `simdutf8=0.1.4`, `memchr=2.7.4`, `unicode-normalization=0.1.24`, `unicase=2.7.0`
- `Frame { file: string, line: maybe int, function: string }` shape
- `SourceLoc { file: string, line: maybe int }` shape
- Frame stack: thread-local, cap-1024, compile-time-emitted (not libunwind)
- `.orSkipFailures()` is pure (no I/O side effects); `.logSkippedFailuresTo(sink)` for logging
- `.withErrors()` returns `Iterable<maybe T errors>`, not `Iterable<Result<T>>`
- 12 new banned-jargon entries: monad, lift, wrap, Result, Option, Either, exception, try, catch, throw, UTF-16, unwrap

### Compiler features

- **`ynz-parser`**: backtick string lexer with interpolation (brace-depth stack for
  nested `{}`), escape processing, `Token::Errors/BacktickString/InterpolationStart/
  InterpolationEnd` (60→64 tokens); `-> T errors` return type; `Expr::InterpolatedString`,
  `Type::ErrorCapable`, `FunctionDecl.errors_capable`; `for ((k,v) in m)` desugaring
- **`ynz-ast`**: `StringPart` enum; `Expr::InterpolatedString`; `Type::ErrorCapable`
- **`ynz-typeck`**: `Type::ErrorsCapable` (18→20 types); flow-sensitive errors tracking
  (`errors_success_narrowed`, `errors_consumed`); string method dispatch (16 methods);
  interpolation stringifiability check; `Iterable<T>` contract verification for user
  shapes; Frame/SourceLoc built-in field dispatch; range-outside-for restriction removed
- **`ynz-codegen`**: `{i64,i64}` errors ABI (mirrors `maybe<T>`); frame push/pop at
  function entry/exit; auto-propagation branch emission; string method call lowering;
  interpolated string builder (`ynz_string_builder_*`); string iteration via
  `ynz_string_codepoint_at`; first-class range as `{i64,i64}` struct; user shape
  `next()` dispatch
- **`ynz-runtime`**: `YnzError`/`YnzFrame` structs; `ynz_frame_push/pop` (cap-1024);
  `ynz_error_new/drop/message`; `ynz_unhandled_error`; NFC equality via
  `unicode-normalization`; 17 string method functions; SIMD search via `memchr`;
  locale-invariant case via `unicase`; `ynz_string_builder_*` family

### Tests

782 tests across 7 crates, all passing. New in M7: 12 lexer tests, 19 parser tests,
29 errors typeck tests, 28 string typeck tests, 21 iterable typeck tests, 6 runtime
unit tests, and 16 integration fixtures (basic/propagation/nested errors, string
methods/interpolation/NFC/boundary, string iteration, first-class range, user
iterables, adversarial OOB/empty/nested).

---

## v0.1.0-m6 — Options + Unions + Narrowing

Commit range: v0.1.0-m5..v0.1.0-m6

### What's new

M6 ships type-driven discrimination — the ability to declare finite sets of named
states (`options`), work with values that can be one of several distinct shapes
(`|` union types), and discriminate between them at compile time with exhaustiveness
checking and flow-sensitive narrowing.

M6 also closes the fallible-conversion catch-up from M2: `(float).toInt()`,
`(number).toInt()`, `string.toInt()`, `string.toFloat()`, and `string.toNumber()`
all return `maybe<T>` and follow locked parsing rules documented in `design/narrowing.md`.

- **`options` types**: `options Status { active, inactive, banned }` declares a finite
  set of named values. Values are `Status.active` etc.; multi-case `if` is exhaustive
  (missing variants are compile errors naming each missing variant). Built-in:
  `SortOrder { asc, desc }` and `Comparison { equal, greater, less }`.
- **`options.toString()`**: returns the variant name as a string at runtime.
- **Union types**: `shape Figure = Circle | Square | Triangle` declares a value that
  can hold any of the listed shapes. `|` in type position. Exhaustive multi-case
  `if` with `is TypeName =>` arms; `else =>` as catch-all.
- **`is`-narrowing**: inside an `is Circle =>` arm, the scrutinee's type is narrowed to
  `Circle` — field access is safe without any cast or `.value`. Works in both multi-case
  form (`if (x) { is Foo => ... }`) and condition form (`if (x is Foo) { ... }`).
- **Shape aliases**: `shape Figure = Circle | Square` declares a named union type using
  the existing `shape` keyword — one keyword for all type declarations.
- **Fallible conversions (M2 catch-up)**:
  - `(int).toInt()` → `int` (identity, infallible)
  - `(float).toInt()` → `maybe<int>` (NaN → none, OOR → none, truncates toward zero)
  - `(number).toInt()` → `maybe<int>` (via decimal128 → float → range-check → truncate)
  - `string.toInt()` → `maybe<int>` (ASCII whitespace strip; `[+-]?[0-9]+` only; no hex/decimal)
  - `string.toFloat()` → `maybe<float>` (decimal + scientific notation; no 0x/0o/0b)
  - `string.toNumber()` → `maybe<number>` (same rules as `.toFloat()`)
- **Early-return narrowing (M5 catch-up)**: `if (!m.exists()) { return }` followed by
  `m.value` is now valid — the compiler proves `m` is non-none after the early exit.
- **M3 catch-up**: `m3_is_type_deferral.ynz` is now a runnable `Circle | Square` union
  demo; the M3 deferral diagnostic is gone.

### Design decisions locked

Three new design files document every M6 decision before any code landed:
`design/options.md` (LLVM i8 lowering, exhaustiveness, ambiguous-shorthand resolution),
`design/unions.md` (tagged-struct layout, `is`-exact-type rule, single-variant rejection),
`design/narrowing.md` (18-row flow-sensitive rules table, recognized-exit set, locked
`||` non-propagation diagnostic text).

### Compiler features

- **`ynz-parser`**: `Token::Options`, `Token::Is` (58→60 tokens); options declaration
  parser; union type in type position; `Is`/`OptionName` arm forms; `Expr::Is` for
  `if (x is Foo)` condition form; `shape Name = Type` alias form; M3 deferral removed.
- **`ynz-ast`**: `Item::OptionsDecl`, `Type::Union`, `TypePath`, `MatchPatternKind::Is`/
  `OptionName`, `Expr::Is`, `ShapeDecl.alias_ty`.
- **`ynz-typeck`**: `OptionsTable` (collection + validation); union alias resolution via
  `ShapeTable.union_aliases`; options/union exhaustiveness; `is`-narrowing; early-return
  narrowing accumulator in `check_stmts`; fallible conversion intrinsics; `check_is_expr`.
- **`ynz-codegen`**: options i8 constants + multi-case switch + `toString` via conditional
  branch to `ynz_string_from_static`; union `{ i64 tag, i64 data }` construction on
  assignment; `Is`-arm tag load + compare; `(float).toInt()` locked IR sequence
  (`fcmp uno` + range-check + raw `fptosi` — NOT `fptosi.sat`); string conversion dispatch.
- **`ynz-runtime`**: `ynz_string_to_int/float/number` (locked parsing rules), `ynz_string_from_static`,
  `ynz_decimal_to_float`.

### Tests

631 tests across 8 crates, all passing. New in M6: 24 runtime unit tests for
string-parsing locked test vectors; 13 typeck tests for options/union semantics;
2 new integration tests for string-conversion catch-up fixtures; 7 new parser tests.


## v0.1.0-m4 — Shapes, Methods, Ownership

Commit range: v0.1.0-m3..v0.1.0-m4

### What's new

- **Shape declarations**: `shape Foo { field: Type }` defines a user data type.
  All fields are required in struct literals. Structural typing: `let p: Player = { name: "x", health: 1 }`.
- **Methods via UFCS**: standalone functions with `self: ShapeName` as first param
  are callable as `value.method()` or `method(value)`. Both are equivalent.
  Yinz is not object-oriented — methods live outside shape bodies.
- **Ownership modifiers**: `share self` (read-only borrow), `lend self` (mutable borrow),
  `give p: T` (ownership transfer). Inferred at call sites; declared in signatures.
- **Ownership analysis**: `const` bindings block all mutation paths. Use-after-give
  is a compile error with both give-site and use-site named in the diagnostic.
- **`extends` (data-only inheritance)**: child inherits parent fields prepended to its own.
- **`follows` (structural contracts)**: verified at compile time against standalone functions.
- **`base shape`**: cannot be instantiated; must be extended. Compile error on attempt.
- **`hidden` fields**: visible only inside the declaring shape's own methods.
- **LLVM ownership attributes**: `share T` → `readonly + noalias`; `lend T` → `noalias`;
  `give T` → neither. Verified by IR snapshot.
- **Runtime shims**: `ynz_alloc` / `ynz_free` added; stack allocation used by default.
- **`.copy()` and `.freeze()`**: trivial struct memcopy; binding mutability lock.
- **M2 catch-up — overflow escape**: `.wrappingAdd/Sub/Mul()` and `.saturatingAdd/Sub/Mul()`
  on `int` via LLVM wrapping arithmetic and `sadd.sat` / `ssub.sat` / `smul.fix.sat`.
- **M2 catch-up — type-attached constants**: `int.max`, `int.min`, `number.epsilon`,
  `number.max`, `number.min`, `float.max`, `float.min`, `float.epsilon`.

### Test count

M3: 310 tests → M4: **316 tests** (added 6 positive + 10 negative M4 integration
fixtures, codegen golden tests, jargon audit).

### Breaking changes

None — all M3 programs compile unchanged under M4.

---

## v0.1.0-m3 — Control Flow + User Functions

### What's new

- **User-defined functions**: Multiple functions per file, parameters with type
  annotations, return types declared on every function, early `return` statements,
  mutual recursion supported via two-pass signature pre-pass.
- **`if` statement**: `if (condition) { body }` with no standalone `else` block —
  early-return and pre-assignment patterns handle alternation.
- **Multi-case `if`**: `if (scrutinee) { 1 => ...; 2 => ...; else => ... }` for
  value-based branching on `int`, `string`, `float`, and `bool`. String comparison
  uses byte-equality via `ynz_string_eq` (Unicode canonical equivalence in M7).
- **`while` loop**: `while (condition) { body }` with full type checking on the bool condition.
- **`for` loop**: `for (i in range(0, n)) { body }` with a temporary `range` builtin
  (replaced by `Iterable[T]` protocol in M7). Loop variable is immutable inside the body.
- **Block scoping**: each `{}` block pushes/pops a scope; shadowing is allowed.
- **Return-path analysis**: non-`nothing` functions must return on every path or get
  a compile error naming the uncovered path. Dead code after a definite `return`
  emits a warning.
- **Parameter read-only enforcement**: assignment to a parameter is a compile error
  with an M4-deferral diagnostic pointing at the `lend` ownership modifier.
- **Dead-code warnings**: code after a definite return renders to stderr even on
  successful builds.
- **Deferred-feature teaching diagnostics**: `is TypeName =>` arms point to M6,
  `share`/`lend`/`give` parameter annotations point to M4, `range` outside
  for-loop position points to M7.
- **`match`/`switch` banned-keyword diagnostics**: teaching messages redirect to
  multi-case `if`.

### Compiler internals

- Two-pass typeck: `module_signatures_query` (salsa) collects all function
  signatures before any body is checked; body typeck depends on this query for
  cross-function call site resolution.
- Return-path analysis in `crates/ynz-typeck/src/return_paths.rs`: pure CFG walk
  over `Block`, no typeck context needed. 7 dedicated unit tests.
- LLVM codegen: two-pass `build_module` forward-declares all functions first
  (mutual recursion), then emits bodies. `lower_stmt_{if,match,while,for,return}`
  helpers. All control-flow uses `alloca`-per-local for uniform variable model;
  LLVM mem2reg elides copies.
- `ynz_string_eq` added to `libynz_rt.a` (pointer arithmetic only; kernel-mode safe).
- Linker now passes `-no-pie` on Linux to match LLVM's non-PIC object output
  (PIE vs non-PIE relocation alignment fix).

## v0.1.0-m2 — Literals, Variables, Arithmetic

### What's new

- **Numeric types**: `int` (i64), `float` (f64), `number` (IEEE 754 decimal128, hand-rolled from scratch with full conformance test suite)
- **Variables**: `let` and `const` declarations with optional type annotations; block-scoped; Levenshtein "did you mean" suggestions on undefined names
- **Arithmetic**: full operator set (`+`, `-`, `*`, `/`, `%`), integer overflow panics, float follows IEEE 754 (no panic on infinity), decimal exact arithmetic (`0.1 + 0.2 == 0.3`)
- **Comparisons and booleans**: `<`, `<=`, `>`, `>=`, `==`, `!=`, `&&`, `||`, `!`, short-circuit evaluation
- **Bitwise operators**: `&`, `|`, `^`, `~`, `<<`, `>>`
- **Type inference**: `let x = 42` infers `int`; `let x = 3.14` infers `number`; annotation overrides default
- **Mixed-type errors**: `int + number` is a compile error with a specific `.toNumber()` suggestion; `number + float` lists both conversion directions and explains the tradeoff
- **Conversion methods**: `.toNumber()`, `.toFloat()`, `.toString()` on all primitive types
- **Polymorphic `print`**: accepts `int`, `float`, `number`, `bool`, and `string`
- **Comments**: `//` line comments

### Compiler internals

- Pratt precedence climber (12-level table, mechanically verified against `spec/operators.md`)
- `PrimitiveIntrinsicTable` replaces M1's `BuiltinTable`; single source of truth for all built-in method dispatch
- Block-scoped variable environment with `is_const` tracking
- LLVM codegen for all M2 constructs: int overflow via `llvm.sadd/ssub/smul.with.overflow.i64`, decimal128 via `ynz-runtime` C ABI, short-circuit `&&`/`||` with phi nodes
- Runtime panic stubs for overflow and division by zero (three-part diagnostic to stderr + abort)
- `expr_types` keyed by `(span.start, span.end)` — fixes span collision between BinOp parent and leftmost child

### Spec

- `spec/operators.md`: added `%` to operator lists and precedence table (level 3)
- `spec/variables.md`: corrected `// compiler knows: number` → `// compiler knows: int`
- `spec/numeric-types.md`: replaced wrong "promotes to most capable" claim with compile-error behavior + example

### Deferred (tracked as catch-up entries)

- `number[N]` for N > 34 (bignum) — M8
- Overflow escape valves (`.wrappingAdd()` etc.) — M4
- Fallible conversions (`.toInt()`) — M6
- Type-attached constants (`int.max`) — M4

### Integration test

```
$ ynz run m2_smoke.ynz
0.3
1763
true
```

---

## v0.1.0-m1 — Hello World

- `ynz run hello.ynz` → `hello, yinz`
- Full pipeline: lex → parse → typecheck → LLVM codegen → link → execute
- All passes wired as salsa queries for incremental rebuilds
- Three-part diagnostic format (WHAT / WHAT-INSTEAD / WHY) with ariadne rendering
- Banned-jargon CI gate
