---
name: "coming-from-go"
description: "A guide for Go developers: what you write in Go, what you write in Yinz, and the habits the compiler will correct."
tags:
  - "yinz-compiler"
  - "guide"
created_at: "2026-11-20"
updated_at: "2026-11-20"
status: "active"
author: "yinz"
metadata:
  type: "guide"
---

# Coming from Go

If you write Go, a lot of Yinz will feel familiar: plain keywords, small programs, one way to do most things, and a compiler that is strict about unused work and mismatched types. What changes is what sits underneath and how much you spell out. There is no garbage collector: every value has one owner, and a stored value cannot be used again after it is handed over. There is no `nil`, no `if err != nil`, no `struct` with methods on receivers, no pointers (`*` and `&`), no `:=`, no `iota`, no `switch`, no `else`, no `package`, and no `go` statement. This guide lists the differences, shows both sides, and shows the compiler message you get when an old habit slips in.

The compiler teaches as you go. Every message below says what is wrong, what to write instead, and why.

A note on the examples: Go examples do not show the Go compiler's messages; a comment says what Go does.

## The concept list

The table and the traps below follow this list, in this order:

1. Variables
2. Text
3. Comments and line endings
4. Data structures
5. Functions
6. Missing values
7. Failures
8. Choosing between kinds of values
9. Named value sets
10. Collections and reading by position
11. Numbers
12. Operators
13. Branching
14. Chaining
15. Files and imports
16. Slow work
17. Copies

## In Go you write → In Yinz you write

| Concept | In Go you write | In Yinz you write |
|---|---|---|
| Variables | `health := 100` can change<br>`var health int = 100` can change<br>`const maxPlayers = 10` can never change, numbers, text and booleans only | `let health = 100` can change<br>`const health = 100` can never change<br>`const maxPlayers = 10` |
| Text | `"Hi"`, `` `raw text` `` (no `${}`)<br>`fmt.Sprintf("Hi %s", name)` | `` `Hi ${name}` `` (backticks only) |
| Comments and line endings | `// note`, `/* note */`<br>`;` ends a statement, but you leave it out | `// note`, `/// doc comment`, no `;` |
| Data structures | `type Player struct { Name string }` | `shape Player { name: String }` |
| Functions | `func (p *Player) Heal(amount int) { }`<br>`func (p Player) Show() string { }` | `function heal(lend self: Player, amount: Int) -> Nothing { }`<br>`function show(share self: Player) -> String { }` |
| Missing values | `nil`, `*string`<br>`value, found := scores["Alice"]` | `Maybe<String>`, `none`<br>`nickname.value` after `if (nickname.exists()) { }` |
| Failures | `(int, error)`, `errors.New("...")`<br>`if err != nil { return 0, err }`, `panic("...")` | `function load() -> String errors { }`<br>`content.failed()`, `content.or(...)` |
| Choosing between kinds of values | `switch s := outline.(type) { case Circle: ... }`<br>`interface`, `any` | `shape Shape = Circle \| Square`<br>`if (shape) { is Circle => ... }` |
| Named value sets | `const ( StatusActive Status = iota; StatusBanned )` | `options Status { active, banned }` |
| Collections and reading by position | `[]int`, `[3]int`, `map[string]int`<br>`scores[5]` panics; a missing map key gives `0` | `Array<Int>`, `Fixed<Int>`, `Map<String, Int>`<br>`scores[5]` is a `Maybe<Int>` |
| Numbers | `int`, `int32`, `uint8`, `float64`<br>overflow wraps around silently | `Int`, `Number`, `Float`<br>overflow stops the program |
| Operators | `&&`, `\|\|`, `!`, `==`<br>`score++`, `score += 5` | `&&`, `\|\|`, `!`, `==`<br>`score = score + 5` |
| Branching | `if { } else if { } else { }`<br>`switch code { case 200: ... default: ... }` | early `return`<br>`if (code) { 200 => ... else => ... }` |
| Chaining | Go has no filter-and-map chains; you write a `for` loop with `append` | a `for` loop, one named step per line |
| Files and imports | `package services`<br>`import "myapp/services"`<br>`FetchUser` (capital letter exports), `func main()` | `` import { fetchUser } from `services/users` ``<br>`export function`<br>`function entrypoint()` |
| Slow work | `go fetch()`, `sync.WaitGroup`<br>`chan int`, `wire <- 2`, `<-wire` | plain `function`; `wait` and `background` only when you need them<br>`Channel<Int>`, `wire.send(2)`, `wire.receive()` |
| Copies | `b := a` copies a struct, but a slice, a map or a pointer is shared<br>`slices.Clone(a)` | `let b = a` hands the value over<br>`a.copy()` |

