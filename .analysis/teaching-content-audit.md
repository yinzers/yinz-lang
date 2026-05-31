# Yinz LSP Teaching-Surface Audit (Golden Rule 11)

**Audit date**: 2026-05-21
**Scope**: 4 surfaces — inlay hint render text + hover tooltips, registry hover docs, diagnostic emissions in `check.rs`, code-action labels.
**Verdict**: Mostly passable for diagnostics; **catastrophic gaps** in registry hover docs (96/97 primitive methods have ZERO documentation) and inlay hint render text (no binding name, no concrete sizes, no contextual WHY).

---

## TOP 10 Highest-Leverage Fixes (where the user will FEEL the difference)

1. **96 of 97 primitive_intrinsics ship with no `doc` field** → hover on `.toUpperCase()`, `.add()`, `.sort()`, `.contains()`, etc. shows only the signature. No examples, no WHY. This is the user's primary discovery surface for the stdlib-of-primitives. Single highest-leverage fix in the audit.
2. **Inlay hint labels lack binding names AND concrete sizes** → `// promoted to fixed — never grown` should be `// promoted to fixed<int, 3> — nums never grown after declaration`. The example in `.claude/rules/inference.md` is more specific than the actual code; the spec is ahead of the implementation.
3. **`muted_hint_domain` WHY text is GENERIC across all 9 domains** → "The compiler figured this out from context. Click to make it explicit in source." applies identically to every Addition-category hint. Violates Rule 11's "specific and contextual" requirement.
4. **Two `Diagnostic::error` WHY strings use banned jargon `infers`** (lines 1835, 1848 of check.rs) — `crates/ynz-diagnostics/src/banned_jargon.rs` bans `infer`/`inference` in user-facing text. Compiler error currently violates its own rule.
5. **Typo "booleanean" at line 1441** of `check.rs` — visible in `print` error WHY text. Looks like a botched sed `bool` → `boolean`.
6. **`TypeMismatch` diagnostic template WHY is generic boilerplate** — "Yinz is strictly typed — every value must exactly match the expected type." Applies to every type mismatch the compiler will ever emit. Useless. WHY must name the specific bind / call / position.
7. **Sized-integer deferred-feature WHYs are 95% identical** (i8/i16/i32/i64/u8/u16/u32/u64) — same paragraph copy-pasted 8 times. A user typing `u16` hovers and reads boilerplate about "cognitive load" that doesn't address their actual case (e.g., FFI struct packing).
8. **MutationOfConst template substitutes `{binding}` mechanically** without explaining WHEN to choose const vs let — WHY ends at "compiler can optimize" without saying what the user gains in practice (`readonly` LLVM attr, lint suggestion stability, etc.).
9. **Code-action label is bare "Replace `X` with `Y`"** — no follow-up hint about WHY the user wants this. A junior user clicks `class` and gets "shape" without learning what just happened. Code-action `description` (LSP optional field) is unused.
10. **`ImportNotFound` WHY claims "Yinz import paths are root-relative" without telling the user where root IS** — "Check that the path is correct" is the WHAT-INSTEAD, but the WHY should explain the root-relative resolution at the specific failure point (e.g., "the entry file is `entrypoint.ynz`; paths resolve from that file's directory").

---

## Surface 1: Inlay Hint Render Text + Hover Tooltips

### Inlay hint render text (in `inlay_hint_passes.rs`)

| Location | Surface | Current text | Quality verdict | Suggested improvement |
|---|---|---|---|---|
| `inlay_hint_passes.rs:243` | variable_type render | `: TypeName` (just the type) | ⚠ Generic WHY | Spec wants `: int (from 42)` — include initializer hint. Currently dropped on the floor; `TypeHint` struct has no provenance field. |
| `inlay_hint_passes.rs:350` | ownership_call_site render | `"share"` / `"lend"` / `"give"` (single word) | ⚠ Missing WHAT-INSTEAD | Spec wants `share (read-only — matches foo's signature)`. The implementation strips the contextual reason; user just sees a keyword. |
| `inlay_hint_passes.rs:427` | copy_points render | `.copy (N bytes, trivially copyable)` (from `copy_size_text`) | ✓ Good | Has size + reason. This is the model the other domains should follow. |
| `inlay_hint_passes.rs:466` | array_to_fixed_promotion label | `"// promoted to fixed — never grown"` | ⚠ Generic WHY | Spec example: `// promoted to fixed<int, 3> — nums never grown after declaration`. Drop in concrete type AND binding name. |
| `inlay_hint_passes.rs:517` | let_to_const_promotion label | `"// effectively const — never reassigned"` | ⚠ Generic WHY | Should name the binding: `// effectively const — count never reassigned, mutated, or lent after declaration`. |

