---
name: "REF-teaching-mission"
description: "Yinz exists not just to be easy to USE, but to actively teach developers to write better code. This is a positioning decision that differentiates Yinz from every other language in its class."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Teaching Mission — A Core Goal of the Language

Yinz exists not just to be easy to USE, but to actively teach developers to write better code. This is a positioning decision that sets Yinz apart from other languages like it.

---

## The Mission

The compiler is not just a checker. It acts like a senior developer coaching a junior developer every time you use it.

**Every compiler message answers three questions:**

1. **WHAT happened** — the problem, in plain English
2. **WHAT to do instead** — the corrected code, ready to copy
3. **WHY** — the real reason: speed, clarity, or the usual Yinz style

A message that doesn't answer all three is incomplete. A suggestion that just says "consider X" without explaining WHY is not a Yinz suggestion.

---

## Why this matters

A language can teach you its syntax. Yinz also wants to teach you to be a BETTER PROGRAMMER while you use it.

Yinz combines four things: **easy syntax**, **real systems skills** (ownership, no garbage collector, native compilation), **compile-time safety** (the compiler catches errors before you run the program), and an **editor that explains everything**. The teaching mission ties them together.

**Long-term hope:** Yinz becomes a language for a first computer science course, because it is both easy to start with AND strong enough for real products. Students learn real systems ideas (ownership, type safety, exact arithmetic) and finish with a language they can ship code in.

---

## What teaching looks like in practice

### Pattern 1 — Choosing how to store data

```ynz
let scores: map<string, number> = { alice: 90, bob: 85, charlie: 78 }
// IDE HINT: Every key here is written out in your code, so a shape is
//           much faster than a map:
//   shape Scores { alice: number, bob: number, charlie: number }
//   let scores: Scores = { alice: 90, bob: 85, charlie: 78 }
//
// Why: Reading a shape field is one step: the computer jumps straight to it.
//      Reading a map key means searching for it, which takes roughly 10 to 50
//      steps. For keys that never change, a shape is about 10x faster AND you
//      get dot access with autocomplete.
```

### Pattern 2 — Speed suggestions

```ynz
let data: array<Player> = []
for (i in range(0, 10000)) {
  data.add({ name: `Player ${i}`, health: 100 })
}
// IDE HINT: Growing an array to a size you already know makes it move to a
//           bigger block of memory about 13 times as it fills up. Reserve the
//           room first to skip that:
//   let data = array.withCapacity<Player>(10000)
//
// Why: Each move copies the whole array. For 10,000 items that is about 130KB
//      of copying you don't need. Reserving the room once is a single step.
//      Filling a known number of items becomes 3-5x faster.
```

### Pattern 3 — The usual Yinz style

```ynz
let nums = [1, 2, 3, 4, 5]
let sum = 0
for (n in nums) {
  sum = sum + n
}
// IDE HINT: This adds up an array one item at a time. In Yinz you can write:
//   let sum = nums.reduce(0, (acc, n) => acc + n)
//
// Why: .reduce() says "combine everything into one value", so the goal is
//      clearer. The compiler produces the same machine code for both, so
//      there is no speed cost. The named version is easier to read.
```

### Pattern 4 — Choosing a number type

```ynz
let price: float = 19.99
// IDE HINT: Using float for financial values may cause rounding errors.
//   Consider number for exact arithmetic:
//     let price: number = 19.99
//
// Why: float stores numbers in binary, which can't hold most decimal
//      fractions exactly (0.1 + 0.2 = 0.30000000000000004). Money math that
//      repeats many times adds up those tiny errors and gives wrong totals.
//      number does its math in base 10, so it is exact for any value you
//      would write by hand. The speed difference is small; the accuracy
//      difference is large.
```

### Pattern 5 — Memory and ownership

```ynz
function takeDamage(share player: Player, amount: number) -> nothing {
  player.health = player.health - amount
  // COMPILE ERROR: Cannot change player — this function only has read-only
  //   access (share).
  //   Use lend when a function needs to change a value without taking
  //   ownership:
  //     function takeDamage(lend player: Player, amount: number) -> nothing
  //
  // Why: share means "let me look at this, you keep ownership." It is
  //      read-only. lend means "let me change this for a moment, then I give
  //      it back." A function that changes a player's state should use lend.
  //      This is how Yinz prevents two parts of a program from changing the
  //      same data at once, without a garbage collector: every function says
  //      up front what it will do with each value.
}
```

---

## The IDE also teaches (muted hints)

The IDE is not only a place to type code. It teaches just as much as compiler error messages do. Whenever the compiler works something out for you, the IDE shows it as faint, muted text that you can read, click to write out yourself, and hover to get a three-part explanation.

So a developer learns even when their code is **completely correct**:

```ynz
let x = 42                    // IDE shows muted ": int" → teaches how the compiler picks types
foo(player)                   // IDE shows muted "share" → teaches ownership at call sites
db.fetch(`users`)             // IDE shows muted "wait" → teaches that the call pauses for the network
arena scratch {
  let temp: array<int> = []     // IDE shows muted ".in(scratch)" → teaches that memory comes from the arena
}
```

Each muted hint shows a tooltip when you hover. The tooltip uses the same WHAT / WHAT-INSTEAD / WHY format as compiler messages, with the same wording, so the explanation is the same whether you meet it as a hint or as a compile error.

**Why both kinds of teaching are needed**:

- **Teaching after a mistake (compiler errors)** starts when you write something wrong. It is useful, but it comes after the mistake.
- **Teaching before a mistake (IDE muted hints)** shows up on every keystroke of valid code. The developer learns how Yinz works by reading their own code with the compiler's choices written on it.

