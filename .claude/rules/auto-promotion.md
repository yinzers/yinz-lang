# Auto-Promotion Rule — Compiler Picks the Stricter/Faster Form

This rule captures Yinz's load-bearing commitment to "fast by design even for inexperienced developers." Whenever the compiler can prove a stricter or faster form of code fits the user's actual usage, it picks that form automatically AND surfaces the choice through teaching surfaces so the user learns over time.

Loaded when designing any new language feature, stdlib type, or compiler optimization. Plan files for milestones must check against this rule (see `.claude/rules/plan-invariants.md` `### Performance` subsection requirement).

---

## The Pattern

When the compiler can prove a stricter/faster form fits the user's actual usage, **three things happen**:

1. **Codegen auto-promotion (silent perf win)**: the compiler emits the stricter form's machine code. The user gets the perf benefit automatically without rewriting source.
2. **Muted IDE hint (informational, always-on)**: per `.claude/rules/inference.md`, the IDE renders a muted-text annotation showing what the compiler decided AND a one-clause "why." Click-to-make-explicit converts the source — IF a typeable explicit form exists.
3. **Tier 3 lint suggestion (teaching, yellow squiggle)**: per `design/linting.md`, a lint rule recommends rewriting the source to the explicit stricter form for code-review clarity and future-proofing.

The criterion for each surface:

| Surface | Applies when |
|---|---|
| Codegen auto-promotion | Compiler can prove the stricter form fits AND the stricter form has a measurable benefit (perf, memory, safety) |
| Muted hint | The stricter explicit form is typeable Yinz syntax (so click-to-make-explicit produces real source) |
| Tier 3 lint suggestion | Explicit form would benefit code review (forces future modifications to break compile rather than silently regress) |

