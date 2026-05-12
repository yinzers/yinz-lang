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

## options vs union types

`options` is for a fixed set of named labels (like states or modes). `or` is for when a value can be one of several different types with different fields:

```
options Status { active, inactive, banned }    // named states — same underlying type
type Shape = Circle | Square | Triangle      // different types — each has different fields
```

Use `options` when the variants are just labels. Use `or` when they have different data shapes. See [Unions](unions.md).
