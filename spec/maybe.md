# Maybe Types

Some values might not exist. `maybe` lets you say that in the type system, so the compiler can make sure you handle it.

---

## The problem

In a lot of languages, any variable can be `null` — and you only find out at runtime when your program crashes. Yinz makes absence explicit in the type, so the compiler catches it before the code ever runs.

---

## maybe<T> — a value that might not exist

```
let name: string = `Patrick`             // always has a value, guaranteed
let nickname: maybe<string> = none       // might not have a value
```

`maybe<string>` means: this is either a `string` or it's `none`. The compiler tracks which and forces you to handle the `none` case before you use it.

---

## none — the absent value

`none` is the built-in value for "nothing here." It only works with `maybe` types:

```
let name: string = none
// COMPILE ERROR: string cannot be none.
// Use maybe<string> if the value is optional.

let name: maybe<string> = none    // fine
```

---

## Accessing a maybe value

Three ways:

**Check if it exists first:**

```
if (nickname.exists()) {
  print(nickname.value)    // compiler knows it's safe inside this block
}
```

**Use a default if it's none:**

```
let display = nickname.or(`Anonymous`)    // get the value or a fallback
```

**`.value` without checking — compile error:**

```
print(nickname.value)
// COMPILE ERROR: nickname is maybe<string> — value might be none.
// Use nickname.or(default) or check nickname.exists() first.
```

---

## In function signatures

```
function findPlayer(share roster: fixed<Player>, name: string) -> maybe<Player> {
  return roster.find(p => p.name == name)    // .find() returns maybe<T>
}
```

The caller must handle the `maybe`:

```
let player = findPlayer(roster, `Alice`)

// Option 1 — use a fallback
let name = player.or(defaultPlayer).name

// Option 2 — check first
if (player.exists()) {
  print(player.value.name)
}
```

---

## `maybe<T>` and `T | none`

`maybe<string>` is the same type as `string | none`. They're interchangeable:

```
let a: maybe<string> = none
let b: string | none = none
// a and b are the same type — either works
```

`maybe` exists because it's shorter and reads more naturally. See [Unions](unions.md).

---

## Dot methods

```
value.exists()      // does it have a value? → boolean
value.or(default)   // get the value, or the fallback if none → T
value.value         // get the raw value (compile error without an exists() check)
```

---

## Early-return narrowing (M6)

The compiler also narrows `.value` after an early-return pattern — you don't need to nest the success path inside the `if` block:

```
function firstOr(nums: share array<int>) -> int {
  let r = nums.first()               // maybe<int>
  if (!r.exists()) { return -1 }     // early exit — compiler proves r has a value below
  return r.value                     // safe: the only way to reach here is if r.exists()
}
```

The early return must use `return`, `panic`, or an infinite `loop` — those are the only forms the compiler can prove always exit. A regular function call that returns `nothing` doesn't count.

See [design/narrowing.md](../design/narrowing.md) for the full rules table.