## Traps

### Variables: `let` can change, and `const` never changes

In Go, `:=` and `var` make a name that can change, and `const` makes one that never changes. In Yinz, `let` makes a name that can change, and `const` makes one that never changes. Go's `const` only holds numbers, text and `true` or `false`. In Yinz, `const` can hold anything, a shape value included, and then none of its fields can change either. Use `const` by default and `let` when the value needs to change.

Go:

```go
package main

func main() {
	const maxPlayers = 10
	maxPlayers = 20 // does not compile: a constant cannot be assigned to
}
```

Yinz:

```
function startRound() -> Nothing {
  const maxPlayers = 10
  maxPlayers = 20
}
// COMPILE ERROR: `maxPlayers` cannot be changed — it was declared with `const`.
// Change `const maxPlayers = ...` to `let maxPlayers = ...` if you need to reassign it.
```

There is no `:=` and no `var`. Both are Go habits, and the compiler gives only general errors for them, so remember the rule instead: write `let health = 100`.

Inside a function you do not write the type; the compiler works it out, as `:=` does. Write a type when you want one, after a colon: `let health: Int = 100`.

A capital letter always means a type, so every name that is not a type is camelCase. In Go, a capital letter means "exported", so `MaxPlayers` was public. In Yinz, `export` marks a name as public, and its spelling stays `maxPlayers`.

### Text: backticks only, and `${}` replaces `fmt.Sprintf`

Go has two forms of text: `"..."` with escapes like `\n`, and backticks for raw text. Backticks in Go never insert values. In Yinz, backticks are the only form, and they do insert values with `${}`. They also run over several lines. The same characters, a different job:

Go:

```go
package main

import "fmt"

func main() {
	name := "Alice"
	greeting := fmt.Sprintf("Hello %s", name)
	fmt.Println(greeting) // Hello Alice

	raw := `Hello ${name}`
	fmt.Println(raw) // Hello ${name}
}
```

Yinz:

```
let name = `Alice`
let greeting = `Hello ${name}`
print(greeting)    // Hello Alice
```

Double quotes do not exist:

```
let greeting = "Hello"
// COMPILE ERROR: Double-quoted strings don't exist in Yinz.
// Use backtick strings instead: `` `hello` `` or `` `Hello ${name}` ``
```

There is no `fmt.Sprintf` and no `%s` or `%d` placeholders. Put the value inside the text with `${}`, as in `` `Hello ${name}` ``.

`fmt.Println` takes any number of values and puts a space between them. In Yinz, `print` is a function that takes exactly one value:

```
print(`Health:`, health)
// COMPILE ERROR: `print` takes 1 argument, but 2 were given.
// Call it with one value: `print(value)`
```

Write ``print(`Health: ${health}`)`` instead.

### Comments and line endings: `//`, `///`, and no `;`

`//` works as in Go. A doc comment is written with `///` directly above a declaration, where Go uses a plain `//` comment that starts with the name. A doc comment shows in the editor when you hover over the name. There is no `/* */` block comment.

```
/// Adds `amount` to the player's health.
function heal(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health + amount
}
```

In Go, the compiler adds the `;` at the end of a line for you, and `gofmt` removes any you type. In Yinz, a newline ends a statement, and a `;` is an error:

```
let health = 100;
// COMPILE ERROR: Semicolons are not used in Yinz.
// Remove the `;` — Yinz uses newlines to end statements.
```

A function gives back a value with `return value`, as in Go.

### Data structures: `shape` holds the fields, and behavior goes in functions

A `shape` lists fields and nothing else, one per line. In Go, a `struct` has fields, and you attach methods to it with a receiver in parentheses: `func (p *Player) Heal(...)`. In Yinz there are no receivers. A method is a standalone function, and `player.heal(20)` is shorthand for `heal(player, 20)`, so you still get autocomplete after the dot.

You build a value by writing its fields. The type goes after the name, and field names are camelCase.

Go:

```go
package main

import "fmt"

type Player struct {
	Name   string
	Health int
}

func (p *Player) Heal(amount int) {
	p.Health += amount
}

func main() {
	player := Player{Name: "Alice", Health: 50}
	player.Heal(20)
	fmt.Println(player.Name, "has", player.Health) // Alice has 70
}
```

Yinz:

```
shape Player {
  name: String
  health: Int
}

function heal(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health + amount
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 50 }
  player.heal(20)
  print(player.health)    // 70
}
```

`struct` is not valid in Yinz. Writing `type Player struct {` gives errors, and among them this warning:

```
type Player struct {
// COMPILE WARNING: `struct` is not a keyword in Yinz.
// `struct` not valid here
```

Its hint says: "Use `shape` to declare a data type: `shape Player { name: String, health: Int }`".

A method with a receiver, such as `func (p *Player) Heal(...)`, is not valid either. The compiler gives only general errors for it, so remember the rule instead: write a standalone function whose first parameter is `self`, as `heal` does above.

### Functions: `share`, `lend` and `give` replace `*Player` and `Player` receivers

A parameter needs a type, and the function needs a return type. `-> Nothing` is a Go function with no result. Inside the body you write no types.

In Go you choose between a pointer, `*Player`, which can change the original, and a plain `Player`, which is a copy. In Yinz you write how the function uses the value, in front of the parameter, in the signature only:

| Go | Yinz | The function |
|---|---|---|
| `p Player`, a copy you only read | `share player: Player` | only reads the value |
| `p *Player`, changes the original | `lend player: Player` | changes the value |
| nothing; Go's garbage collector keeps a value alive while anything points to it | `give player: Player` | takes the value for good |

Nothing is written at the call. Go's `heal(&player, 20)` becomes `heal(player, 20)`. You write no `&` to take an address and no `*` to read through one. Your editor shows a muted note at the call saying which of the three the function uses.

In Go, a value receiver can change a field and the change is silently lost, because it changed a copy:

```go
package main

import "fmt"

type Player struct {
	Name   string
	Health int
}

func (p Player) Heal(amount int) {
	p.Health += amount // changes a copy; the caller never sees it
}

func main() {
	player := Player{Name: "Alice", Health: 50}
	player.Heal(20)
	fmt.Println(player.Health) // 50
}
```

In Yinz, the compiler stops you when a function that only reads tries to change a value:

```
function heal(share self: Player, amount: Int) -> Nothing {
  self.health = self.health + amount
}
// COMPILE ERROR: `self` is declared `share` — read-only — so its fields cannot be changed.
// Change the parameter to `lend self: Player` if this function needs to modify it.
```

`*` and `&` in a signature are not valid. The compiler gives only general errors for them, so remember the rule instead: write `share`, `lend` or `give` in front of the parameter, and no `*`.

### Missing values: `Maybe<T>` and `none` replace `nil`

A value that might be missing has the type `Maybe<T>`. It is either a `T` or `none`. You cannot use it until you handle the `none` case. A `Player` is never `none`, so there is no nil pointer to trip over.

Go:

```go
package main

import "fmt"

func main() {
	var nickname *string // nil: no nickname
	display := "Anonymous"
	if nickname != nil {
		display = *nickname
	}
	fmt.Println(display) // Anonymous
}
```

Yinz:

```
let nickname: Maybe<String> = none
let display = nickname.or(`Anonymous`)

if (nickname.exists()) {
  print(nickname.value)
}
```

`if nickname != nil` becomes `if (nickname.exists())`, and inside the block `nickname.value` is safe. Giving a default is `.or(x)`.

Go lets you read through a nil pointer, and the program panics when it runs. In Yinz, skipping the check does not compile:

```
let nickname: Maybe<String> = none
print(nickname.value)
// COMPILE ERROR: `Maybe.value` requires you to first check `m.exists()`.
// Add a check: `if (m.exists()) { print(m.value) }`. Or use a default: `m.or(0)`.
```

`nil` does not exist. Write `none`: `let nickname: Maybe<String> = none`. The compiler gives only general errors for `nil`, so remember the rule.

`Maybe` means "no value" is a normal outcome. When the caller must deal with a failure, use `errors`. Go's `value, found := scores["Alice"]` is also a `Maybe`: the Collections trap shows it.

### Failures: `errors` replaces `(value, error)`, `if err != nil` and `panic`

A function that can fail says so in its signature with the word `errors`. It still returns one value. There is no second `error` result, no `errors.New` to build one and no `if err != nil` after the call. The caller supplies a fallback with `.or(...)`, checks with `.failed()`, or passes the failure up by marking its own function `errors` too. Using the result of an `errors` function without handling it is a compile error.

Go:

