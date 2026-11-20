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
| The type of a variable | `let score = 42` | `: Int` after `score` |
| Who holds a value at a call | `printName(player)` | `share`, `lend` or `give` after `player`. Click jumps to `printName`'s signature, where the word is written. |
| A store that hands a value over | `list.add(a)` | `handed to` and the place |
| A copy of a small value | `let n: Int = 5` | `.copy()` where the copy happens |
| A pause on a slow call | `fetchFeed(user)` | `wait` before the call |
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
  let score = 42                // muted ": Int" after score
                                // click → becomes `let score: Int = 42`
}
```

The ownership hint at a call is informational. `share`, `lend` and `give` are written in function signatures only, so clicking the hint jumps to the function's signature:

```ynz
shape Player {
  name: String
  health: Int
}

function printName(share player: Player) -> Nothing {
  print(player.name)
}

function entrypoint() -> Nothing {
  const player: Player = { name: `Patrick`, health: 100 }
  printName(player)             // muted "share" after player
                                // click → the IDE jumps to printName's signature
}
```

---

## Hover tooltip format

Every muted hint shows a three-part tooltip on hover, in the same WHAT / WHAT INSTEAD / WHY format as compiler errors.

Hovering the muted `share` in the example above shows:

> **WHAT**: Shows who holds a value at this line. After a call it is the ownership word the compiler picked: `share` (the function only reads it), `lend` (the function may change it, you keep it) or `give` (the function takes it over). After a store it reads `handed to` and the place: storing a value hands it over, so that place is now its one owner and the name before the hint cannot be read after this line.
>
> **WHAT INSTEAD**: Nothing to type for a call; write `share`, `lend` or `give` in the function's signature to make the choice visible. For a store, if you read the value again, store a copy instead (`list.add(a.copy())`), or move the line that reads it above the store.
>
> **WHY**: Every value has one owner, and the owner frees the value when it is done with it. If two names could reach one value, one of them would free it under the other, so a store hands the value over and a call tells you whether you still hold it. A copy is a separate value, so both names can keep theirs.

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