### Hover tooltips (in `ynz-registry/src/lib.rs::lsp_inlay_hint_hover_for`)

The function generates hovers from `muted_hint_domain` entries (`features.toml:1878+`). The WHAT is from `description`; WHAT-INSTEAD is `example_hint_rendered`; WHY is one of three generic sentences by `placement_category`.

| Domain | WHAT (description) | WHY (generated per category) | Quality verdict | Suggested improvement |
|---|---|---|---|---|
| `variable_type` | "Infers the type of a variable from its initializer expression" | "The compiler figured this out from context. Click to make it explicit in source." | ✗ Banned-jargon (`infers`) + Generic WHY | "Yinz looks at the right-hand side to figure out the type — `42` is an int, so `x` is an int. Adding the annotation makes the type explicit at the binding site, useful when the right-hand side is far away or complex." |
| `function_param_type` | "Infers a function parameter type from the call-site argument" | (same generic) | ✗ Banned-jargon | "When you write `foo(x => x + 1)`, the compiler looks at `foo`'s signature to figure out what type `x` must be." |
| `ownership_call_site` | "Shows the ownership modifier the compiler infers at a function call site" | "This shows what the compiler decided at this call site. No source change is needed." | ✗ Banned-jargon (`infers`) + Generic WHY | "`foo(player)` shows `share` because `player` is `const` AND `foo`'s signature declares the first parameter as `share`. The compiler picks based on both sides. To force a different ownership, declare `player` differently or use a function with a different signature." |
| `wait_points` | "Shows where the compiler auto-inserts a wait at an I/O suspension point" | "The compiler figured this out from context. Click to make it explicit in source." | ⚠ Generic WHY (also deferred until v0.3) | "I/O calls suspend the current task until the OS returns data. Yinz inserts a `wait` for you so the scheduler can run other tasks during the suspension — but writing `wait` explicitly tells future readers this line might pause." |
| `lifetimes` | "Shows inferred lifetimes (shown only on user request — usually noise)" | (informational generic) | ✗ Banned-jargon (`inferred`) + Generic | Rewrite or remove the entry entirely until lifetimes have user-facing surface. |
| `allocators` | "Shows the inferred allocator for a new value inside an arena scope" | (addition generic) | ✗ Banned-jargon | "Inside an `arena scratch { }` block, new collections use the arena allocator automatically — no per-allocation heap call. Writing `.in(scratch)` explicitly makes the allocator visible." |
| `copy_points` | "Shows where a trivially-copyable value is implicitly copied at a call site" | (addition generic) | ⚠ Generic WHY | "`int`, `float`, `bool`, and `number` are passed by copy — they fit in a register and don't own any heap memory. No `.share`/`.lend`/`.give` needed (and none allowed)." |
| `array_to_fixed_promotion` | "Shows when the compiler promotes array<T> to fixed<T> because the value is never grown" | "The compiler picked the stricter form automatically. Hover to see the alternative you could write explicitly." | ⚠ Generic WHY | "`nums` is declared `array<int>` but never has `.add()`, `.remove()`, or any mutation call. The compiler emits `fixed<int, 3>` codegen (stack-allocated, no heap call). Writing `let nums: fixed<int, 3>` makes that explicit AND a future `.add()` becomes a compile error rather than silently going back to heap." |
| `let_to_const_promotion` | "Shows when a let binding is effectively const because it is never reassigned, mutated, or lent" | (replacement generic) | ⚠ Generic WHY | "`count` is never reassigned, never has a mutation method called, and never passed to a `lend` parameter. The compiler treats it as `const` for optimization (LLVM `readonly` attribute). Writing `const count` makes the intent explicit and turns any future mutation into a compile error." |

**Pattern observed**: the WHY text is mechanically generated per `placement_category`, so all 5 Addition-category hints share one WHY. Rule 11 explicitly bans this: "the WHY must be **specific and contextual** — not generic ('avoids allocation') but tied to the actual call site." The current code violates Rule 11 by design.