Both are required. Error messages fix misunderstandings; muted hints prevent them.

**How hints look**:

- Neutral muted (gray): harmless choices — types, how long a value lives, where memory comes from
- Cautionary muted (red-tinted): choices that change data or affect ownership — `lend`, `give`, and automatic sharing of a value across threads
- Compile errors use the normal error look (red squiggly line plus a message panel), so they never look like muted hints

The full list of hints is in [REF-ide-hints.md](REF-ide-hints.md).

**One wording everywhere**: each concept has one standard explanation. A compile error that says "`player` is `const` — `foo` needs to mutate it but `const` blocks mutation" uses the same wording as the IDE tooltip that explains why `share` was chosen for a `const` value. The explanation is written once and every surface reuses it, so the docs, the error and the tooltip never disagree.

---

## The What / What-Instead / Why Format — Required for Every Message

Every error, warning and suggestion from the Yinz compiler MUST follow this structure.

```
[severity]: [WHAT — a short statement of the problem]

  [WHAT TO DO INSTEAD — corrected code, ready to copy]

  [WHY — the real reason: speed, correctness, or the usual Yinz style]
```

**A message without a WHY is not a Yinz message.** It is a code review with no explanation. The WHY is the whole point of the teaching mission.

This applies to:
- Compile errors (Tier 1)
- Compile warnings (Tier 2)
- IDE suggestions (Tier 3)
- Lint messages (all tiers)
- Test failure output
- Even `panic()` messages from production code (a panic should say what state caused it)

---

## A Question for Every New Feature

Every new feature, rule, or message must answer four questions:

1. Does it follow the golden rules?
2. Does it repeat an idea Yinz already has?
3. Is the default the fastest option?
4. **Does it teach the user something, or does it just hide complexity?**

The fourth question is the teaching check.

**Things that pass it:** error messages, lint suggestions, IDE hints, even example text in these docs. They inform the user.

**Things that might fail it:** automatic type conversions, global values that appear from nowhere, behavior that happens with no explanation, and anything that hides what the machine is really doing.

If a feature makes it easier to write code without learning why it works, it fails the teaching mission. Yinz should make it easy to write GOOD code AND to understand WHY it is good.

---

## What the compiler should teach

The compiler's teaching covers at least these areas:

| Area | Examples |
|------|----------|
| **Choosing a data layout** | shape or map, array or fixed array, when to use which |
| **Speed habits** | reserving room up front, avoiding repeated memory requests, why merged steps are fast |
| **Memory and ownership** | when to use share, lend, give and copy, and why ownership matters |
| **Number types** | when to use number, float or int, and why exact decimals are the default |
| **The usual Yinz style** | one step per line instead of chaining, named intermediate values, descriptive function names |
| **Design habits** | when a shape is better than a map, when a union is better than yes/no flags |
| **Conventions** | naming (Rule 10), how to handle errors, how to organize modules |
| **Habits to avoid** | what NOT to do and why (arrays with gaps, deeply nested code, unexplained numbers) |

Each area has rules in the linter and hints in the IDE. Hints for one module, such as "use `path.join()` instead of joining strings with `+`", arrive with that module.

---

## Spotting habits to avoid

Beyond pointing at single lines, the compiler should recognize whole patterns of code that cause trouble and explain why:

- **Values changed all over the place** — "you changed this shape in 4 different functions; consider returning a new value instead, which is easier to follow"
- **Functions that do too much** — "this function does 5 separate things; consider splitting it"
- **Unexplained numbers** — "what does 3600 mean here? Consider `60 * 60` or `const secondsPerHour = 3600`"
- **Deeply nested conditions** — "5 levels of nesting; consider returning early"
- **Yes/no flags as parameters** — "a boolean flag suggests two different jobs; consider two functions or an `options` type"

Each pattern is a chance to teach. The IDE and compiler don't FORBID these patterns, because sometimes they are right. They suggest the alternative and explain the reasoning.

---

## How these docs are written

The teaching mission applies to these docs too:

- They are written for a developer who just finished high school, knows JavaScript, and has never done systems programming
- Examples use realistic code (Player, User, score, health), not foo/bar/x/y
- Compiler error messages are SHOWN, not just described
- Each design choice comes with the reason a curious reader would ask about

The docs are a teacher too, not just a reference.

---

## Success: schools choose Yinz

Yinz's success isn't measured by GitHub stars or hiring posts. The measure is:

> **Schools choose Yinz for a first programming course, because students learn real systems ideas in their first semester AND finish with a language they can ship real code in.**

That is the bar. Every error message, lint rule and doc example should move toward it.

If a feature would make it HARDER for a beginner to learn programming basics, it doesn't fit Yinz. If a feature actively TEACHES a basic idea, it is exactly what Yinz should have.

---

## How this connects to the golden rules

- **Rule 3 (the compiler is a teacher)** — a direct match. The teaching mission goes beyond explaining errors after the fact: it also teaches through suggestions while your code is valid.
- **Rule 4 (self-documenting syntax)** — about how code reads. The teaching mission is the next layer: not only readable, but explaining itself as you read.
- **Rule 5 (human-readable over jargon)** — about the words in the language. The teaching mission holds compiler messages to the same standard.
- **Rule 8 (compiler does the hard work)** — about speed. The teaching mission adds: tell the user WHAT you did and WHY.

The teaching mission is what ties Rules 3, 4 and 5 into one identity.