```go
package main

import (
	"errors"
	"fmt"
)

func battingAverage(hits int, atBats int) (int, error) {
	if atBats == 0 {
		return 0, errors.New("no at-bats")
	}
	return hits * 1000 / atBats, nil
}

func main() {
	average, err := battingAverage(127, 400)
	if err != nil {
		average = 0
	}
	fmt.Println(average) // 317
}
```

Yinz:

```
function battingAverage(hits: Int, atBats: Int) -> Int errors {
  return (hits * 1000) / atBats
}

function entrypoint() -> Nothing {
  let attempt = battingAverage(127, 400)
  let average = attempt.or(0)    // the fallback value, used when the call fails
  print(average)    // 317
}
```

The Go version returns an error when `atBats` is zero, and the caller sets `average` to 0. Yinz does not work that way. Dividing by zero is not a failure you can catch: it stops the program with `RUNTIME ERROR: division by zero (Int)`. Check the number before you divide.

To look at the failure yourself, use `.failed()`:

```
let checked = battingAverage(50, 200)
if (checked.failed()) {
  print(`battingAverage failed`)
}
```

Two results, as in `-> (Int, Error)`, are not valid in a signature. The compiler gives only general errors for them, so remember the rule: one return type, and add the word `errors` after it.

To pass a failure up, write `errors` on the calling function. Go's `return 0, err` has no match, because there is nothing to return by hand.

Go lets you drop an error with `average, _ := battingAverage(127, 400)`. In Yinz, a failure left unhandled is a compile error, so it cannot slip through:

```
let attempt = battingAverage(127, 400)
print(attempt)
// COMPILE ERROR: This function can fail, but the failure is not handled here.
// Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.
```

`errors` is for failures the caller can expect, such as a file that is not there. A bug, such as an overflowing `Int`, stops the whole program. You do not write `panic(...)` for it, because Yinz has no such call. Mark the function `errors` for a failure the caller can expect, and let the compiler stop the program for a bug.

### Choosing between kinds of values: a union of shapes replaces type switches

In Go, a value that can be one of several types is held in an interface, and a type switch picks it apart. Go's compiler does not tell you when a case is missing from the switch. In Yinz, each kind is its own `shape`, and a union joins them with `|`. You check the kind with `is` inside an `if` with one arm per kind. Inside an arm, the value already has the right kind, so you read its fields directly.

Go:

```go
package main

import "fmt"

type Circle struct{ Radius int }
type Square struct{ Side int }
type Triangle struct{ BaseLength, Height int }

type Shape any

func describe(outline Shape) {
	switch s := outline.(type) {
	case Circle:
		fmt.Println("circle, radius", s.Radius)
	case Square:
		fmt.Println("square")
	case Triangle:
		fmt.Println("triangle")
	}
}

func main() {
	describe(Circle{Radius: 3}) // circle, radius 3
}
```

Yinz:

```
shape Circle {
  radius: Int
}

shape Square {
  side: Int
}

shape Triangle {
  baseLength: Int
  height: Int
}

shape Shape = Circle | Square | Triangle

function describe(share outline: Shape) -> Nothing {
  if (outline) {
    is Circle => {
      print(`circle, radius ${outline.radius}`)
    }
    is Square => {
      print(`square`)
    }
    is Triangle => {
      print(`triangle`)
    }
  }
}

function entrypoint() -> Nothing {
  let circle: Circle = { radius: 3 }
  describe(circle)    // circle, radius 3
}
```

`base` is a reserved word, so a field cannot use that name; the Triangle field is called `baseLength`.

A forgotten kind is an error, and the compiler names it:

```
if (outline) {
  is Circle => { print(`circle`) }
  is Square => { print(`square`) }
}
// COMPILE ERROR: Non-exhaustive union multi-case — 1 variant is not handled: Triangle.
// Add the missing arms (e.g. `is Triangle =>`) or add an `else =>` default arm.
```

Go's type switch is not valid. Writing `switch outline.(type) {` gives errors, and among them this warning:

```
switch outline.(type) {
// COMPILE WARNING: `switch` is not a keyword in Yinz.
// Use multi-case `if` with `=>` arms: `if (value) { 1 => ...; 2 => ...; else => ... }`
```

A Go `interface` also describes what a value can do, such as having a `Heal` method. That is a separate topic in Yinz, called contracts and written with `follows`. This guide does not cover it.

### Named value sets: `options` replaces `iota` constants

An `options` declaration lists the allowed names. There is no `iota`, no `type Status int` and no prefix such as `StatusActive`: you write the names once, and you pick one with `Status.active`.

Go:

```go
package main

import "fmt"

type Status int

const (
	StatusActive Status = iota
	StatusBanned
)

func main() {
	status := StatusActive
	switch status {
	case StatusActive:
		fmt.Println("online")
	case StatusBanned:
		fmt.Println("banned")
	}
}
```

Yinz:

```
options Status {
  active
  banned
}

function entrypoint() -> Nothing {
  let status: Status = Status.active
  if (status) {
    active => {
      print(`online`)
    }
    banned => {
      print(`banned`)
    }
  }
}
```

This prints `online`. In Go, `Status(7)` is a valid `Status` and nothing checks that your `switch` covers every constant. In Yinz, the compiler checks that your `if` handles every name in the set.

`iota` does not exist, and a `const ( ... )` group is not how Yinz lists names. The compiler gives only general errors for it, so remember the rule: use `options`, as above.

Writing `enum` gives errors, and among them this warning:

```
enum Status {
// COMPILE WARNING: `enum` is not a keyword in Yinz.
// Use `options` to declare a set of named values: `options Status { active, inactive, banned }`
```

### Collections and reading by position: reading never panics

Every Yinz type starts with a capital letter, built-in collections included:

| Go | Yinz |
|---|---|
| `[]int`, a slice that grows | `Array<Int>`, grows |
| `[3]int`, size fixed | `Fixed<Int>`, size set at creation |
| `map[string]int` | `Map<String, Int>` |

In Go, `scores[5]` on a slice with three items panics, and a missing map key quietly gives the zero value, so a player with `0` hits and a player who is not in the map look the same. To tell them apart you write `hits, found := m[key]`. In Yinz, reading by position or by key gives a `Maybe`, so the plain `[ ]` is already the safe form.

Go:

```go
package main

import "fmt"

func main() {
	scores := []int{70, 85, 92}
	// fmt.Println(scores[5]) would panic: index out of range [5] with length 3
	score := 0
	if len(scores) > 5 {
		score = scores[5]
	}
	fmt.Println(score) // 0

	hitLeaders := map[string]int{"clemente": 3000, "wagner": 3420}
	hits := hitLeaders["stargell"]
	fmt.Println(hits) // 0, and you cannot tell "missing" from a real 0
}
```

Yinz:

```
let scores: Array<Int> = [70, 85, 92]
let foundScore = scores[5]
let score = foundScore.or(0)
print(score)    // 0

let hitLeaders: Map<String, Int> = { `clemente`: 3000, `wagner`: 3420 }
let foundHits = hitLeaders[`stargell`]
let hits = foundHits.or(0)
print(hits)    // 0
```

The everyday functions have new names, and they are called with a dot:

| Go | Yinz |
|---|---|
| `len(scores)` | `scores.count()` |
| `scores = append(scores, 88)` | `scores.add(88)` |
| `slices.Contains(scores, 88)` | `scores.contains(88)` |
| `scores[len(scores)-1]`, panics when empty | `scores.last()`, a `Maybe` |

Go's collection names are not valid. `[]Int` gives only general errors, so remember the rule: write `Array<Int>`. There is no `len(...)` either; write `scores.count()`.

A value written with `{ ... }` is a single shape value, not a collection. Use `[...]` to make one:

```
let team: Array<Player> = { name: `Alice`, health: 100 }
// COMPILE ERROR: `{ ... }` creates a single `Player` value, not an `Array<Player>`.
// Put it inside `[...]` to make an array: `[{ ... }]`
```

Writing past the end by position stops the whole program with no message, so use `.add(item)` to grow an `Array`.

### Numbers: one `Int`, an exact `Number`, and overflow stops the program

Go makes you choose a size and sign: `int8`, `int32`, `uint64`, and plain `int`, which is 64 bits on a 64-bit computer. In Yinz, `Int` is the whole number type for everyday use, and it holds about ±9.2 × 10^18. A result that does not fit stops the whole program with `RUNTIME ERROR: Int overflow in '<op>' at …`. In Go, the same mistake wraps around with no warning:

```go
package main

import (
	"fmt"
	"math"
)

func double(score int32) int32 {
	return score * 2
}

func main() {
	fmt.Println(double(math.MaxInt32)) // -2
}
```

A size such as `int32` is not a Yinz type:

```
let health: int32 = 100
// COMPILE ERROR: `int32` is not a known type.
```

Use `Int`.

Whole-number division works as in Go: `/` on two `Int` values gives an `Int`, and the decimals are cut off. `//` starts a comment, so it is not an operator:

```
let half = 7 / 2
print(half)    // 3
```