**Action**: replace the `match entry.placement_category` block in `lsp_inlay_hint_hover_for` with a per-domain WHY field on `MutedHintDomainEntry` (sourced from `features.toml`).

---

## Surface 2: Registry Hover Docs

### `[[keyword]]` entries (29 entries, since-tag only)

| Entry | Current hover | Quality verdict | Suggested improvement |
|---|---|---|---|
| `function`, `let`, `const`, `if`, `else`, `while`, `for`, `in`, `return`, `nothing`, `true`, `false`, `shape`, `follows`, `extends`, `base`, `hidden`, `dynamic`, `Self`, `self`, `none`, `options`, `is`, `errors`, `import`, `export`, `sensitive`, `wait`, `background` | Format: `## Keyword: \`function\`\n\nIntroduced in M1.` | ✗ Missing entirely | Every keyword needs a `hover_summary` field (1-line description) and `hover_detail` (longer text with example). E.g., `wait`: summary "Force completion of an async call before continuing." detail "`wait foo()` suspends the current function until `foo()` returns. Without `wait`, an async call's result isn't available — the compiler inserts `wait` automatically when needed, but writing it explicitly tells readers this line may pause." |

**Quality verdict for ALL 29**: ✗ Missing entirely.

### `[[primitive_intrinsic]]` entries (97 entries)

96 of 97 have NO `doc` field. The single exception is `array.append()` (line 1678).

| Method | Receiver | Has `doc`? | Quality verdict |
|---|---|---|---|
| `append` | `array` | YES | ✓ Good — names the contrast with `.add()` |
| `toString` | int, float, number, bool | NO | ✗ Missing entirely |
| `toFloat` | int, number | NO | ✗ Missing entirely |
| `toInt` | float, number, string | NO | ✗ Missing entirely (each returns `maybe<int>` — user has NO explanation of when/why this fails) |
| `toNumber` | int, float, string | NO | ✗ Missing entirely |
| `wrappingAdd`/`Sub`/`Mul` | int | NO | ✗ Missing entirely — these are the user's ONLY way to opt out of overflow trapping; without docs they will not know this exists |
| `saturatingAdd`/`Sub`/`Mul` | int | NO | ✗ Missing entirely |
| `contains` | string, array, fixed | NO | ✗ Missing entirely |
| `indexOf` | string | NO | ✗ Missing entirely — returns `maybe<int>` — when does it fail? |
| `startsWith`/`endsWith` | string | NO | ✗ Missing entirely |
| `toUpperCase`/`toLowerCase`/`trim` | string | NO | ✗ Missing entirely (locale handling per stdlib-design.md Rule 3 NOT documented) |
| `count`/`byteCount`/`graphemeCount` | string | NO | ✗ Missing entirely — three count methods, NO hover explaining which to pick (the Yinz answer is critical: codepoint vs UTF-8 byte vs user-perceived character) |
| `get`/`graphemeAt`/`byteAt` | string | NO | ✗ Missing entirely |
| `substring`/`split`/`replace` | string | NO | ✗ Missing entirely |
| `exists`/`or` | maybe<T> | NO | ✗ Missing entirely — `.or(default)` is the primary `maybe<T>` consumption pattern; user must read the spec |
| `get`/`find`/`has`/`set`/`remove`/`clear`/`count`/`keys`/`values`/`entries` | map | NO | ✗ Missing entirely (10 methods, no docs) |
| `add`/`remove`/`removeFirst`/`removeLast`/`clear`/`get`/`first`/`last`/`count`/`contains`/`sort`/`sortFast`/`sortStrict`/`filter`/`unique`/`limit`/`copy`/`prepend`/`concat`/`freeze`/`find`/`set`/`map` | array | NO (except `append`) | ✗ Missing entirely — 23 array methods, ONE has doc. `sort` vs `sortFast` vs `sortStrict` is the critical override-pair from `auto-promotion.md` — user CANNOT discover this without reading design docs |
| 17 methods on `fixed` | fixed | NO | ✗ Missing entirely (mirrors array, same gap) |

**Pattern observed**: the LSP hover handler (`lsp_adapter.rs:226-249`) DOES wire up `doc` into hover output ("if let Some(doc) = e.doc { detail.push_str("\n\n"); detail.push_str(doc); }"). The wire is built; the data is missing. This is a content gap, not an engineering gap.

