# Teaching Mission — A First-Class Language Goal

Yinz exists not just to be easy to USE, but to actively teach developers to write better code. This is a positioning decision that differentiates Yinz from every other language in its class.

---

## The Mission

The compiler is not a checker. It is a senior developer mentoring a junior developer at every interaction.

**Every diagnostic answers three questions:**

1. **WHAT happened** — the issue, in plain English
2. **WHAT to do instead** — the corrected pattern, ready to copy
3. **WHY** — the underlying reason: performance, clarity, idiomatic Yinz, or convention

A diagnostic that fails to answer all three is incomplete. Suggestions that just say "consider X" without explaining WHY are not Yinz suggestions.

---

## Why this matters

Most languages teach you the LANGUAGE. Yinz wants to teach you to be a BETTER PROGRAMMER through the language.

Current CS-101 teaching languages all have problems:

- **Python** — easy to learn, but no types, no compilation, no real systems exposure. Students learn nothing about how computers actually work.
- **Java** — full of ceremony (`public static void main(String[] args)`) and outdated patterns (anonymous inner classes, factory factories).
- **C/C++** — teaches systems thinking but the learning curve is brutal and modern tooling is missing.
- **Rust** — teaches ownership but the borrow checker punishes beginners and most universities can't fit it in one semester.

Yinz's unique combination: **easy syntax (Python-tier learning curve) + real systems exposure (ownership, no GC, native compilation) + compile-time safety (catches errors before run) + IDE that explains everything.** No current language has all four. The teaching mission makes this combination explicit.

**Long-term aspiration:** Yinz becomes a CS-101 teaching language because it's both approachable AND production-grade. Students learn real systems concepts (ownership, type safety, exact arithmetic) without the brutal learning curve of Rust or C++, and graduate with a language they can actually ship code in.

---

## What teaching looks like in practice

### Pattern 1 — Data structure recommendations

```yinz
let scores: map[string, number] = { alice: 90, bob: 85, charlie: 78 }
// IDE HINT: All keys here are compile-time string literals — a type
//           is significantly faster:
//   type Scores { alice: number, bob: number, charlie: number }
//   let scores: Scores = { alice: 90, bob: 85, charlie: 78 }
//
// Why: Type field access compiles to a direct memory offset (~1 instruction).
//      Map key access requires a hash lookup (~10-50 instructions). For
//      static keys, types are ~10x faster AND you get dot-access syntax
//      with autocomplete.
```

### Pattern 2 — Performance suggestions

```yinz
let data: array[Player] = []
for (i in range(0, 10000)) {
  data.add({ name: `Player ${i}`, health: 100 })
}
// IDE HINT: Growing an array to a known final size triggers ~13 reallocations
//           as the array's capacity doubles. Pre-allocate to skip them:
//   let data = array.withCapacity[Player](10000)
//
// Why: Each reallocation copies the entire array. For 10,000 elements that's
//      ~130KB of needless copying. Pre-allocation is one allocation total.
//      3-5x speedup for filling-known-size patterns.
```

### Pattern 3 — Idiomatic Yinz

```yinz
let nums = [1, 2, 3, 4, 5]
let sum = 0
for (n in nums) {
  sum = sum + n
}
// IDE HINT: This sums an array element-by-element. Idiomatic Yinz:
//   let sum = nums.reduce(0, (acc, n) => acc + n)
//
// Why: .reduce() expresses "collapse to a single value" — clearer intent.
//      The compiler generates identical machine code for both, so no perf
//      penalty. The named version is easier to read and self-documenting.
```

### Pattern 4 — Type system teaching

```yinz
let price: float = 19.99
// IDE HINT: Using float for financial values may cause rounding errors.
//   Consider number for exact arithmetic:
//     let price: number = 19.99
//
// Why: float uses binary floating-point, which can't represent most decimal
//      fractions exactly (0.1 + 0.2 = 0.30000000000000004). Financial math
//      that accumulates errors over many operations produces wrong totals.
//      number uses decimal arithmetic — exact for any value you'd write
//      by hand. The performance difference is minor; the correctness
//      difference is significant.
```

### Pattern 5 — Memory and lifetime teaching

```yinz
function takeDamage(share player: Player, amount: number) -> nothing {
  player.health = player.health - amount
  // COMPILE ERROR: Cannot modify player — it was shared (read-only access).
  //   Use .lend for modifications that don't transfer ownership:
  //     function takeDamage(lend player: Player, amount: number) -> nothing
  //
  // Why: .share means "let me look at this, you keep ownership." It's
  //      read-only. .lend means "let me modify this temporarily, then I
  //      give it back." For methods that mutate a player's state, .lend
  //      is the right choice. This is how Yinz prevents data races without
  //      a garbage collector — every reference declares its intent up front.
```

---

## The What/What-Instead/Why Format — Required for All Diagnostics

Every error, warning, and suggestion in the Yinz compiler MUST follow this structure. The format is mandatory.