When the explicit form has NO typeable syntax (e.g., auto-SoA layout transform — there's no `soa array<Player>` keyword), the muted-hint protocol does NOT apply. Use codegen + Tier 3 lint only.

When the perf difference is purely compile-time (no runtime cost change), skip the codegen change and use just the lint surface.

---

## Why This Rule Is Load-Bearing

Yinz's positioning depends on it: "Rust-level performance with TypeScript-level readability, approachable by jr devs who don't know systems programming."

Without auto-promotion, beginners write the slow form (`array<T>` everywhere, `let` for everything, manual SoA never) and stay slow until they read enough code to learn the perf-correct form. With auto-promotion:

- Beginners write whatever feels natural — get fast code automatically
- The lint suggestion teaches them the explicit form over time, so they grow into perf-aware coders
- The muted hint shows them what the compiler decided, so they can reason about behavior without it being magic

This is the practical mechanism behind Golden Rule 10 ("efficiency first, dynamic after"). Without a documented pattern, contributors will half-implement it (some features auto-promote silently with no teaching; others require the user to manually rewrite source for the perf win). Both halves are necessary; this rule documents both.

---

## Examples (Current and Locked)

### Both surfaces apply (typeable explicit form exists)

- **`array<T>` → `fixed<T>`** when never grown. Codegen + muted hint + Tier 3 lint (`prefer-fixed-when-immutable`). See `design/collections.md` "Auto-promotion" section for canonical rationale.
- **`let` → `const`** when never reassigned/mutated/lent. Lint surface (`mutable-when-const-suffices` in `design/linting.md`); codegen difference is minimal but the source-level explicit form is high-value. Per the inference rule, the muted hint also applies because `const` IS typeable.

### Codegen-only (no typeable form)

- **Auto-SoA layout transform** (`design/future/auto-soa.md`): compiler picks Struct-of-Arrays for hot loops. No source-level `soa` keyword. Codegen + Tier 3 lint (no muted hint).
- **Auto-Arc inference** for cross-thread shared state (`design/future/concurrency.md`): codegen-only; no source-level `Arc<T>` type to make explicit.
- **Auto-`wait` insertion** at I/O suspension points (`design/future/concurrency.md`): the muted hint DOES apply here because `wait` IS typeable. Edge case — codegen + muted hint, but no lint suggestion (writing explicit `wait` everywhere would be noise, not improvement).
- **Auto-parallelization of independent statements** (`design/concurrency.md`): codegen-only; no syntax for "schedule these in parallel."

### When NOT to auto-promote

- **When the analysis required would slow the compiler unacceptably** for non-release builds. Defer expensive analyses to `--release` or PGO-mode.
- **When the user has explicit syntax pinning the slower form**. If a user wrote `let count = 5` and never modifies `count`, the lint suggests `const` — but if they wrote `let` deliberately knowing they MIGHT modify it later (e.g., they're sketching), respect the choice. Don't override explicit user syntax with an auto-promotion that would force a future explicit rewrite.
- **When the "stricter" form has a runtime cost the user might not want.** Hypothetical example: if a future Yinz allocator tier "shared cache" was faster for repeated lookups but slower for write-heavy use, the compiler shouldn't auto-promote without proving the access pattern matches. The general rule: auto-promotion only when the stricter form is unambiguously better OR when the user's usage proves the better-pattern fits.

---

## Project-Creation Checklist

When designing ANY new language feature, stdlib type, or compiler optimization, add to your design doc an explicit "Auto-Promotion Analysis" subsection answering:

- [ ] **Is there a stricter or faster form of this feature?** (e.g., narrow type, faster algorithm, more constrained ownership, more efficient layout)
- [ ] **Can the compiler prove the stricter form fits in some cases?** If no, this rule doesn't apply — note that and move on.
- [ ] **For cases where it can prove the fit:**
  - **Codegen-promote?** (Yes if there's a measurable runtime/memory benefit)
  - **Muted hint?** (Yes if the explicit form is typeable Yinz syntax — must complete to real source per `.claude/rules/inference.md`)
  - **Tier 3 lint suggestion?** (Yes if writing the explicit form would benefit code review or future-proof the code)
- [ ] **What's the lint rule name?** (Convention: `prefer-X-when-Y`, e.g., `prefer-fixed-when-immutable`)
- [ ] **What does the muted hint render inline?** (Must be informative-at-a-glance per `.claude/rules/inference.md` "Muted hint shows" column)
- [ ] **What does the hover tooltip say?** (Must follow WHAT/WHAT-INSTEAD/WHY per Golden Rule 11)
- [ ] **What's the user-facing teaching error if the analysis fails the proof?** (e.g., user wrote `fixed<T>` then `.add()` — compile error must explain why and suggest `array<T>`)

If the new feature has no auto-promotion candidates (rare — most stdlib types and language features have at least one), state that explicitly so reviewers know it was considered, not forgotten.

---

## How This Rule Interacts With Other Rules

- **Golden Rule 4 (compiler does the hard work)**: auto-promotion is the operational mechanism. Rule 4 says the compiler does smart things; this rule says HOW for the "stricter form fits" case.
- **Golden Rule 10 (efficiency first, dynamic after)**: auto-promotion is what makes efficiency the actual default — without this pattern, the "fast form" is only used when the user picks it explicitly.
- **Golden Rule 11 (compiler is teacher)**: auto-promotion's teaching surfaces (muted hint, lint suggestion) are governed by Rule 11. The hover tooltip text MUST follow WHAT/WHAT-INSTEAD/WHY.
- **`.claude/rules/inference.md`**: muted-hint protocol — when and how the auto-promotion shows up as informational text.
- **`design/linting.md`**: Tier 3 lint suggestions — when and how the auto-promotion shows up as a teaching squiggle.
- **`.claude/rules/plan-invariants.md`**: milestone plans must evaluate auto-promotion opportunities in the `### Performance` invariant subsection.

---

## Banned Anti-Patterns

The following violate this rule and should be flagged in code review or design review:

1. **Silent codegen promotion with no teaching surface.** If the compiler auto-promotes `let` to `const` in codegen but provides no muted hint or lint, the user has no way to learn the rule. Auto-promotion without teaching is magic; magic is anti-Yinz.
2. **Lint suggestion without auto-promotion.** If the lint says "use `fixed<T>` for perf" but the compiler doesn't auto-emit `fixed<T>` codegen for the proven-fits case, then code that ignores the lint stays slow. Punishes laziness instead of rewarding it. Anti-Yinz.
3. **Manual opt-in for the perf-correct form when the compiler could prove it.** If the user has to type `fast<T>` explicitly to get the fast version (when the compiler could have proven it fits), that's the inverse of this rule.
4. **Inconsistent hint-vs-lint application across similar features.** If `array→fixed` gets both surfaces but `let→const` gets only one (or vice versa), that's drift. The criterion (typeability) should be applied consistently.

---

## Cross-References

- `design/golden-rules.md` Rules 4, 10, 11 (the rules this pattern operationalizes)
- `design/collections.md` "Auto-promotion: `array<T>` → `fixed<T>`" section (canonical rationale + first locked example)
- `design/linting.md` (Tier 3 lint suggestion mechanism + `prefer-fixed-when-immutable` and `mutable-when-const-suffices`)
- `.claude/rules/inference.md` (muted-hint protocol + "Two Surfaces for the Same Decision" section)
- `.claude/rules/plan-invariants.md` (milestone plans must check auto-promotion)
- `design/teaching-mission.md` (the broader teaching goal this pattern serves)
- `design/future/auto-soa.md` (codegen-only example — no typeable form)
- `design/future/concurrency.md` (auto-Arc, auto-`wait`, auto-parallelization examples)
