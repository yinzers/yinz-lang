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

Any expression works inside `${}`. Each `${expr}` evaluates the expression once, converts it to a string, and inserts it. Two identical `${x}` expressions evaluate `x` twice — no memoizing.

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
| `\0` | Null byte |

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

## Safe indexing — brackets are sugar for .get()

Reading a code point at a position uses brackets, same as collections. The result is `maybe string` — never crashes on out-of-bounds:

```ynz
let name = `Alice`
let first = name[0]             // sugar for name.get(0) → maybe string ("A")
let huh = name[100]             // maybe string → none (out of bounds, safe)

if (first.exists()) {
  print(first.value)            // "A"
}
```

A single character in Yinz is just a 1-length string. No separate `char` type.

---

## Strings are immutable

You cannot replace a character by index:

```ynz
let name = `Alice`
name[0] = `B`
// COMPILE ERROR: Strings are read-only. To change the value, rebuild:
//   let newName = "B" + name.substring(1)
// Or rebind the variable:
//   name = `Blice`
```

---

## Iterating over a string

`for c in s` steps through the string one code point at a time. Each `c` is a 1-character string:

```ynz
for c in `café` {
  print(c)    // c, a, f, é  (4 steps — é is one code point)
}
```

For grapheme clusters (what humans see as one character, including emoji), use `.graphemes()` — available in v0.6+.

---

## Three ways to index — code point, byte, grapheme

By default, `.get()` / `[n]` indexes by **Unicode code point**. Two additional methods give finer or coarser control:

| Method | Indexes by | Returns | Use when |
|--------|------------|---------|----------|
| `.get(n)` or `s[n]` | Code point | `maybe string` (1 code point) | Most text manipulation |
| `.byteAt(n)` | UTF-8 byte | `maybe int` (0–255) | Parsers, protocols, raw data |
| `.graphemeAt(n)` | Grapheme cluster | `maybe string` (1 grapheme) | Text rendering, cursor positioning, emoji |

Companion length methods:

```ynz
`café`.count()              // 4 (code points — what .get() counts)
`café`.byteCount()          // 5 (the é is 2 bytes in UTF-8)
`café`.graphemeCount()      // 4
```

For simple ASCII text all three return the same value. They diverge for non-ASCII:

```ynz
let s = `café`               // 'e' followed by combining acute (two code points, one grapheme)

s.byteCount()                // 6  (UTF-8 bytes: c a f e combining-accent)
s.count()                    // 5  (code points — the accent is its own code point)
s.graphemeCount()            // 4  (the e + accent fuses into one grapheme)

s.get(0).value               // "c"
s.get(3).value               // "e"     (the standalone e, not the combined character)
s.graphemeAt(3).value        // "é"     (e + accent combined — what humans see)
```

And for compound emoji:

```ynz
let family = `👨‍👩‍👧`

family.byteCount()           // 18  (UTF-8 is verbose for emoji)
family.count()               // 5   (5 code points)
family.graphemeCount()       // 1   (one human-perceived character)

family.get(0).value          // "👨"         (just the man, first code point)
family.graphemeAt(0).value   // "👨‍👩‍👧"     (the whole family — one grapheme)
```

---

## String methods

### Searching

```ynz
`Patrick`.contains(`rick`)       // true
`Patrick`.indexOf(`rick`)        // maybe int → some 3
`Patrick`.startsWith(`Pat`)      // true
`Patrick`.endsWith(`ick`)        // true
```

### Transforming

```ynz
`hello`.toUpperCase()            // "HELLO"
`HELLO`.toLowerCase()            // "hello"
`  hi  `.trim()                  // "hi"
`a,b,c`.split(`,`)              // array<string> ["a", "b", "c"]
`hello`.substring(1, 3)          // "el"
`hello`.replace(`l`, `r`)        // "herro"
```

### Lengths

```ynz
`café`.count()                   // 4 (code points)
`café`.byteCount()               // 5 (UTF-8 bytes)
`café`.graphemeCount()           // 4 (grapheme clusters)
```

---

## Unicode string equality

Two strings that look the same to a human always compare as equal, even if their underlying byte sequences differ:

```ynz
let a = `café`    // e followed by combining acute accent (2 code points for the é)
let b = `café`    // precomposed é (1 code point)

a == b             // true — Yinz normalizes to NFC before comparing
```

This is called NFC canonical equivalence. You never need to think about it for normal string comparisons.
