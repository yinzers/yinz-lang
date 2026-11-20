---
name: "REF-teaching-mission"
description: "Yinz exists not just to be easy to USE, but to actively teach developers to write better code. The compiler and the IDE explain what they do and why."
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

Yinz exists not just to be easy to USE, but to actively teach developers to write better code. The compiler and the IDE explain what they do and why, so you learn as you write.

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

---

## What teaching looks like in practice

### Pattern 1 — Speed suggestions

```ynz
let weekdays: Array<String> = [`Mon`, `Tue`, `Wed`, `Thu`, `Fri`]
// IDE HINT: `Array` carries a dotted underline. Hover says:
//   WHAT:    The compiler stores this value as a fixed-size `Fixed<T>` instead of
//            a growable `Array<T>`, because nothing in this function ever grows it.
//   INSTEAD: Write `Fixed` instead of `Array` in the annotation, for example
//            `let weekdays: Fixed<String> = [...]`.
//   WHY:     A fixed-size list never has to grow, so it is cheaper to create and
//            use than a growable `Array`, and writing `Fixed` tells readers its
//            size will not change.
```

### Pattern 2 — The usual Yinz style

```ynz
let serverName = `prod-01`
// IDE HINT: `let` carries a dotted underline. Hover says:
//   WHAT:    The compiler treats this `let` as a `const`, because nothing in this
//            function ever reassigns it, changes it, or lends it out to be changed.
//   INSTEAD: Write `const` instead of `let`, for example `const serverName = `prod-01``.
//   WHY:     A value that never changes can be treated as fixed for the whole
//            function, and writing `const` tells readers at a glance that it will
//            not change.
```

### Pattern 3 — Memory and ownership

```ynz
function takeDamage(share player: Player, amount: Number) -> Nothing {
  player.health = player.health - amount
  // COMPILE ERROR: `player` is declared `share` — read-only — so its fields cannot be changed.
  //   Change the parameter to `lend player: Player` if this function needs to modify it.
  //
  // Why: A `share` parameter is a read-only borrow: the caller keeps ownership
  //      and trusts the value is unchanged after the call. When a function
  //      modifies a value, declare `lend` so the change is visible at every
  //      call site.
}
```

---

## The IDE also teaches (muted hints)

The IDE is not only a place to type code. It teaches just as much as compiler error messages do. Whenever the compiler works something out for you, the IDE shows it as faint, muted text that you can read, click to write out yourself, and hover to get a three-part explanation.

So a developer learns even when their code is **completely correct**:

```ynz
let maxHealth = 42            // IDE shows muted ": Int" → teaches how the compiler picks types
heal(player)                  // IDE shows muted "share" → teaches ownership at call sites
sleep(100)                    // IDE shows muted "wait" → teaches that the call pauses
```

Each muted hint shows a tooltip when you hover. The tooltip uses the same WHAT / WHAT-INSTEAD / WHY format as compiler messages, with the same wording, so the explanation is the same whether you meet it as a hint or as a compile error.

**Why both kinds of teaching are needed**:

- **Teaching after a mistake (compiler errors)** starts when you write something wrong. It is useful, but it comes after the mistake.
- **Teaching before a mistake (IDE muted hints)** shows up on every keystroke of valid code. The developer learns how Yinz works by reading their own code with the compiler's choices written on it.

Both are required. Error messages fix misunderstandings; muted hints prevent them.

**How hints look**:

- Muted text: choices the compiler made for you — types, `share`, `lend`, `give`, `wait`
- Compile errors use the normal error look (red squiggly line plus a message panel), so they never look like muted hints

The full list of hints is in [REF-ide-hints.md](REF-ide-hints.md).

**One wording everywhere**: each concept has one standard explanation. A compile error that says "`player` is `const` — `heal` needs to mutate it but `const` blocks mutation" uses the same wording as the IDE tooltip that explains why `share` was chosen for a `const` value. The explanation is written once and every surface reuses it, so the docs, the error and the tooltip never disagree.

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

---

## What the compiler teaches

| Area | Examples |
|------|----------|
| **Choosing a data layout** | `Array` or `Fixed`, when to use which |
| **Memory and ownership** | when the compiler chose share, lend, give and copy, and why |
| **The usual Yinz style** | `const` instead of `let` when a value never changes |

The full list of hints is in [REF-ide-hints.md](REF-ide-hints.md).

---

## How these docs are written

The teaching mission applies to these docs too:

- They are written for a developer who just finished high school, knows JavaScript, and has never done systems programming
- Examples use realistic code (Player, User, score, health), not foo/bar/x/y
- Compiler error messages are SHOWN, not just described

---

## How this connects to the golden rules

- **Rule 3 (the compiler is a teacher)** — a direct match. The teaching mission goes beyond explaining errors after the fact: it also teaches through suggestions while your code is valid.
- **Rule 4 (self-documenting syntax)** — about how code reads. The teaching mission is the next layer: not only readable, but explaining itself as you read.
- **Rule 5 (human-readable over jargon)** — about the words in the language. The teaching mission holds compiler messages to the same standard.
- **Rule 8 (compiler does the hard work)** — about speed. The teaching mission adds: tell the user WHAT you did and WHY.

The teaching mission is what ties Rules 3, 4 and 5 into one identity.