For a result with decimals, use decimal numbers: `7.0 / 2.0`.
Go's `float64` gives `0.30000000000000004` for `0.1` plus `0.2` once the numbers are stored in variables. Yinz has two decimal types. `Number` stores decimal digits exactly, so you get the answer you expect. `Float` is fast binary arithmetic like `float64`, with the same tiny rounding differences:

```go
package main

import "fmt"

func main() {
	a := 0.1
	b := 0.2
	fmt.Println(a + b) // 0.30000000000000004
}
```

Writing `fmt.Println(0.1 + 0.2)` in Go prints `0.3`, because Go adds number literals exactly before it stores the result. That is a Go detail; in Yinz you do not need it.

```
let total = 0.1 + 0.2
print(total)    // 0.3
```

Use `Int` for whole numbers, `Number` for anything where the exact value matters, and `Float` only when you want speed and can accept rounding differences. Go's `float64` is not a Yinz type:

```
let price: float64 = 24.5
// COMPILE ERROR: `float64` is not a known type.
```

Use `Float` or `Number`.

### Operators: `&&`, `||`, `!` and `==` match Go, and `++` and `+=` do not exist

The logic operators are the same as in Go:

```
function canEnter(score: Int, banned: Boolean, isAdmin: Boolean) -> Boolean {
  if (isAdmin) {
    return true
  }
  return score >= 10 && !banned
}
```

A single `|` is for unions, not for "or".

As in Go, `==` never converts types silently, and comparing different types is an error. Go says `mismatched types string and int`:

```
let input: String = `5`
let count: Int = 5
if (input == count) { print(1) }
// COMPILE ERROR: `==` cannot be used with `String` and `Int`.
// Check that both sides have the same type.
```

Go's `strconv.Atoi(input)` gives back `(int, error)`. In Yinz, `.toInt()` gives a `Maybe<Int>`:

```
let input: String = `5`
let parsed = input.toInt()
let count = parsed.or(0)
print(count)    // 5
```

`+=`, `-=`, `*=`, `/=` and `++` do not exist. Write the assignment out:

```
let score = 10
score += 5
// COMPILE ERROR: `+=` is not supported in Yinz.
// Use `x = x + n` instead.

score = score + 5
```

```
score++
// COMPILE ERROR: `++` is not supported in Yinz.
// Use `x = x + 1` instead.
```

### Branching: no `else`, no `switch`

Go already prefers an early `return` over an `else` after a `return`. In Yinz, handle one case with an early `return`, then carry on. This replaces an `if` / `else if` / `else` chain. The condition always has parentheses, and the block always has braces. Go writes neither parentheses nor a `;`.

Go:

```go
package main

func describe(health int) string {
	if health <= 0 {
		return "dead"
	} else if health < 30 {
		return "in danger"
	} else {
		return "healthy"
	}
}
```

Yinz:

```
function describe(health: Int) -> String {
  if (health <= 0) {
    return `dead`
  }
  if (health < 30) {
    return `in danger`
  }
  return `healthy`
}
```

A condition without parentheses is not valid:

