# Strings

Text in Yinz.

---

## String literals — double quotes

```
let name: string = "Patrick"
let greeting: string = "Hello, world"
```

---

## Interpolation — backticks with ${}

Put any value inside a string using `${}`:

```
let msg = `Hello ${name}, you have ${health} HP`
let debug = `Player at (${pos.x}, ${pos.y}) with ${player.health} health`
```

Any expression works inside `${}`:

```
let label = `Score: ${player.score * 2}`
let status = `${active.count()} players active`
```

---

## Concatenation — use + to join strings

```
let full = "Hello " + name
let path = folder + "/" + filename
```

---

## string is a built-in type

No imports needed. `string` is available everywhere.

---

## Multi-line strings

Multi-line strings use backticks:

```
let message = `
  Dear ${name},
  Your score is ${score}.
`
```

---

## Safe indexing — brackets are sugar for .get()

Reading a character at a position uses brackets, same as collections. The result is `maybe string` — never crashes on out-of-bounds:

```
let name = "Alice"
let first = name[0]             // sugar for name.get(0) → maybe string ("A")
let huh = name[100]             // maybe string → none (out of bounds, safe)

if (first.exists()) {
  print(first.value)            // "A"
}
```

A single character in Yinz is just a 1-length string. No separate `char` type — that's one less concept to learn, and it matches how JavaScript treats characters.

---

## Strings are immutable

You cannot replace a character by index:

```
let name = "Alice"
name[0] = "B"
// COMPILE ERROR: Strings are immutable. To change the value, rebuild:
//   let newName = "B" + name.substring(1)
// Or rebind the variable:
//   name = "Blice"
```

This avoids a whole class of bugs around in-place re-encoding and mutation in multi-byte text.

---

## Three ways to index — code point, byte, grapheme

By default, `.get()` / `[n]` indexes by **Unicode code point** — what users typically think of as a "character." Two additional methods exist for the cases where you need different semantics:

| Method | Indexes by | Returns | Use when |
|--------|------------|---------|----------|
| `.get(n)` or `s[n]` | Code point | `maybe string` (1 code point) | Most text manipulation |
| `.byteAt(n)` | UTF-8 byte | `maybe int` (0-255) | Parsers, protocols, raw data |
| `.graphemeAt(n)` | Grapheme cluster | `maybe string` (1 grapheme) | Text rendering, cursor positioning, emoji handling |

Companion length methods:

```
"café".count()              // 4 (code points — what .get() iterates over)
"café".byteCount()          // 5 (the é is 2 bytes in UTF-8)
"café".graphemeCount()      // 4
```

For simple ASCII text, all three return the same thing. They diverge for non-ASCII:

```
let s = "café"               // 'e' followed by combining acute (two code points, one grapheme)

s.byteCount()                // 6  (UTF-8 bytes: 'c' 'a' 'f' 'e' ́)
s.count()                    // 5  (code points — the accent is its own code point)
s.graphemeCount()            // 4  (the 'e' + accent fuses into one grapheme)

s.get(0).value               // "c"
s.get(3).value               // "e"     (the standalone 'e', not the combined character)
s.graphemeAt(3).value        // "é"     (e + accent combined — what humans see)
```

And for compound emoji:

```
let family = "👨‍👩‍👧"

family.byteCount()           // 18  (UTF-8 is verbose for emoji)
family.count()               // 5   (5 code points: man + zero-width-joiner + woman + zwj + girl)
family.graphemeCount()       // 1   (one human-perceived character)

family.get(0).value          // "👨"            (just the man, first code point)
family.graphemeAt(0).value   // "👨‍👩‍👧"        (the whole family — one grapheme)
```

Use `.get()` / `[n]` for everyday text. Reach for `.byteAt()` and `.graphemeAt()` only when you actually need byte-level or human-perceived-character semantics.
