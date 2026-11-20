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
let msg = `Hello ${name}, you have ${health} HP`
let debug = `Player at (${pos.x}, ${pos.y}) with ${player.health} health`
let label = `Score: ${player.score * 2}`
let status = `${active.count()} players active`
```

Any expression works inside `${}`. Each `${...}` works out its expression once, turns the result into text, and puts it in the string. If you write the same `${x}` twice, `x` is worked out twice.

---

## Multi-line strings

Newlines just work inside backticks:

```ynz
let message = `
  Dear ${name},
  Your score is ${score}.
`
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

## Concatenation — use + to join strings

```ynz
let full = `Hello ` + name
let path = folder + `/` + filename
```

---

## string is a built-in type

No imports needed. `string` is available everywhere.

---

## Safe indexing — brackets are a shortcut for .get()

Use brackets to read one character at a position, just as you do with collections. The result is `maybe<string>`, so a position past the end never crashes your program:

```ynz
let name = `Alice`
let first = name[0]             // a shortcut for name.get(0) → maybe<string> (`A`)
let huh = name[100]             // maybe<string> → none (past the end, safe)

if (first.exists()) {
  print(first.value)            // A
}
```

A single character in Yinz is just a string of length 1. There is no separate `char` type.

*Character* here means a *code point*: one entry in the Unicode standard, which is the list of every letter, symbol and emoji a computer can show. Most characters you type are one code point. See "Three ways to index" below for the exceptions.

---

## Strings cannot be changed in place

You cannot replace a character by its position:

```ynz
let name = `Alice`
name[0] = `B`
// COMPILE ERROR: `string` does not support index assignment.
//   Index assignment works on `array<T>`, `fixed<T>`, and `map<K, V>`.
// To change the value, build a new string:
//   let newName = `B` + name.substring(1)
// Or give the variable a new value:
//   name = `Blice`
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

By default, `.get()` and `[n]` count by **code point**. Two more functions count differently:

| Function | Counts by | Returns | Use when |
|----------|-----------|---------|----------|
| `.get(n)` or `s[n]` | Code point | `maybe<string>` (1 code point) | Most text work |
| `.byteAt(n)` | Byte (the raw numbers a file stores, 0–255) | `maybe<int>` | Reading file formats and network data |
| `.graphemeAt(n)` | Grapheme (one character as a person sees it) | `maybe<string>` (1 grapheme) | Showing text, moving a cursor, emoji |

A *grapheme* is what a person sees as one character. Sometimes it is built from several code points, such as a letter plus an accent mark, or an emoji family made of several people joined together.

Matching length functions:

```ynz
`café`.count()              // 4 (code points — what .get() counts)
`café`.byteCount()          // 5 (the é takes 2 bytes)
`café`.graphemeCount()      // 4
```

For plain English letters and digits, all three give the same number. They differ for other text:

```ynz
let word = `café`               // an `e` followed by a separate accent mark (two code points, one grapheme)

word.byteCount()                // 6  (c, a, f, e, and 2 bytes for the accent mark)
word.count()                    // 5  (code points — the accent mark counts as its own)
word.graphemeCount()            // 4  (the e and its accent count as one)

let foundFirst = word.get(0)
let first = foundFirst.or(``)                // c

let foundFourth = word.get(3)
let fourth = foundFourth.or(``)              // e     (just the e, without its accent)

let foundGrapheme = word.graphemeAt(3)
let grapheme = foundGrapheme.or(``)          // é     (the e with its accent — what a person sees)
```

And for an emoji made of several people:

```ynz
let family = `👨‍👩‍👧`

family.byteCount()           // 18  (emoji take many bytes)
family.count()               // 5   (5 code points)
family.graphemeCount()       // 1   (one character as a person sees it)

let foundPerson = family.get(0)
let person = foundPerson.or(``)              // 👨         (just the man, the first code point)

let foundFamily = family.graphemeAt(0)
let wholeFamily = foundFamily.or(``)         // 👨‍👩‍👧     (the whole family — one grapheme)
```

---

## String methods

### Searching

```ynz
`Patrick`.contains(`rick`)       // true
`Patrick`.indexOf(`rick`)        // maybe<int> → 3
`Patrick`.startsWith(`Pat`)      // true
`Patrick`.endsWith(`ick`)        // true
```

### Transforming

```ynz
`hello`.toUpperCase()            // HELLO
`HELLO`.toLowerCase()            // hello
`  hi  `.trim()                  // hi
`a,b,c`.split(`,`)              // array<string> [`a`, `b`, `c`]
`hello`.substring(1, 3)          // el
`hello`.replace(`l`, `r`)        // herro
```

### Lengths

```ynz
`café`.count()                   // 4 (code points)
`café`.byteCount()               // 5 (bytes)
`café`.graphemeCount()           // 4 (characters as a person sees them)
```

---

## Text that looks the same is equal

Two strings that look the same to a person compare as equal, even when they are stored differently:

```ynz
let accentSeparate = `café`    // an `e` followed by a separate accent mark (2 code points for the é)
let accentJoined = `café`      // one single é code point

accentSeparate == accentJoined   // true — Yinz puts both in the same standard form before comparing
```

You never need to think about this for ordinary comparisons.

---

## Other quote styles are errors

**Single quotes (`'...'`) are not string delimiters in Yinz:**

```
print('hello world')
// COMPILE ERROR: Single-quoted strings don't exist in Yinz.
// Use backtick strings instead: `hello world`
```

**Double quotes (`"..."`) are also not valid:**

```
print("hello world")
// COMPILE ERROR: Double-quoted strings don't exist in Yinz.
// Use backtick strings instead: `hello world`
```

Yinz has exactly one string form. You never have to choose between quote styles.