```
[severity tier]: [WHAT — concise statement of the issue]

  [WHAT TO DO INSTEAD — corrected code, ready to copy]

  [WHY — underlying reason: performance, correctness, idiomatic Yinz, or convention]
```

**A diagnostic that doesn't explain WHY is not a Yinz diagnostic.** It's a code review without context. The WHY is the entire point of the teaching mission.

This applies to:
- Compile errors (Tier 1)
- Compile warnings (Tier 2)
- IDE suggestions (Tier 3)
- Lint rule messages (all tiers)
- Test failure output (compiler-as-teacher applies to test results too)
- Even `panic()` messages from production code (panics should explain what state caused them)

---

## Decision-Time Criterion

Per `.claude/rules/language-design.md`, every new feature, rule, or message must answer four questions:

1. Does it follow the golden rules? (existing)
2. Does it duplicate an existing concept? (existing)
3. Is the default the most performant option? (existing)
4. **Does it teach the user something, or does it just hide complexity?** (NEW)

The fourth question is the teaching-mission gate.

**Features that PASS through it:** error messages, lint suggestions, IDE hints, even spec example text. They inform the user.

**Features that might FAIL it:** implicit conversions, magic globals, automatic behaviors that happen without diagnostic explanation, abstractions that hide what the machine is actually doing.

If a feature would make it easier to write code without learning anything about WHY it works, that's a teaching-mission failure. Yinz should make it easy to write GOOD code AND understand WHY it's good.

---

## Teaching Domains — What the compiler should teach

The compiler's teaching surface area covers (at minimum):

| Domain | Examples |
|--------|----------|
| **Data structure choice** | type vs map, array vs fixed, when to use what |
| **Performance patterns** | pre-allocation, avoiding repeated allocations, why fusion works |
| **Memory and ownership** | when to use share/lend/give/copy, why ownership matters |
| **Type system** | when to use number vs float vs int, why exact decimal is the default |
| **Idiomatic Yinz** | step-by-step over chaining, named intermediates, descriptive functions |
| **Design patterns** | when types beat maps, when discriminated unions beat boolean flags |
| **Convention** | naming (Rule 13), error handling style, module organization |
| **Anti-patterns** | what NOT to do and why (sparse arrays, deeply nested code, magic numbers) |

Each domain provides rules in the linting system and contextual hints in the IDE. Module-specific teaching (e.g., "use path.join() not string concatenation") ships with the relevant stdlib module.

---

## Anti-Pattern Detection

Beyond pointing out issues, the compiler should recognize SHAPES of problematic code and explain why they're problems:

- **Mutation chains** — "you mutated this struct in 4 different functions; consider returning new values instead, easier to reason about"
- **God functions** — "this function does 5 distinct things; consider splitting"
- **Magic numbers** — "what does 3600 mean here? Consider `60 * 60` or `const secondsPerHour = 3600`"
- **Deeply nested conditionals** — "5 levels of nesting; consider early returns or a state machine"
- **Boolean parameter flags** — "boolean flag suggests two different operations; consider two functions or an options type"

Each pattern is a teaching opportunity. The IDE/compiler doesn't FORBID these patterns — sometimes they're correct. It suggests the alternative and explains the reasoning.

---

## Spec Documentation Style

The teaching mission extends to the spec itself. Per `.claude/rules/spec-writing.md`:

- Write for a developer who just graduated high school, knows JavaScript, and has never done systems programming
- Every example shows realistic code (Player, User, score, health) — not foo/bar/x/y
- Compiler error messages are SHOWN in the spec, not just described
- Every section answers WHY this design decision was made (the "you'd ask if you weren't holding a book")

The spec is teaching too. Not just a reference.

---

## University Adoption — Long-Term Success Metric

Yinz's success isn't measured by GitHub stars or hiring posts. The success metric is:

> **CS-101 programs adopt Yinz as their teaching language because students learn real systems concepts in their first semester AND graduate with a production-grade language they can ship code in.**

That's the bar. Everything we ship — every error message, every lint rule, every spec example — should compound toward that goal.

If a feature would make it HARDER for a beginner to understand programming fundamentals, that feature isn't Yinz-flavored. If a feature actively TEACHES a fundamental, it's exactly what Yinz should be.

---

## How This Interacts with Other Rules

- **Rule 2 (self-documenting)** — surface readability. Teaching mission is the LAYER ABOVE: not just readable, but actively explaining as you read.
- **Rule 4 (compiler does the hard work)** — performance work. Teaching mission says: also tell the user WHAT you did and WHY.
- **Rule 11 (compiler is a teacher)** — directly aligned. The teaching mission expands Rule 11 from reactive (errors explain) to proactive (suggestions teach even when code is valid).
- **Rule 12 (human-readable over jargon)** — language surface. Teaching mission applies the same standard to diagnostic messages.

The teaching mission is what ties all the readability/teaching golden rules into a coherent identity. It's the "why" behind Rules 2, 11, and 12.
