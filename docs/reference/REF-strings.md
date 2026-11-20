---
name: "REF-strings"
description: "User-facing reference for Yinz string literals: backtick quoting, ${} interpolation, multi-line text, and related string behavior."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Strings

Text in Yinz.

---

## String literals — backtick quotes

Yinz has one string form: backtick-quoted strings. Always. No double-quote form, no single-quote form.

```ynz
let name = `Patrick`
let greeting = `Hello, world`
```

Backtick-quoted strings always support interpolation and multi-line text — you never have to switch to a different string form.

---

## Interpolation — put any expression inside ${}

Put any value inside a string using `${}`:

```ynz
shape Position {
  x: Int
  y: Int
}

shape Player {
  health: Int
  score: Int
}

function entrypoint() -> Nothing {
  let name = `Patrick`
  let health = 90
  let pos: Position = { x: 3, y: 4 }
  let player: Player = { health: 80, score: 21 }
  let activeCount = 7

  let msg = `Hello ${name}, you have ${health} HP`
  let debug = `Player at (${pos.x}, ${pos.y}) with ${player.health} health`
  let label = `Score: ${player.score * 2}`
  let status = `${activeCount} players active`
  print(msg)
  print(debug)
  print(label)
  print(status)
}
```

Any expression works inside `${}`. Each `${...}` works out its expression once, turns the result into text, and puts it in the string. If you write the same `${x}` twice, `x` is worked out twice.

---

## Multi-line strings

Newlines just work inside backticks:

```ynz
let name = `Patrick`
let score = 12
let message = `
  Dear ${name},
  Your score is ${score}.
`
print(message)
```

No special triple-quote form needed. One quote form covers everything.

---

## Escape sequences

Inside backtick strings:

| Sequence | Meaning |
|---|---|
| `` \` `` | Literal backtick |
| `\${` | Literal `${` (useful when writing docs about Yinz syntax) |
| `\n` | Newline |
| `\t` | Tab |
| `\\` | Backslash |
| `\r` | Carriage return |
| `\0` | The character with code 0 (it shows nothing; it is used in some file formats) |

---

## Joining strings — use interpolation

Yinz has no `+` for strings. Put the pieces inside one backtick string with `${}`:

```ynz
let name = `Alice`
let folder = `docs`
let filename = `notes.txt`

let full = `Hello ${name}`
let path = `${folder}/${filename}`
print(full)
print(path)
```

Writing `+` between two strings is a compile error:

```ynz
let first = `Hello `
let second = `Alice`
let full = first + second
// COMPILE ERROR: `+` cannot be used with `String` and `String`.
// Check that both sides have the same type.
// Yinz does not convert between types automatically. Both sides of an expression must have the same type.
```

---

## String is a built-in type

No imports needed. `String` is available everywhere.

---

## Safe indexing — .get() returns a Maybe

Use `.get(position)` to read one character at a position. The result is `Maybe<String>`, so a position past the end never crashes your program:

```ynz
let name = `Alice`
let first = name.get(0)         // Maybe<String> holding `A`
let huh = name.get(100)         // Maybe<String> holding nothing (past the end, safe)

if (first.exists()) {
  print(first.value)            // A
}
print(huh.exists())             // false
```

A single character in Yinz is just a string of length 1. There is no separate `char` type.

*Character* here means a *code point*: one entry in the Unicode standard, which is the list of every letter, symbol and emoji a computer can show. Most characters you type are one code point. See "Three ways to index" below for the exceptions.

---

## Strings cannot be changed in place

You cannot replace a character by its position:

```ynz
let name = `Alice`
name[0] = `B`
// COMPILE ERROR: `String` does not support index assignment.
// Index assignment works on `Array<T>`, `Fixed<T>`, and `Map<K, V>`.
// Only built-in collection types support `collection[index] = value` syntax.
```

To change the text, give the variable a new value or build a new string:

```ynz
let name = `Alice`
let newName = `B${name.substring(1, 5)}`
print(newName)              // Blice
name = `Blice`
print(name)
```

---

## Looping over a string

`for (letter in word)` goes through the string one code point at a time. Each `letter` is a string of length 1:

```ynz
for (letter in `café`) {
  print(letter)    // c, a, f, é  (4 steps — é is one code point)
}
```

To loop over what a person sees as one character (including emoji), use the grapheme functions in the next section.

---

## Three ways to index — code point, byte, grapheme

By default, `.get(n)` counts by **code point**. Two more functions count differently:

| Function | Counts by | Returns | Use when |
|----------|-----------|---------|----------|
| `.get(n)` | Code point | `Maybe<String>` (1 code point) | Most text work |
| `.byteAt(n)` | Byte (the raw numbers a file stores, 0–255) | `Maybe<Int>` | Reading file formats and network data |
| `.graphemeAt(n)` | Grapheme (one character as a person sees it) | `Maybe<String>` (1 grapheme) | Showing text, moving a cursor, emoji |

A *grapheme* is what a person sees as one character. Sometimes it is built from several code points, such as a letter plus an accent mark, or an emoji family made of several people joined together.

Matching length functions:

```ynz
print(`café`.count())              // 4 (code points — what .get() counts)
print(`café`.byteCount())          // 5 (the é takes 2 bytes)
print(`café`.graphemeCount())      // 4
```

For plain English letters and digits, all three give the same number. They differ for other text:

```ynz
let word = `café`            // an `e` followed by a separate accent mark (two code points, one grapheme)

print(word.byteCount())           // 6  (c, a, f, e, and 2 bytes for the accent mark)
print(word.count())               // 5  (code points — the accent mark counts as its own)
print(word.graphemeCount())       // 4  (the e and its accent count as one)

let foundFirst = word.get(0)
let first = foundFirst.or(``)                // c

let foundFourth = word.get(3)
let fourth = foundFourth.or(``)              // e     (just the e, without its accent)

let foundGrapheme = word.graphemeAt(3)
let grapheme = foundGrapheme.or(``)          // é     (the e with its accent — what a person sees)
print(first)
print(fourth)
print(grapheme)
```

And for an emoji made of several people:

```ynz
let family = `👨‍👩‍👧`

print(family.byteCount())           // 18  (emoji take many bytes)
print(family.count())               // 5   (5 code points)
print(family.graphemeCount())       // 1   (one character as a person sees it)

let foundPerson = family.get(0)
let person = foundPerson.or(``)              // 👨         (just the man, the first code point)

let foundFamily = family.graphemeAt(0)
let wholeFamily = foundFamily.or(``)         // 👨‍👩‍👧     (the whole family — one grapheme)
print(person)
print(wholeFamily)
```

---

## String methods

### Searching

```ynz
let text = `Patrick`
print(text.contains(`rick`))       // true
let position = text.indexOf(`rick`)
print(position.or(0))              // 3 (indexOf returns Maybe<Int>)
print(text.startsWith(`Pat`))      // true
print(text.endsWith(`ick`))        // true
```

### Transforming

```ynz
print(`hello`.toUpperCase())       // HELLO
print(`HELLO`.toLowerCase())       // hello
print(`  hi  `.trim())            // hi
let pieces = `a,b,c`.split(`,`)   // Array<String> [`a`, `b`, `c`]
print(pieces.count())              // 3
print(`hello`.substring(1, 3))     // el
print(`hello`.replace(`l`, `r`))   // herro
```

### Lengths

```ynz
print(`café`.count())              // 4 (code points)
print(`café`.byteCount())          // 5 (bytes)
print(`café`.graphemeCount())      // 4 (characters as a person sees them)
```

---

## Text that looks the same is equal

Two strings that look the same to a person compare as equal, even when they are stored differently:

```ynz
let accentSeparate = `café`   // an `e` followed by a separate accent mark (2 code points for the é)
let accentJoined = `café`          // one single é code point

print(accentSeparate == accentJoined)   // true — Yinz puts both in the same standard form before comparing
```

You never need to think about this for ordinary comparisons.

---

## Other quote styles are errors

**Single quotes (`'...'`) are not string delimiters in Yinz:**

```
print('hello world')
// COMPILE ERROR: Single-quoted strings don't exist in Yinz.
// Use backtick strings instead: `` `hello world` ``
// Yinz has one string form — backtick strings. They support interpolation and span multiple lines without extra syntax.
```

**Double quotes (`"..."`) are also not valid:**

```
print("hello world")
// COMPILE ERROR: Double-quoted strings don't exist in Yinz.
// Use backtick strings instead: `` `hello` `` or `` `Hello ${name}` ``
// Yinz has one string form — backtick-quoted strings. They always support interpolation and span multiple lines without extra syntax.
```

Yinz has exactly one string form. You never have to choose between quote styles.
