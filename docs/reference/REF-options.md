---
name: "REF-options"
description: "An options type defines a fixed set of named values — a multiple-choice list baked into the type system."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Options

An `options` type defines a fixed set of named values — a multiple-choice list baked into the type system.

---

## Defining options

```ynz
options Status {
  active,
  inactive,
  banned
}
```

"The options for status are: active, inactive, or banned." Plain English.

---

## Using an options value

```ynz
options Status {
  active,
  inactive,
  banned
}

function denyAccess() -> Nothing {
  print(`access denied`)
}

function entrypoint() -> Nothing {
  let current: Status = Status.active

  if (current == Status.banned) {
    denyAccess()
  }
}
```

---

## Exhaustiveness — all variants must be handled

When you use a multi-case `if` on an options value, you must handle every variant, or include `else =>`:

```ynz
options Status {
  active,
  inactive,
  banned
}

function entrypoint() -> Nothing {
  let current: Status = Status.active

  if (current) {
    active => print(`active`)
    inactive => print(`inactive`)
    banned => print(`banned`)
  }
}
```

Missing a variant is a compile error. The compiler tells you exactly which ones are missing. With the `banned` arm removed, it reports:

```
Non-exhaustive options multi-case — `Status` has 3 variants; 1 are not handled: banned.
```

Add the missing arm (`banned =>`) or add an `else =>` default arm.

---

## options vs union types

`options` is for a fixed set of named labels (like states or modes). `|` is for when a value can be one of several different shapes with different fields:

```ynz
options Status {
  active,
  inactive,
  banned
}

shape Circle {
  radius: Float
}

shape Square {
  side: Float
}

shape Figure = Circle | Square
```

Use `options` when the variants are just labels. Use `|` when they have different data shapes. See [Unions](REF-unions.md).

---

## Display strings — human-readable `.toString()` values

By default, `.toString()` on an options value returns the variant name as a string:

```ynz
options Direction {
  north,
  south,
  east,
  west
}

function entrypoint() -> Nothing {
  let heading: Direction = Direction.north
  let label = heading.toString()
  print(label)                       // north
}
```

You can attach a display string to any variant with `: ` and a backtick string:

```ynz
options Timeframe {
  oneMinute: `One Minute`
  fiveMinute: `Five Minute`
  hourly: `Hourly`
  daily: `Daily`
}

function entrypoint() -> Nothing {
  let frame: Timeframe = Timeframe.daily
  let label = frame.toString()
  print(label)                       // Daily
}
```

Variants without a display string still return their name:

```ynz
options Status {
  active: `Active`
  inactive
  banned: `Banned`
}
```

Here `Status.inactive.toString()` returns `inactive`.

Display strings are useful for UI labels and log output. The variant name is still used for comparisons and type-checking. The display string only affects `.toString()`.

---

## Commas are optional

Variants can be comma-separated or newline-separated — both work:

```ynz
options Status { active, inactive, banned }

options Timeframe {
  hourly: `Hourly`
  daily: `Daily`
}
```
