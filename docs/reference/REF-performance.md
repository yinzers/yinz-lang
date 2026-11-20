---
name: "REF-performance"
description: "What Yinz does for speed without you asking: native code, no garbage collector, inline fields, fixed-size lists and automatic layout choices."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Performance

Yinz is fast by default. This page lists what the compiler does for you and what you write to get it.

---

## Native code

Yinz compiles ahead of time to native machine code. There is no interpreter and no just-in-time compiler running next to your program.

---

## No garbage collector

Memory is freed when its owner goes out of scope. The compiler works out who owns each value, so the program does no background cleanup and has no pauses.

---

## Flat memory layout

When you declare a `shape`, its fields sit side by side in memory. A shape inside another shape is stored inline, not as a link to somewhere else:

```ynz
shape Position {
  x: Float
  y: Float
}

shape Player {
  name: String
  health: Int
  position: Position
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, health: 100, position: { x: 1.5, y: 2.5 } }
  print(player.position.x.toString())
}
```

`position` lives directly inside `Player`. Reading `player.position.x` reads memory right next to the rest of the `Player`.

---

## Fast choices are the default

`Fixed<T>` lists are stored on the stack and their size is locked at creation. Stack storage is faster than heap storage.

A shape is faster than a `Map<String, Int>` for values whose names you know when you write the code. A shape field is read at a fixed position. A map looks each key up while the program runs.

```ynz
shape Stats {
  health: Int
  attack: Int
  defense: Int
}

function entrypoint() -> Nothing {
  let stats: Stats = { health: 100, attack: 12, defense: 8 }
  print(stats.attack.toString())
}
```

---

## What the compiler does for you

- **Types:** you leave out annotations the compiler can work out from the value.
- **Fixed lists:** an `Array<T>` that never grows is stored as `Fixed<T>`.
- **Constants:** a `let` that is never reassigned is treated as `const`.
- **Memory layout:** nested shapes are stored inline. A large `Array` of shape values whose loops read at most two fields is stored as one block of memory per field.
- **Allocation:** values live on the stack when they can and on the heap only when they must.
- **Ownership:** memory is freed exactly when its owner is done with it.

The IDE shows these choices as hints. See [REF-ide-hints.md](REF-ide-hints.md) and [REF-linting.md](REF-linting.md).
