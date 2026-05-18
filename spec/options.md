# Options

An `options` type defines a fixed set of named values — a multiple-choice list baked into the type system.

---

## Defining options

```
options Status {
  active,
  inactive,
  banned
}
```

"The options for status are: active, inactive, or banned." Plain English.

---

## Using an options value

```
let current: Status = Status.active

if (current == Status.banned) {
  denyAccess()
}
```

---

## Shorthand when the type is already known

The compiler can figure out the full name from context:

```
let ranked = players.sort(p => p.health, desc)              // shorthand — compiler knows SortOrder.desc
let ranked = players.sort(p => p.health, SortOrder.desc)    // explicit — also valid
```

IDE autocomplete surfaces the available options at the call site so you don't have to remember them.

---

## Built-in options

These are part of the standard library — no import needed:

```
options SortOrder {
  asc,
  desc
}

options Comparison {
  equal,
  greater,
  less
}
```

---

## Exhaustiveness — all variants must be handled

When you use multi-case `if` on an options type, you must handle every variant — or include `else =>`:

```
if (current) {
  Status.active  => print("active")
  Status.inactive => print("inactive")
  Status.banned  => print("banned")
}
```

Missing a variant is a compile error. The compiler tells you exactly which ones are missing.

```
// COMPILE ERROR: Non-exhaustive options multi-case — Status has 3 variants; only 2 are handled.
// Missing variant: banned
// Add: Status.banned => ...  or add: else => ...
```

---

## Shorthand — when the type is already known (revisited)

If two options types in scope both define the same variant name, shorthand is ambiguous and the compiler rejects it:

```
options SortOrder { asc, desc }
options Direction { up, down, desc }   // also has 'desc'

let sorted = players.sort(p => p.health, desc)
// COMPILE ERROR: 'desc' is ambiguous — it's a variant of both SortOrder and Direction.
// Use the qualified form: SortOrder.desc or Direction.desc
```

If a function and an options variant share the same name, the function takes priority. If that produces a type error, qualify the variant explicitly.

---

## options vs union types

`options` is for a fixed set of named labels (like states or modes). `|` is for when a value can be one of several different types with different fields:

```
options Status { active, inactive, banned }    // named states — same underlying type
shape Shape = Circle | Square | Triangle      // different types — each has different fields
```

Use `options` when the variants are just labels. Use `|` when they have different data shapes. See [Unions](unions.md).
