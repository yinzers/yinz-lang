---
name: "REF-ide-hints"
description: "The muted-text hints the IDE shows for what the compiler worked out for you, how to click one to write it out, and how to read the hover explanation."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# IDE Hints — Muted Text

When the compiler works something out for you, the IDE shows what it decided as **muted text**. You can read it, click it to write it into your code, and hover it for a three-part explanation.

---

## What gets hinted

| What the compiler worked out | Example source | Muted hint shows |
|---|---|---|
| The type of a variable | `let x = 42` | `: Int` after `x` |
| Who holds a value at a call | `foo(player)` | `share`, `lend` or `give` after `player`. Click jumps to `foo`'s signature, where the word is written. |
| A store that hands a value over | `list.add(a)` | `handed to` and the place |
| A copy of a small value | `let n: Int = 5` | `.copy()` where the copy happens |
| A pause on a slow call | `sleep(100)` | `wait` before the call |
| Where a `background` task runs | `background process(order)` | which pool the task runs on |
| Statements that run at the same time | `let hits = crunchStat(2)` | `// runs at the same time as line 12` |
| A channel's default capacity | `Channel<Int>()` | `64` inside the parentheses |
| A value shared across tasks by counting | `background render(scene)` | `// shared by reference count with 2 tasks — read-only` |
| An `Array` that never grows | `let scores: Array<Int> = [90, 72]` | a dotted underline on `Array` (see [Replacement hints](#replacement-hints-the-underlined-token)) |
| A `let` that never changes | `let attempts = 3` | a dotted underline on `let` (same section) |

---

## Visual styling

Hints are shown in muted text, lighter than your code. Compile errors use a separate style (a red squiggly underline and the error panel) and never appear as muted hints.

---

## Hint text and click action

Most hints complete to Yinz you could have typed. Click one to write it into your source.

```ynz
function entrypoint() -> Nothing {
  let x = 42                    // muted ": Int" after x
                                // click → becomes `let x: Int = 42`
}
```

The ownership hint at a call is informational. `share`, `lend` and `give` are written in function signatures only, so clicking the hint jumps to the function's signature:

```ynz
shape Player {
  name: String
  health: Int
}

function foo(share p: Player) -> Nothing {
  print(p.name)
}

function entrypoint() -> Nothing {
  const player: Player = { name: `Patrick`, health: 100 }
  foo(player)                   // muted "share" after player
                                // click → the IDE jumps to foo's signature
}
```

---

## Hover tooltip format

Every muted hint shows a three-part tooltip on hover, in the same WHAT / WHAT INSTEAD / WHY format as compiler errors.

Hovering the muted `share` in the example above shows:

> **WHAT**: The compiler chose `share` at this call site. `foo`'s signature declares its parameter as `share`, and `player` is `const` (which can only be shared, never lent or given). The function gets read-only access; you keep ownership.
>
> **WHAT INSTEAD**: To see the contract, view `foo`'s signature — `function foo(share p: Player)`. There is no syntax for ownership words at call sites; the signature is where you write them.
>
> **WHY**: `const` bindings can only grant read-only access. If you need a function that changes `player`, declare `player` with `let` and change `foo`'s signature to `lend p: Player`.

---

## Replacement hints: the underlined token

When the compiler already uses a stricter form than the one you wrote, the IDE adds no text. It puts a cyan dotted underline on the token you wrote (red stays for errors and yellow for lint):

```ynz
function entrypoint() -> Nothing {
  let highScores: Array<Int> = [90, 72, 85]
}
```

Both the `let` and the `Array` are underlined, because nothing reassigns `highScores` and nothing grows it.

The underline colour is the theme colour `yinz.lessonUnderline` (`#4FD1E8` in dark themes, `#0E7490` in light themes). To change it, set it in `workbench.colorCustomizations`:

```json
"workbench.colorCustomizations": { "yinz.lessonUnderline": "#ff9f1c" }
```

- **Hover** an underlined token: the three-part explanation (WHAT, WHAT INSTEAD, WHY) for that choice.
- **Click** the lightbulb on it: the *Write 'const' instead of 'let'* / *Write 'Fixed' instead of 'Array'* action rewrites exactly that token. After both:

```ynz
function entrypoint() -> Nothing {
  const highScores: Fixed<Int> = [90, 72, 85]
}
```

The rewrite is optional: the program already behaves this way. A binding that does change has no underline and no rewrite: a `let attempts = 3` followed by `attempts = 4`, or an `Array` that gets `highScores.add(60)`.

---

## Related pages

- [REF-golden-rules.md](REF-golden-rules.md), Rule 3: every teaching surface answers WHAT, WHAT INSTEAD and WHY.
- [REF-teaching-mission.md](REF-teaching-mission.md): the IDE as a teaching surface.
- [REF-compiler-errors.md](REF-compiler-errors.md): the three-part format hints share with errors.
- [REF-concurrency.md](REF-concurrency.md): the `wait`, `background` and channel hints.