**Recommended priority**:
- **Tier A — must ship before v0.2 releases publicly**: `sortFast`/`sortStrict` (auto-promotion override hints, critical to learning Yinz's perf model); `count`/`byteCount`/`graphemeCount` (semantically distinct, easy to use wrong); `wrappingAdd`/`saturating*` (only way to opt out of overflow trapping); `toInt`/`toFloat`/`toNumber` on string (each returns `maybe<>` — user needs failure mode explained); `add` vs `append` on array (mutation vs new-value — already half-done by the existing `append` doc).
- **Tier B**: string methods, maybe<T> methods, map methods.
- **Tier C**: pure ergonomic methods (`trim`, `toUpperCase`, `first`, `last`) — short docs sufficient.

### `[[type_attached_constant]]` entries (8 entries)

| Constant | Hover format | Quality verdict | Suggested improvement |
|---|---|---|---|
| `int.max`, `int.min` | `## \`int.max\`\n\nValue: \`9223372036854775807\`\n\nType: \`int\`\n\nIntroduced in M4.` | ⚠ Boring (re-states WHAT) | Add WHY context: "9.2 × 10^18 — bigger than any count a human writes by hand. Used for sentinel values when you need 'no real cap.'" Same pattern as the `u64` deferred-feature substitute text already does. |
| `float.max`, `float.min`, `float.epsilon` | (same format) | ⚠ Boring + Missing WHY for `epsilon` | `float.epsilon`: "The smallest difference two `float` values can have. Use this when comparing floats: `(a - b).abs() < float.epsilon` instead of `a == b`." This is the canonical use-case and isn't documented. |
| `number.max`, `number.min`, `number.epsilon` | (same format) | ⚠ Missing WHY | Same as float but with decimal128 context. |

### `[[banned_jargon]]` entries (~50 entries)

Sampling 12 representative entries:

| Banned word | Replacement message | Quality verdict | Suggested improvement |
|---|---|---|---|
| `propagate` | "auto-propagate (use the `errors` keyword — errors flow up automatically)" | ✓ Good | — |
| `narrow` | "the compiler figures out the specific type from context" | ✓ Good (avoids jargon) | — |
| `infer` | "figure out automatically" | ✓ Good | — |
| `discriminator` | "(describe what distinguishes each case instead)" | ⚠ Generic WHY (parenthetical replacement instead of concrete word) | "the field whose value tells you which case this is" — give the user a concrete phrase they can substitute |
| `polymorphic` | "works with multiple types" | ✓ Good | — |
| `monomorphize` | "generate a type-specific version of" | ✓ Good | — |
| `covariant` / `contravariant` | "(describe the specific relationship instead)" | ⚠ Generic | These rarely come up in user-facing Yinz; if they do, give concrete phrasing per kind |
| `ADT` | "shape or options type" | ✓ Good | — |
| `monad` | "(no direct replacement — describe what the operation does instead)" | ⚠ Generic | Acceptable — `monad` genuinely doesn't have a single replacement. Keep as-is. |
| `lift` | "(describe what the operation does instead)" | ⚠ Generic | Same — acceptable. |
| `Result` | "use the `errors` keyword: `function foo() -> T errors`" | ✓ Good — has concrete example | — |
| `Option` | "`maybe T` — a value that might not be present" | ✓ Good | — |

**Overall**: banned_jargon entries are the strongest registry surface — concrete replacements, mostly Rule 11-compliant. The parenthetical "(describe what distinguishes each case instead)" pattern is the one weak spot — replace with a positive phrase the user can actually type.

### `[[deferred_language_feature]]` entries (~15 entries)

| Entry | Substitute | WHY | Quality verdict |
|---|---|---|---|
| `f32`, `f64`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32` (9 entries) | All identical: "Use `int`/`float`... Sized variants ship in v2+..." | All identical 95% of body text | ✗ Boring (re-states WHAT) — same paragraph 9 times. User hovering `u16` for "I'm doing FFI struct packing" gets boilerplate about "cognitive load." |
| `u64` | "Use `int`... `int` in Yinz covers 0 to 9.2 × 10^18 — bigger than any count a human writes by hand." | (same general WHY) | ✓ Good substitute — names the concrete range | The `u64` entry is the model: concrete numbers, named use-case. Apply this pattern to all sized-integer entries. |
| `test` | "Use a different identifier..." | ✓ Good (concrete, actionable) | — |
| `scratch` | "Use regular `let` bindings for now. Arena allocators ship in v0.2..." | ✓ Good (concrete substitute) | — |
| `gpu`, `foreign` | (concrete substitutes) | ✓ Good | — |
| `self-referential-shape` | "Use a `maybe T` field or restructure to avoid cycles." | ✓ Good (actionable workaround) | — |

**Action**: rewrite the 9 sized-integer entries with concrete use-case-specific WHYs (FFI struct packing → "wait for v2+ where `foreign` blocks accept C ABI types"; bit-flag encoding → "use `int` with bit operators today, sized variants in v2+ give you compile-time bound checks"; etc.).

### `[[diagnostic_template]]` entries (16 entries)

| Template | WHAT | WHAT-INSTEAD | WHY | Verdict |
|---|---|---|---|---|
| `TypeMismatch` | "Expected {expected} here." | "Change the value so it matches {expected}." | "Yinz is strictly typed — every value must exactly match the expected type." | ⚠ Generic WHY (applies to every type mismatch ever) — should reference {expected_origin} (where was the constraint declared?) |
| `MutationOfConst` | "`{binding}` is declared `const` and cannot be changed." | "Change `const {binding}` to `let {binding}` if you need to reassign it." | "`const` bindings are immutable — the compiler can optimize them more aggressively and they signal intent to readers." | ✓ Good (specific to const/let distinction) |
| `NotDefined` | "`{name}` is not defined in this scope." | "Check the spelling, or declare `{name}` before using it." | "Yinz requires every name to be declared before it's used." | ⚠ Generic — already-known general rule, not specific to THIS failure. Should reference closest available name from `make_not_defined_diag` (which already exists in check.rs:489+ as a code path) |
| `MissingField` | "Missing required field `{field}` in the shape literal." | "Add `{field}: <value>` to the shape literal." | "`shape` values must set every field — there are no defaults unless declared `hidden`. This ensures construction sites are explicit." | ✓ Good — names the rule (no defaults) and the rationale (explicit construction) |
| `HiddenAccess` | "`{field}` is a hidden field — only accessible from within its declaring file." | "Use the methods or functions provided by that file to access `{field}`." | "Hidden fields are an encapsulation boundary..." | ✓ Good |
| `ImportNotFound` | "Imported module `{path}` was not found." | "Check that the path is correct (paths are root-relative, starting from the project's entry file)." | "Yinz import paths are root-relative, not relative to the current file." | ⚠ Missing WHY context — WHAT-INSTEAD references "root-relative" but doesn't tell user where root is. Add: "Your project root is `{project_root}`; paths resolve from there." (requires plumbing root into template) |
| `Consumed` | "`{name}` was already given away and cannot be used again." | "Use `.copy()` before giving a value away if you need to keep using it." | "Once a value is given (ownership transferred), the original binding no longer holds a valid value..." | ✓ Good |
| `Borrowed` | "`{name}` is already borrowed and cannot be given or reassigned while the borrow is active." | "End the borrow before giving or reassigning `{name}`." | "Only one mutable borrow of a value can exist at a time. This prevents data races and use-after-move bugs." | ✓ Good |
| `MissingReturn` | "This function does not return a value on all paths." | "Add a `return` statement in every branch, or ensure the last expression is the return value." | "Every code path in a function that declares a return type must actually return a value of that type." | ⚠ Generic WHY (restates WHAT) — should name the specific missing path (e.g., "the `else` branch on line 42 falls through with no return") — requires per-emission customization, not template-rendered |
| `BannedKeyword` | "`{keyword}` is not a Yinz keyword." | "{what_instead}" | "{why}" | ✓ Good (delegates to banned_declaration_keyword entries which DO have specific WHYs) |
| `UnusedImport` | "`{name}` is imported but never used." | "Remove `{name}` from the import list, or use it somewhere in this file." | "Unused imports add noise — every import signals to readers that this file depends on that symbol." | ✓ Good |
| `WatchNoYinzToml` / `WatchChildSpawnFailed` / `WatchFsWatcherInitFailed` / `WatchRssHardStop` / `WatchMemoryPollingUnavailable` | (various — operational watch errors) | (specific instructions) | (specific causes) | ✓ Good — operational errors with concrete what-to-check steps |

### `[[muted_hint_domain]]` description fields

Already covered in Surface 1 above. Key issue: 3 entries use `infer`/`inferred` in `description`, which is the user-facing text rendered into the hover tooltip.

---

## Surface 3: Diagnostic Emission Sites in `check.rs` (sampled 20+)

### Banned-jargon and typo violations

| Line | Diagnostic | Current text | Verdict | Suggested fix |
|---|---|---|---|---|
| 1441 | `print` non-printable WHY | "`print` works with: int, float, number, **booleanean**, string, and any shape." | ✗ Typo | Fix to "boolean" |
| 1835 | Generic fn type-param unresolved WHY | "Yinz **infers** type parameters from the argument types. If there are no arguments, specify the type explicitly." | ✗ Banned-jargon | Replace with "Yinz figures out type parameters from the argument types. If there are no arguments, specify the type explicitly." |
| 1848 | Same diagnostic, multi-tp case WHY | "Yinz **infers** type parameters from the argument types..." | ✗ Banned-jargon | Same fix |

### Quality verdict for sampled emission sites

| Line | Context | WHAT verdict | WHAT-INSTEAD verdict | WHY verdict | Overall |
|---|---|---|---|---|---|
| 245 | Function missing return | ✓ names function + expected type | ✓ concrete examples (return + else =>) | ✓ explains "fall off the end" | ✓ Good |
| 257 | Dead code warning | ✓ clear | ✓ two options | ✓ explains return semantics | ✓ Good |
| 411 | `let h = background fn()` | ✓ clear | ✓ explains drop-binding + v0.3 timeline | ✓ explains fire-and-forget AND roadmap | ✓ Good |
| 444 | Let value type mismatch | ✓ shows both types | ✓ two options | ⚠ "value on the right side must match annotation" — restates WHAT, doesn't name WHY (Yinz is strict, no implicit conversion) | ⚠ Generic WHY |
| 500 | Param reassign | ✓ clear | ✓ workaround | ✓ explains read-only parameter rule | ✓ Good |
| 508 | Loop-var reassign | ✓ clear | ✓ counter workaround | ✓ explains stepping behavior | ✓ Good |
| 516 | Const reassign | ✓ clear | ✓ const→let | ✓ explains const-vs-let | ✓ Good |
| 530 | Assign-type-mismatch | ✓ specific | ✓ specific | ✓ names the binding's declared type AND what new type would do | ✓ Good |
| 553 | If-cond non-bool | ✓ clear | ✓ example | ⚠ "Any other type cannot be used" — circular | ⚠ Generic WHY |
| 664 | Match-arm type mismatch | ✓ specific | ✓ specific | ⚠ "Each arm pattern must have the same type" — generic rule | ⚠ Generic WHY |
| 733 | Non-exhaustive options multi-case | ✓ names missing variants | ✓ concrete arms OR else => | ✓ explains compile-time variant knowledge | ✓ Good |
| 778 | Non-exhaustive union | ✓ same pattern as 733 | ✓ same | ✓ same | ✓ Good |
| 803 | While-cond non-bool | (mirror of 553) | (mirror) | ⚠ Generic | ⚠ Generic WHY |
| 836 | For-loop over non-iterable | ✓ names type | ✓ lists iterables + custom path | ✓ names Iterable<T> protocol explicitly | ✓ Good |
| 879 | Shape.next() wrong return type | ✓ shows mismatch | ✓ concrete rewrite | ✓ names contract + none-sentinel | ✓ Good |
| 891 | Shape doesn't follow Iterable | ✓ clear | ✓ concrete add | ✓ names contract + none | ✓ Good |
| 906 | Return without value, non-Nothing fn | ✓ clear | ✓ concrete | ✓ explains every-path rule | ✓ Good |
| 918 | Return with value, Nothing fn | ✓ clear | ✓ concrete | ✓ explains nothing-fn semantics | ✓ Good |
| 939 | Return-type mismatch | ✓ specific | ✓ concrete | ⚠ "function's declared return type is..." — circular | ⚠ Generic WHY |
| 1018 | 1-arg intrinsic arg-type mismatch | ✓ shows method + types | ✓ concrete | ⚠ "primitive arithmetic operation that only works on int" — could say WHY (overflow trapping, register width) | ⚠ Generic WHY |
| 1091 | `none` with no annotation | ✓ clear | ✓ example | ✓ explains maybe<T> + T inference | ✓ Good |
| 1101 | `none` in non-maybe slot | ✓ clear | ✓ concrete maybe<int> | ✓ explains valid placement | ✓ Good |
| 1129 | Bracket access on unsupported type | ✓ clear | ✓ lists supporting types | ⚠ WHY restates options; could explain "Yinz requires indexable proof" | ⚠ Generic WHY |
| 1160 | Non-stringifiable interpolation | ✓ clear | ✓ concrete toString() | ✓ explains interpolation mechanism | ✓ Good |
| 1190 | `background <non-call>` | ✓ clear | ✓ example | ✓ explains background semantics | ✓ Good |
| 1219 | `background fn` with share-borrow | ✓ clear | ✓ concrete .copy() | ✓ excellent — names memory-safety hole + workaround | ✓ Excellent (model for others) |
| 1245 | Use-after-give | ✓ names binding | ✓ .copy() | ✓ explains ownership | ✓ Good |
| 1386 | `print` arg-count mismatch | ✓ clear | ✓ concrete | ✓ explains multi-print pattern | ✓ Good |
| 1416 | `print` of ErrorsCapable | ✓ clear | ✓ three options | ✓ explains failure-propagation requirement | ✓ Good |
| 1434 | `print` of non-printable | ✓ clear | ✓ context-aware (collection vs scalar) | ⚠ typo "booleanean" + lists not exhaustive | ✗ Typo |
| 1455 | range arg non-int | ✓ clear | ✓ example | ✓ explains range semantics | ✓ Good |
| 1469 | range arg-count | ✓ clear | ✓ two forms | ✓ explains both | ✓ Good |
| 1502 | Give a const | ✓ clear | ✓ const→let | ✓ explains read-only | ✓ Good |
| 1516 | Lend a const | ✓ clear (mentions fn name) | ✓ specific | ✓ explains lend semantics | ✓ Good |
| 1538/1776 | Arg-count mismatch (user fn / generic) | ✓ specific | ✓ specific | ⚠ "Every function call must match" — generic rule | ⚠ Generic WHY |
| 1652 | `%` on number | ✓ specific | ✓ workaround | ✓ excellent — cites IEEE 754-2008 §5.3.1 + names rounding-mode issue | ✓ Excellent (model for others) |
| 1686 | Cross-options-type compare | ✓ specific | ✓ same-type/convert | ✓ explains shared-meaning issue | ✓ Good |
| 1728 | Unary `-` on bad type | ✓ specific | ✓ lists valid | ✓ explains negation semantics | ✓ Good |
| 1944 | `.toInt()` on bool | ✓ specific | ✓ if-expression workaround | ✓ names coercion-bug class | ✓ Good |
| 1965 | array unknown method | ✓ specific (with elem type) | ✓ lists available | ⚠ WHY restates WHAT | ⚠ Generic WHY |
| 1979 | fixed unknown method | ✓ specific | ✓ lists available + .add/.remove note | ✓ explains fixed-vs-array | ✓ Good |
| 1993 | maybe unknown method | ✓ specific | ✓ lists | ✓ explains .value-after-exists pattern | ✓ Good |
| 2069 | UFCS first-param mismatch | ✓ specific | ✓ concrete signature | ✓ explains UFCS sugar | ✓ Good |
| 2078 | No fn defined for shape | ✓ specific | ✓ concrete signature | ✓ explains UFCS + both call forms | ✓ Good |

### Summary verdict on Surface 3

- **Good (with specific contextual WHY)**: ~32 of 45 sampled — including some excellent diagnostics that should be the model (line 1219 background+share, line 1652 `%` on number with IEEE citation).
- **Generic WHY (restates WHAT or general Yinz rule)**: ~10 of 45. Pattern: "Yinz is strict, period." or "every function call must match." Worst offenders are the `if`/`while`/`for-cond` series and the arg-count mismatches. Fix: name the SPECIFIC mismatch ("the cond on line N produced `int`, not `boolean` — `if` always needs `boolean`").
- **Banned-jargon / typo**: 3 sites (lines 1441, 1835, 1848). Mechanical fix.

---

## Surface 4: Code-Action Labels (`code_action.rs`)

| Label source | Format | Quality verdict | Suggested improvement |
|---|---|---|---|
| `lsp_code_action_label_for` (registry, line 169) | `"Replace `\{token}` with `\{replacement}`"` | ✓ Clear (NOT a generic "Quick fix") | Add a SECOND code-action attribute: `description` (LSP optional) with the WHY — "Yinz uses `shape` for all data declarations. See Spec: type-system.md." Users discover the WHY only by hovering the diagnostic, not the action itself. |
| Auto-import (line 186) | `"Import `\{name}` from `\{import_path}`"` | ✓ Good | — |
| Remove unused import (line 259) | `"Remove unused import `\{name}`"` | ✓ Good | — |
| `BannedKeyword` for `async` (no replacement) | Returns None (no action offered) | ⚠ Missing | User sees `async` flagged but gets no quick-fix. Should still offer a non-edit code action: "Learn how `async` maps to Yinz" → opens hover to the deferred-feature entry. Currently a dead-end. |

**Pattern observed**: code-action titles are concrete (good) but unaccompanied by WHY/context. Users learn nothing from clicking "Replace `class` with `shape`" — they get a clean diff and no education. The fix is one LSP field (`description`) and one WHY string per code-action template.

---

## Action Items (Prioritized)

1. **Fix typo** at `check.rs:1441` (`booleanean` → `boolean`) — mechanical, 30-second fix.
2. **Replace `infers` in two WHY strings** at `check.rs:1835, 1848` with "figures out" — banned-jargon compliance.
3. **Add per-domain WHY field to `MutedHintDomainEntry`** + populate 9 domain-specific WHYs in `features.toml` + update `lsp_inlay_hint_hover_for` to use it. Removes the generic-WHY-per-category violation of Rule 11.
4. **Add `doc` field to all 96 doc-less primitive_intrinsic entries** in `features.toml`. Tier A (~15 entries) is blocking for v0.2 release credibility; Tier B/C can phase in. The wire is already built — content gap only.
5. **Rewrite 8 sized-integer deferred-feature WHYs** to be use-case-specific (one paragraph per integer type, naming the FFI/bit-flag/embedded-target case). Use the existing `u64` entry as the model.
6. **Add `hover_summary` and `hover_detail` fields to all 29 `[[keyword]]` entries**. Update `lsp_hover_for_token` to consume them. Without this, every keyword hover shows just "Introduced in Mx" which is a teaching dead-end.
7. **Replace generic WHYs in 10 diagnostic emissions** in `check.rs` (catalog: lines 444, 553, 664, 803, 939, 1018, 1129, 1538, 1776, 1965). The fix is uniform: name the specific constraint origin (which declaration, which call site) and not the general Yinz rule.
8. **Update inlay hint render text** to include binding names + concrete sizes per the `.claude/rules/inference.md` spec. Affects `array_to_fixed_promotion`, `let_to_const_promotion`, `variable_type` (provenance string), and `ownership_call_site` (contextual reason).
9. **Add `description` field to all code actions** carrying the WHY (one line). Wire it through the existing `CodeAction` struct (`code_action.rs:120, 185, 258`).
10. **Audit `TypeMismatch`, `NotDefined`, `MissingReturn`, `ImportNotFound`** template WHY strings — all currently violate Rule 11's specificity requirement. These four are the most-frequently-emitted diagnostics; fix them and the perceived teaching quality jumps measurably.

---

## Cross-References

- `/workspaces/ynz/CLAUDE.md` — Golden Rule 11 (compiler is a teacher)
- `/workspaces/ynz/.claude/rules/inference.md` — muted-hint protocol with the spec-vs-implementation gap on render text
- `/workspaces/ynz/.claude/rules/vocabulary.md` — banned-jargon list
- `/workspaces/ynz/crates/ynz-typeck/src/inlay_hint_passes.rs:466, 517` — inlay hint label sites
- `/workspaces/ynz/crates/ynz-registry/src/lib.rs:111-145` — `lsp_inlay_hint_hover_for` (the generic-WHY generator)
- `/workspaces/ynz/crates/ynz-registry/src/lsp_adapter.rs:216-303` — `lsp_hover_for_token` (consumes `doc` field; data missing)
- `/workspaces/ynz/registry/features.toml` — all registry entries
- `/workspaces/ynz/crates/ynz-typeck/src/check.rs:1441, 1835, 1848` — banned-jargon + typo violations
- `/workspaces/ynz/crates/ynz-lsp/src/code_action.rs` — code-action labels