```
if health <= 0 {
// COMPILE ERROR: Expected `(` after `if`.
// Write `if (condition) { ... }` with the condition in parentheses.
```

An `else` block is not valid:

```
if (health <= 0) {
  return `dead`
} else {
  return `alive`
}
// COMPILE ERROR: `else` cannot appear here.
```

Yinz has no `else` block, so end the first branch with `return` and carry on below it.

When you cannot return, set a value first and change it. This replaces Go's `message := "in danger"` followed by an `if` that changes it, and Go has no `if` that gives a value either:

```
let message = `in danger`
if (health > 50) {
  message = `healthy`
}
print(message)
```

To compare one value against several cases, use a multi-case `if`. `else =>` is the catch-all, like Go's `default:`:

```go
package main

import "fmt"

func describeStatus(statusCode int) {
	switch statusCode {
	case 200:
		fmt.Println("ok")
	case 404:
		fmt.Println("not found")
	default:
		fmt.Println("unexpected:", statusCode)
	}
}
```

```
function describeStatus(statusCode: Int) -> Nothing {
  if (statusCode) {
    200 => {
      print(`ok`)
    }
    404 => {
      print(`not found`)
    }
    else => {
      print(`unexpected: ${statusCode}`)
    }
  }
}
```

Writing `switch` gives errors, and among them this warning:

```
switch statusCode {
// COMPILE WARNING: `switch` is not a keyword in Yinz.
// Use multi-case `if` with `=>` arms: `if (value) { 1 => ...; 2 => ...; else => ... }`
```

### Chaining: a loop with `append` becomes named steps

Go has no built-in filter-and-map chains, so you build a result with a `for` loop and `append`. Yinz has no chaining at all. Each step goes on its own line with a name, so you can inspect every value. A loop that builds a collection one item at a time looks like Go's.

Go:

```go
package main

import "fmt"

type Player struct {
	Name   string
	Health int
}

func main() {
	players := []Player{
		{Name: "Alice", Health: 100},
		{Name: "Bob", Health: 0},
		{Name: "Cara", Health: 80},
	}
	var activeNames []string
	for _, player := range players {
		if player.Health > 0 {
			activeNames = append(activeNames, player.Name)
		}
	}
	fmt.Println(len(activeNames)) // 2
}
```

Yinz:

```
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let players: Array<Player> = [
    { name: `Alice`, health: 100 },
    { name: `Bob`, health: 0 },
    { name: `Cara`, health: 80 }
  ]
  let activeNames: Array<String> = []
  for (player in players) {
    if (player.health > 0) {
      activeNames.add(player.name)
    }
  }
  let activeCount = activeNames.count()
  print(activeCount)    // 2
}
```

A chain of calls written on one line is not written in Yinz. Give each result a name on its own line, as above.

### Files and imports: no `package`; paths start at the project root

In Go, every file starts with `package name`, a package is a folder of files, and a capital first letter exports a name. You call it through the package name: `services.FetchUser(7)`. In Yinz there is no `package` line: a file is found by its path. A single `import` brings in the names you list, and you call them without a prefix. Import paths start at your project root and are written in backticks. A leading `./` or `../` is an error:

```
import { fetchUser } from `../services/users`
// COMPILE ERROR: Import paths must be project-root relative, not relative paths.
// Write `services/users` instead of `../services/users`.
```

Go, in a project whose `go.mod` says `module myapp`:

```go
// services/users.go
package services

import "fmt"

func FetchUser(userID int) string {
	return fmt.Sprintf("user %d", userID)
}
```

```go
// main.go
package main

import (
	"fmt"
	"myapp/services"
)

func main() {
	fmt.Println(services.FetchUser(7)) // user 7
}
```

Yinz, with the exporting file above it:

```
// services/users.ynz
export function fetchUser(userId: Int) -> String {
  return `user ${userId}`
}
```

```
// entrypoint.ynz
import { fetchUser } from `services/users`

function entrypoint() -> Nothing {
  print(fetchUser(7))    // user 7
}
```

Names are private unless you mark them. A capital letter no longer means "exported": `export` does, and there are no default exports. Every Yinz program starts at `function entrypoint()`, where Go has `func main()`.

A `package` line is not valid. The compiler gives only general errors for it, so remember the rule: leave it out, and let the file's path be its name.

Two imports cannot use the same name. Yinz stops you and asks you to rename one:

```
import { Player } from `models/game`
import { Player } from `external/legacy`
// COMPILE ERROR: Import name `Player` is already bound by a previous import.
// Use a different local name: `import { Player as otherName } from "..."`
```

### Slow work: no `go` statement; the compiler overlaps independent calls

In Go, you start a goroutine with `go`, wait for the group with a `sync.WaitGroup`, and pass results by variable or by channel. In Yinz, you mark nothing. When a call has to pause, such as a network call, the compiler pauses your function by itself. Independent calls run at the same time without extra code, so there is no `WaitGroup`.

Go:

```go
package main

import (
	"fmt"
	"sync"
	"time"
)

func fetchLowerBowlPrice() float64 {
	time.Sleep(60 * time.Millisecond)
	return 24.50
}

func fetchUpperDeckPrice() float64 {
	time.Sleep(60 * time.Millisecond)
	return 31.75
}

func main() {
	var lowerBowl, upperDeck float64
	var group sync.WaitGroup
	group.Add(2)
	go func() {
		defer group.Done()
		lowerBowl = fetchLowerBowlPrice()
	}()
	go func() {
		defer group.Done()
		upperDeck = fetchUpperDeckPrice()
	}()
	group.Wait()
	fmt.Println("lower bowl ticket:", lowerBowl) // lower bowl ticket: 24.5
	fmt.Println("upper deck ticket:", upperDeck) // upper deck ticket: 31.75
}
```

Yinz:

```
function fetchLowerBowlPrice() -> Number {
  wait sleep(60)
  return 24.50
}

function fetchUpperDeckPrice() -> Number {
  wait sleep(60)
  return 31.75
}

function entrypoint() -> Nothing {
  let lowerBowl = fetchLowerBowlPrice()
  let upperDeck = fetchUpperDeckPrice()
  print(`lower bowl ticket: ${lowerBowl}`)
  print(`upper deck ticket: ${upperDeck}`)
}
```

The two fetches have nothing to do with each other, so they overlap: the wait is about 60 milliseconds, not 120. The `sleep` takes milliseconds, as `60 * time.Millisecond` does.

Your editor shows a muted `wait` hint where a call pauses. Write `wait` yourself only when two calls must finish in a set order and the compiler cannot see why. Write `background` to start a task that keeps running after your function returns. That is the closest match to `go`.

Channels carry values from one task to another, as in Go. You create one with `Channel<T>()`, where the number is how many values it holds at once, and you use `.send()` and `.receive()` in place of `<-`. `.receive()` gives a `Maybe`, because a channel can be closed. Each `.send()` and `.receive()` goes on its own line.

Go:

```go
package main

import "fmt"

func relayScores(wire chan int) {
	wire <- 2
	wire <- 5
	wire <- 3
}

func main() {
	wire := make(chan int, 1)
	go relayScores(wire)
	total := 0
	for i := 0; i < 3; i++ {
		total += <-wire
	}
	fmt.Println("runs this game:", total) // runs this game: 10
}
```

Yinz:

```
function relayScores(lend wire: Channel<Int>) -> Nothing {
  wire.send(2)
  wire.send(5)
  wire.send(3)
}

function entrypoint() -> Nothing {
  let wire: Channel<Int> = Channel<Int>(1)
  background relayScores(wire)
  let first = wire.receive()
  let firstScore = first.or(0)
  let second = wire.receive()
  let secondScore = second.or(0)
  let third = wire.receive()
  let thirdScore = third.or(0)
  let total = firstScore + secondScore + thirdScore
  print(`runs this game: ${total}`)    // runs this game: 10
}
```

The Go words are not valid. `go relayScores(wire)` gives only general errors, so remember the rule: write `background relayScores(wire)`. `wire <- 2` gives a misleading error, because the compiler reads `<` as a comparison and says it cannot be used with a Channel. Write `wire.send(2)` instead.

### Copies: `let b = a` hands the value over, and `.copy()` replaces sharing by accident

In Go, `b := a` copies a struct or an array, but it only copies the reference for a slice, a map or a pointer, so both names see each other's changes. Which one you get depends on the type. In Yinz, the rule is one rule for every type: storing a value in another name, an array or a shape hands it over, and the old name cannot be read afterward. Asking for a second, separate value is `.copy()`.

Go:

```go
package main

import "fmt"

func main() {
	scores := []int{70, 85, 92}
	backup := scores
	backup[0] = 0
	fmt.Println(scores[0]) // 0: both names use the same items
}
```

Yinz:

```
let scores: Array<Int> = [70, 85, 92]
let backup: Array<Int> = scores
let total = scores.count()
// COMPILE ERROR: `scores` was stored in `backup` and cannot be used here — storing a value hands it over.
// If you still need `scores` after the store, store a copy instead: `let backup = scores.copy()`. If you only need it before the store, put this line above the store.
```

Ask for a second value with `.copy()`. Go's `slices.Clone(scores)` is the match:

```
let scores: Array<Int> = [70, 85, 92]
let backup = scores.copy()
backup.add(100)
let total = scores.count()
print(total)    // 3
```

`slices.Clone` does not exist in Yinz. It gives only general errors, so remember the rule: write `scores.copy()`.

Passing a value to a function is not a store: the signature decides, with `share`, `lend` or `give` as the Functions trap shows. The same hand-over rule applies when you store a shape value in an array. In Go, `append(path, start)` copies `start`, and a later change to `start` does not reach the slice. In Yinz, you ask for the copy:

```
shape Position {
  x: Int
  y: Int
}

let path: Array<Position> = []
let start: Position = { x: 1, y: 2 }
path.add(start.copy())
start.x = 99
```

To change a stored element, write through the collection: `path[0] = { x: 99, y: 2 }`.

## Where to go next

Each topic has a full reference page in [`docs/reference/`](../reference/REF-language-overview.md). Start with [Variables](../reference/REF-variables.md), [Maybe Types](../reference/REF-maybe.md), [Errors](../reference/REF-errors.md) and [Concurrency](../reference/REF-concurrency.md).
