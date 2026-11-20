---
name: Yinz Auto-Promotion Protocol
description: "Applies when designing a language feature, stdlib type or compiler optimization where the compiler could pick a stricter or faster form, or users might need to override its pick."
trigger: relevance
---

When the compiler can prove a stricter or faster form fits the user's actual usage, it picks that form automatically and teaches the user the choice. Why: beginners write whatever feels natural and get fast code, and the teaching surfaces turn them into performance-aware developers; silent magic would teach nothing.

## Surfaces

- **Codegen promotion:** emit the stricter form when it has a measurable benefit (speed, memory, safety). Why: the user gains without rewriting source.
- **IDE hint:** show it when the explicit form is typeable Yinz syntax. Why: click-to-make-explicit must produce real source.
- **Tier 3 lint suggestion:** suggest the explicit form when it helps code review. Why: a later edit then breaks compilation instead of silently regressing.

If no typeable explicit form exists (auto struct-of-arrays layout), use codegen plus lint only. If the difference exists only at compile time, use the lint only. Example: an `Array<T>` never grown becomes `Fixed<T>`, with all three surfaces.

## When not to auto-promote

- **The analysis is too slow for non-release builds:** defer it to release builds. Why: compile speed is a goal.
- **The user's explicit syntax pins the slower form:** respect it. Why: overriding it forces a surprise rewrite later.
- **The stricter form has a runtime cost the usage does not prove worth it:** do not promote. Why: promotion is for forms that are clearly better.

## Banned anti-patterns

1. Silent promotion with no hint or lint. Why: magic with no teaching.
2. A lint suggestion with no promotion. Why: code that ignores the lint stays slow, which punishes laziness.
3. A manual opt-in for a form the compiler could prove fits. Why: it is the inverse of this rule.
4. Applying the surfaces inconsistently across similar features. Why: apply the typeability criterion the same way everywhere, or it drifts.

## Design checklist

Every new feature, stdlib type or optimization answers these in its design doc:

- Is there a stricter or faster form the compiler can prove fits? If not, say so. Why: it shows the question was considered, not forgotten.
- Which surfaces apply, by the criteria above?
- The lint rule name, as `prefer-X-when-Y`.
- What the IDE hint shows inline, and the hover text (WHAT, WHAT INSTEAD, WHY).
- The teaching error when the proof fails (`.add()` on a `Fixed<T>` explains why and suggests `Array<T>`).
- The override analysis below.

## Overrides

When the compiler picks between forms, weigh both directions: force the auto-pick, and force the other pick. Why: users get the pick wrong both ways. Per direction, ask:

- **Is there a real use case?** If not, omit it. Why: an unneeded override is surface to maintain.
- **Does existing syntax already express it** (`let`/`const`, a `Fixed<T>`/`Array<T>` annotation)? Add nothing. Why: a second way to say it is clutter.
- **Does it make an observable difference?** If not, omit it (map pre-sizing). Why: it would be a knob that changes nothing.
- **Is the direction possible at all?** Forcing static dispatch on an unknown type is impossible, and forcing `Arc` with no thread crossing is pointless. Why: the compiler cannot manufacture knowledge, and unneeded cost helps no one.
- **Would it be a second API for the same capability?** That is banned: skip it or redesign it. Why: parallel APIs split the stdlib.
- **Does it weaken safety?** Then the name says so (a fast hash on attacker-controlled keys). Why: the risk must be visible at the call site.
- **Is it discoverable?** The IDE hint or lint offers it as the WHAT INSTEAD. Why: users cannot use an override they never see.
- **Does it add a runtime dependency?** It must still work in `--kernel`. Why: kernel mode has no runtime.
- **Is the pick ABI-locked?** That rules out an override only after v1.0; before it, breaking changes are allowed. Why: an override on a locked ABI would force every dependent binary to recompile.

Name paired overrides consistently, as with `sortFast()` and `sortStrict()` (sort needs both: composite items that tie freely want fast, and multi-step primitives want stable). Why: one naming pattern lets users guess the other half.

Record each deliberate omission, with its reason, in the feature's design doc. Why: a later reader can then tell a considered omission from a forgotten one.
