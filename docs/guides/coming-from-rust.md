---
name: "coming-from-rust"
description: "A guide for Rust developers: what you write in Rust, what you write in Yinz, and the habits the compiler will correct."
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

# Coming from Rust

If you write Rust, Yinz will feel like a close cousin. There is no garbage collector, every value has one owner, a stored value cannot be used again after it is handed over, a missing value is a type (`Maybe`), a failure is part of a function's signature (`errors`), and the compiler tells you when you forget a case. What changes is the spelling and the amount you write. There is no `&` or `&mut`, no `mut`, no lifetime annotations, no `struct` or `impl` block, no `match`, no macros such as `println!`, and no `mod` or `use`. This guide lists the differences, shows both sides, and shows the compiler message you get when an old habit slips in.

The compiler teaches as you go. Every message below says what is wrong, what to write instead, and why.

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

## In Rust you write → In Yinz you write

| Concept | In Rust you write | In Yinz you write |
|---|---|---|
| Variables | `let health = 100;` can never change<br>`let mut health = 100;` can change<br>`const MAX_PLAYERS: u32 = 10;` | `const health = 100` can never change<br>`let health = 100` can change<br>`const maxPlayers = 10` |
| Text | `"Hi"`, `String::from("Hi")`<br>`format!("Hi {name}")` | `` `Hi ${name}` `` (backticks only) |
| Comments and line endings | `// note`, `/// doc comment`, `/* note */`<br>`;` ends a statement | `// note`, `/// doc comment`, no `;` |
| Data structures | `struct Player { name: String }`<br>`impl Player { }` | `shape Player { name: String }` |
| Functions | `fn heal(&mut self, amount: i32) { }`<br>`fn show(&self) -> String { }` | `function heal(lend self: Player, amount: Int) -> Nothing { }`<br>`function show(share self: Player) -> String { }` |
| Missing values | `Option<String>`, `None`, `Some(name)`<br>`nickname.unwrap()` | `Maybe<String>`, `none`<br>`nickname.value` after `if (nickname.exists()) { }` |
| Failures | `Result<i32, String>`, `Err(...)`, `?`<br>`panic!("...")` | `function load() -> String errors { }`<br>`content.failed()`, `content.or(...)` |
| Choosing between kinds of values | `enum Shape { Circle { radius: i32 }, Square { side: i32 } }`<br>`match shape { Shape::Circle { .. } => ... }` | `shape Shape = Circle \| Square`<br>`if (shape) { is Circle => ... }` |
| Named value sets | `enum Status { Active, Banned }` | `options Status { active, banned }` |
| Collections and reading by position | `Vec<i32>`, `[i32; 3]`, `HashMap<String, i32>`<br>`scores[5]` panics; `scores.get(5)` is an `Option` | `Array<Int>`, `Fixed<Int>`, `Map<String, Int>`<br>`scores[5]` is a `Maybe<Int>` |
| Numbers | `i32`, `i64`, `u8`, `usize`, `f64`<br>`0.1 + 0.2` is `0.30000000000000004` | `Int`, `Number`, `Float`<br>`0.1 + 0.2` is `0.3` |
| Operators | `&&`, `\|\|`, `!`, `==`<br>`score += 5` | `&&`, `\|\|`, `!`, `==`<br>`score = score + 5` |
| Branching | `if { } else if { } else { }`<br>`match code { 200 => ..., _ => ... }` | early `return`<br>`if (code) { 200 => ... else => ... }` |
| Chaining | `players.iter().filter(f).map(g).collect()` | a `for` loop, one named step per line |
| Files and imports | `mod services;`<br>`use crate::services::users::fetch_user;`<br>`pub fn`, `fn main()` | `` import { fetchUser } from `services/users` ``<br>`export function`<br>`function entrypoint()` |
| Slow work | `async fn`, `.await`, an async runtime such as `tokio`<br>`tokio::join!(...)` | plain `function`; `wait` and `background` only when you need them |
| Copies | `let b = a;` moves the value<br>`a.clone()` | `let b = a` hands the value over<br>`a.copy()` |

## Traps

### Variables: `let` can change, and `const` never changes

In Rust, `let` makes a name that cannot change, and you add `mut` to allow it. In Yinz it is the other way round: `let` makes a name that can change, and `const` makes one that never changes. So Rust's plain `let` is Yinz's `const`, and Rust's `let mut` is Yinz's `let`.

Rust:

```rust
fn main() {
    let health = 100;
    health = 50;
}
// error[E0384]: cannot assign twice to immutable variable `health`
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

There is no `mut`. `let` is already changeable; `const` is the fixed one.

A `const` value cannot change at all, fields included, as in Rust when you leave `mut` off a whole struct value. Use `const` by default and `let` when the value needs to change.

A capital letter always means a type, so every name that is not a type is camelCase, constants included. Rust's `MAX_PLAYERS` becomes `maxPlayers`, and `fetch_user` becomes `fetchUser`.

### Text: backticks only, and `${}` replaces `format!`

Rust has `&str` (text written in the code), `String` (text you can grow), and `format!` to build text. Yinz has one text type, `String`, and one way to write it: backticks. They support `${}` and run over several lines.

```
let greeting = "Hello"
// COMPILE ERROR: Double-quoted strings don't exist in Yinz.
// Use backtick strings instead: `` `hello` `` or `` `Hello ${name}` ``
```

Put what you would write inside `format!` into backticks instead:

```
let name = `Alice`
let greeting = `Hello ${name}`
print(greeting)    // Hello Alice
```

`println!` is a macro in Rust. In Yinz, `print` is a normal function that takes exactly one value:

```
print("Health: {health}")
// COMPILE ERROR: Double-quoted strings don't exist in Yinz.
// Use backtick strings instead: `` `hello` `` or `` `Hello ${name}` ``
```

Write ``print(`Health: ${health}`)`` instead.

### Comments and line endings: `//`, `///`, and no `;`

Comments work as in Rust: `//` for a note and `///` for a doc comment placed directly above a declaration. A doc comment shows in the editor when you hover over the name. There is no `/* */` block comment. In Yinz, `//!` is just a plain comment (Rust uses it for inner doc comments); doc comments are `///`.

```
/// Adds `amount` to the player's health.
function heal(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health + amount
}
```

In Rust, `;` ends a statement. In Yinz, a newline ends a statement, and a `;` is an error:

```
let health = 100;
// COMPILE ERROR: Semicolons are not used in Yinz.
// Remove the `;` — Yinz uses newlines to end statements.
```

Rust lets the last line of a function be its result when it has no `;`. In Yinz you write `return value`.

### Data structures: `shape` holds the fields, and there is no `struct` or `impl`

A `shape` lists fields and nothing else, one per line. Behavior goes in standalone functions, which the next trap shows. There is no `impl` block and no `new`: you build a value by writing its fields, and the type goes after the name.

Rust:

```rust
struct Player {
    name: String,
    health: i32,
}

fn main() {
    let player = Player { name: String::from("Alice"), health: 50 };
    println!("{}", player.name);
}
```

Yinz:

```
shape Player {
  name: String
  health: Int
}

let player: Player = { name: `Alice`, health: 50 }
```

Neither Rust form is valid in Yinz. Writing `struct` gives errors, and among them you also get this warning:

```
struct Player {
// COMPILE WARNING: `struct` is not a keyword in Yinz.
// `struct` not valid here
```

Its hint teaches the fix: use `shape` to declare a data type: `shape Player { name: String, health: Int }`.

An `impl Player {` line gives only generic errors, so remember the rule instead: there is no `impl` block, and functions stand alone, as the next trap shows.

### Functions: `share` and `lend` replace `&` and `&mut`

A parameter needs a type, and the function needs a return type. `-> Nothing` is Rust's `()`: it returns no value. Inside the body you write no types.

How a function uses a value is written in front of the parameter, in the signature only:

| Rust | Yinz | The function |
|---|---|---|
| `player: &Player` or `&self` | `share player: Player` | only reads the value |
| `player: &mut Player` or `&mut self` | `lend player: Player` | changes the value |
| `player: Player` or `self` | `give player: Player` | takes the value for good |

Nothing is written at the call. Rust's `heal(&mut player, 20)` becomes `heal(player, 20)`, and your editor shows a muted note at the call saying which of the three the function uses. There are no lifetime annotations such as `<'a>` to write either.

A function has no `impl` block to live in. It stands alone, and `player.heal(20)` is shorthand for `heal(player, 20)`. The first parameter is named `self` by habit, and you write it yourself.

Rust:

```rust
struct Player {
    name: String,
    health: i32,
}

impl Player {
    fn heal(&mut self, amount: i32) {
        self.health += amount;
    }
}

fn main() {
    let mut player = Player { name: String::from("Alice"), health: 50 };
    player.heal(20);
    println!("{} has {}", player.name, player.health);    // Alice has 70
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

`&` and `&mut` in a signature are not valid. The `&mut` gives only generic errors, so remember the rule instead: write `lend` in front of the parameter, with no `&`. The `i32` in the same line does get a message of its own:

```
function heal(player: &mut Player, amount: i32) -> Nothing {
// COMPILE WARNING: `i32` is not a keyword in Yinz.
// Use `Int` for whole numbers.
```

Rust traits (`trait`, `impl Trait for`) are a separate topic in Yinz, called contracts and written with `follows`. This guide does not cover them.

### Missing values: `Maybe<T>` and `none` replace `Option<T>`

A value that might be missing has the type `Maybe<T>`. It is either a `T` or `none`. There is no `Some(...)` to wrap a present value. As in Rust, you cannot use it until you handle the `none` case. Unlike Rust, there is no `.unwrap()` that compiles and then panics: reading `.value` without a check is a compile error.

Rust:

```rust
fn main() {
    let nickname: Option<String> = None;
    if let Some(name) = &nickname {
        println!("{name}");
    }
    let display = nickname.unwrap_or(String::from("Anonymous"));
    println!("{display}");    // Anonymous
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

`if let Some(name) = ...` becomes `if (nickname.exists())`, and inside the block `nickname.value` is safe. `unwrap_or(x)` becomes `.or(x)`.

Skipping the check does not compile:

```
let nickname: Maybe<String> = none
print(nickname.value)
// COMPILE ERROR: `Maybe.value` requires you to first check `m.exists()`.
// Add a check: `if (m.exists()) { print(m.value) }`. Or use a default: `m.or(0)`.
```

`.unwrap()` does not exist:

```
let name = nickname.unwrap()
// COMPILE ERROR: `Maybe<String>` does not have a method called `unwrap`.
// Available methods: exists(), or(default).
```

`Maybe` means "no value" is a normal outcome. When the caller must deal with a failure, use `errors`.

### Failures: `errors` replaces `Result`, `?` and `panic!`

A function that can fail says so in its signature with the word `errors`. There is no error type to name and no `Ok` or `Err` to wrap. The caller supplies a fallback with `.or(...)`, checks with `.failed()`, or passes the failure up by marking its own function `errors` too. Using the result of an `errors` function without handling it is a compile error.

Rust:

```rust
fn batting_average(hits: i32, at_bats: i32) -> Result<i32, String> {
    if at_bats == 0 {
        return Err(String::from("no at-bats"));
    }
    Ok(hits * 1000 / at_bats)
}

fn main() {
    let average = batting_average(127, 400).unwrap_or(0);
    println!("{average}");    // 317
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

The Rust version returns `Err` when `at_bats` is zero, so `unwrap_or(0)` gives 0 there. Yinz does not work that way. Dividing by zero is not a failure you can catch: it stops the program with `RUNTIME ERROR: division by zero (Int)`. Check the number before you divide.

To look at the failure yourself, use `.failed()`:

```
let checked = battingAverage(50, 200)
if (checked.failed()) {
  print(`battingAverage failed`)
}
```

Rust's `Result<i32, String>` and `?` are not valid in Yinz:

```
function report(hits: Int, atBats: Int) -> Result<String, String> {
  let average = battingAverage(hits, atBats)?
  return `average: ${average}`
}
// COMPILE ERROR: `Result` is not a known type.
```

The compiler also points at the `errors` keyword. To pass a failure up, write `errors` on the calling function instead of a `?` at the call.

A failure left unhandled is a compile error, so it cannot slip through:

```
let attempt = battingAverage(127, 400)
print(attempt)
// COMPILE ERROR: This function can fail, but the failure is not handled here.
// Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.
```

`errors` is for failures the caller can expect, such as a file that is not there. A bug, such as an overflowing `Int`, stops the whole program. You do not write `panic!` for it: `panic!(...)` gives only generic errors. An overflowing `Int` stops the program with a message like this, and the program exits with code 134:

```
RUNTIME ERROR: Int overflow in '*' at …
```

### Choosing between kinds of values: a union of shapes replaces `enum` with data

In Rust, an `enum` variant can carry data, and `match` picks it apart. In Yinz, each kind is its own `shape`, and a union joins them with `|`. You check the kind with `is` inside an `if` with one arm per kind. Inside an arm, the value already has the right kind, so you read its fields directly.

Rust:

```rust
enum Shape {
    Circle { radius: i32 },
    Square { side: i32 },
    Triangle { base_length: i32, height: i32 },
}

fn describe(outline: &Shape) {
    match outline {
        Shape::Circle { radius } => println!("circle, radius {radius}"),
        Shape::Square { .. } => println!("square"),
        Shape::Triangle { .. } => println!("triangle"),
    }
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
```

As in Rust, a forgotten kind is an error, and the compiler names it:

```
if (outline) {
  is Circle => { print(`circle`) }
  is Square => { print(`square`) }
}
// COMPILE ERROR: Non-exhaustive union multi-case — 1 variant is not handled: Triangle.
// Add the missing arms (e.g. `is Triangle =>`) or add an `else =>` default arm.
```

`match` and `enum Shape { Circle { radius: Int } }` are not valid. Write unions with `|`; words such as `or` do not work. Writing `match` gives errors, and among them you also get this warning:

```
match outline {
// COMPILE WARNING: `match` is not a keyword in Yinz.
// Use multi-case `if` with `=>` arms: `if (value) { 1 => ...; 2 => ...; else => ... }`
```

### Named value sets: `options` replaces `enum` without data

An `options` declaration lists the allowed names, as a Rust `enum` with no data does. Names are camelCase, and you pick one with `Status.active`, where Rust writes `Status::Active`. You write no `#[derive(...)]` line.

Rust:

```rust
enum Status {
    Active,
    Banned,
}

fn main() {
    let status = Status::Active;
    match status {
        Status::Active => println!("online"),
        Status::Banned => println!("banned"),
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

This prints `online`. The compiler checks that your `if` handles every name in the set, as `match` does.

Writing `enum` gives errors, and among them you also get this warning:

```
enum Status {
// COMPILE WARNING: `enum` is not a keyword in Yinz.
// Use `options` to declare a set of named values: `options Status { active, inactive, banned }`
```

### Collections and reading by position: reading never panics

Every Yinz type starts with a capital letter, built-in collections included:

| Rust | Yinz |
|---|---|
| `Vec<i32>`, grows | `Array<Int>`, grows |
| `[i32; 3]`, size fixed, on the stack | `Fixed<Int>`, size set at creation |
| `HashMap<String, i32>` | `Map<String, Int>` |

In Rust, `scores[5]` on a list with three items panics, and you reach for `.get(5)` to get an `Option`. In Yinz, reading by position or by key gives a `Maybe`, so the plain `[ ]` is already the safe form:

```rust
fn main() {
    let scores = vec![70, 85, 92];
    let found = scores.get(5);              // Option<&i32>
    let score = found.copied().unwrap_or(0);
    println!("{score}");                    // 0
}
```

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

The everyday functions have new names:

| Rust | Yinz |
|---|---|
| `scores.len()` | `scores.count()` |
| `scores.push(88)` | `scores.add(88)` |
| `scores.contains(&88)` | `scores.contains(88)` |
| `scores.last()`, an `Option` | `scores.last()`, a `Maybe` |

Rust's collection names are not valid:

```
let scores: Vec<Int> = [70, 85, 92]
// COMPILE ERROR: `Vec` is not a known type.
```

Use `Array<Int>` instead.

A value written with `{ ... }` is a single shape value, not a collection. Use `[...]` to make one:

```
let team: Array<Player> = { name: `Alice`, health: 100 }
// COMPILE ERROR: `{ ... }` creates a single `Player` value, not an `Array<Player>`.
// Put it inside `[...]` to make an array: `[{ ... }]`
```

Writing past the end by position stops the whole program with no message, so use `.add` to grow an `Array`.

### Numbers: one `Int`, an exact `Number`, and overflow stops the program

Rust makes you choose a size and sign: `i8`, `i32`, `u64`, `usize`. In Yinz, `Int` is the whole number type for everyday use, and it holds about ±9.2 × 10^18. A result that does not fit stops the whole program with `RUNTIME ERROR: Int overflow in '<op>' at …`. In Rust, the same mistake panics in a debug build and silently wraps around in a release build:

```rust
fn double(score: i32) -> i32 {
    score * 2
}

fn main() {
    println!("{}", double(i32::MAX));
    // debug build: panics with "attempt to multiply with overflow"
    // release build: prints -2
}
```

A size such as `i32` is not a Yinz type. You get an error, and among the messages this warning:

```
let health: i32 = 100
// COMPILE ERROR: `i32` is not a known type.
// COMPILE WARNING: `i32` is not a keyword in Yinz.
// Use `Int` for whole numbers.
```

Whole-number division works as in Rust: `/` on two `Int` values gives an `Int`, and the decimals are cut off. `//` starts a comment, so it is not an operator:

```
let half = 7 / 2
print(half)    // 3
```

For a result with decimals, use decimal numbers: `7.0 / 2.0`.

Rust's `f64` gives `0.30000000000000004` for `0.1 + 0.2`. Yinz has two decimal types. `Number` stores decimal digits exactly, so you get the answer you expect. `Float` is fast binary arithmetic like `f64`, with the same tiny rounding differences:

```rust
fn main() {
    let total = 0.1 + 0.2;
    println!("{total}");    // 0.30000000000000004
}
```

```
let total = 0.1 + 0.2
print(total)    // 0.3
```

Use `Int` for whole numbers, `Number` for anything where the exact value matters, and `Float` only when you want speed and can accept rounding differences.

### Operators: `&&`, `||`, `!` and `==` match Rust, and `+=` does not exist

The logic operators are the same as in Rust:

```
function canEnter(score: Int, banned: Boolean, isAdmin: Boolean) -> Boolean {
  if (isAdmin) {
    return true
  }
  return score >= 10 && !banned
}
```

A single `|` is for unions, not for "or".

As in Rust, `==` never converts types silently, and comparing different types is an error:

```
let input: String = `5`
let count: Int = 5
if (input == count) { print(1) }
// COMPILE ERROR: `==` cannot be used with `String` and `Int`.
// Check that both sides have the same type.
```

Rust's `input.parse::<i32>()` gives a `Result`. In Yinz, `.toInt()` gives a `Maybe<Int>`:

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

### Branching: no `else`, no `match`

In Rust, `if` and `match` are expressions that give a value. In Yinz, handle one case with an early `return`, then carry on. This replaces an `if` / `else if` / `else` chain. The condition always has parentheses, and the block always has braces.

Rust:

```rust
fn describe(health: i32) -> String {
    if health <= 0 {
        String::from("dead")
    } else if health < 30 {
        String::from("in danger")
    } else {
        String::from("healthy")
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

When you cannot return, set a value first and change it. This replaces `let message = if health > 50 { ... } else { ... };`:

```
let message = `in danger`
if (health > 50) {
  message = `healthy`
}
print(message)
```

To compare one value against several cases, use a multi-case `if`. `else =>` is the catch-all, like Rust's `_ =>`:

```rust
fn describe_status(status_code: i32) {
    match status_code {
        200 => println!("ok"),
        404 => println!("not found"),
        other => println!("unexpected: {other}"),
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

The catch-all arm does not give the value a new name, so `other` becomes the `statusCode` you already have.

Writing `match` gives errors, and among them you also get this warning:

```
match (statusCode) {
// COMPILE WARNING: `match` is not a keyword in Yinz.
// Use multi-case `if` with `=>` arms: `if (value) { 1 => ...; 2 => ...; else => ... }`
```

### Chaining: an iterator chain becomes named steps

Yinz has no chaining. Each step goes on its own line with a name, so you can inspect every value. There is no `.iter()` to start and no `.collect()` to finish: a `for` loop builds the result one item at a time.

Rust:

```rust
struct Player {
    name: String,
    health: i32,
}

fn main() {
    let players = vec![
        Player { name: String::from("Alice"), health: 100 },
        Player { name: String::from("Bob"), health: 0 },
        Player { name: String::from("Cara"), health: 80 },
    ];
    let active_names: Vec<String> = players
        .iter()
        .filter(|player| player.health > 0)
        .map(|player| player.name.clone())
        .collect();
    println!("{}", active_names.len());    // 2
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

### Files and imports: no `mod` and no `use`; paths start at the project root

In Rust, a `mod services;` line adds a file to the program, and `use` brings a name into scope. In Yinz there is no `mod`: a file is found by its path. A single `import` brings in the names you list. Import paths start at your project root and are written in backticks. A leading `./` or `../` is an error:

```
import { fetchUser } from `../services/users`
// COMPILE ERROR: Import paths must be project-root relative, not relative paths.
// Write `services/users` instead of `../services/users`.
```

The correct form, with the exporting file above it:

```
// services/users.ynz
export function fetchUser(userId: Int) -> String {
  return `user ${userId}`
}
```

```
// entrypoint.ynz
import { fetchUser } from `services/users`
```

As in Rust, names are private unless you mark them. Rust's `pub` is `export`, and there are no default exports.

Every Yinz program starts at `function entrypoint()`, where Rust has `fn main()`:

```
function entrypoint() -> Nothing {
  print(`hello, yinz`)
}
```

Rust's `mod` and `use` lines are not valid. They give only generic errors, so remember the rule instead: there is no `mod`, and you write `` import { fetchUser } from `services/users` `` where Rust has `use`.

Two imports cannot use the same name. Yinz stops you and asks you to rename one:

```
import { Player } from `models/game`
import { Player } from `external/legacy`
// COMPILE ERROR: Import name `Player` is already bound by a previous import.
// Use a different local name: `import { Player as otherName } from "..."`
```

### Slow work: no `async fn`, no `.await`, no runtime to pick

In Rust, `async fn` makes a function that gives back a future, `.await` runs it, and the standard library has no runtime, so you add a crate such as `tokio` and mark `main`. In Yinz, you mark nothing. When a call has to pause, such as a network call, the compiler pauses your function by itself. Independent calls run at the same time without extra code, so there is no `tokio::join!`.

Rust (with the `tokio` crate, using its `macros`, `rt-multi-thread` and `time` features):

```rust
use std::time::Duration;
use tokio::time::sleep;

async fn fetch_lower_bowl_price() -> f64 {
    sleep(Duration::from_millis(60)).await;
    24.50
}

async fn fetch_upper_deck_price() -> f64 {
    sleep(Duration::from_millis(60)).await;
    31.75
}

#[tokio::main]
async fn main() {
    let (lower_bowl, upper_deck) =
        tokio::join!(fetch_lower_bowl_price(), fetch_upper_deck_price());
    println!("lower bowl ticket: {lower_bowl}");
    println!("upper deck ticket: {upper_deck}");
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

The two fetches have nothing to do with each other, so they overlap: the wait is about 60 milliseconds, not 120. The `sleep` takes milliseconds, as `Duration::from_millis` does.

Your editor shows a muted `wait` hint where a call pauses. Write `wait` yourself only when two calls must finish in a set order and the compiler cannot see why. Write `background` to start a task that keeps running after your function returns, which is the closest match to `tokio::spawn`.

Writing the Rust words gives errors, and among them you also get these warnings:

```
async fn fetchPrice() -> Number {
// COMPILE WARNING: `async` is not a keyword in Yinz.
// `async` not valid here
```

Write `wait foo()` at the call site — no declaration-level annotation needed.

```
let price = fetchPrice().await
// COMPILE WARNING: `await` is not a keyword in Yinz.
// Write `wait foo()` at the call site.
```

The prefix form `await fetchPrice()` is also not Yinz:

```
let price = await fetchPrice()
// COMPILE WARNING: `await` is not a keyword in Yinz.
// Write `wait foo()` at the call site.
```

### Copies: `let b = a` hands the value over, and `.copy()` replaces `.clone()`

This is the part of Rust you already know. Storing a value in another name, an array or a shape hands it over, and the old name cannot be read afterward. Rust calls it a move. Asking for a second value is `.copy()`, where Rust has `.clone()`.

Rust:

```rust
fn main() {
    let scores = vec![70, 85, 92];
    let backup = scores;
    let total = scores.len();
    // error[E0382]: borrow of moved value: `scores`
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

Ask for a second value with `.copy()`:

```
let scores: Array<Int> = [70, 85, 92]
let backup = scores.copy()
backup.add(100)
let total = scores.count()
print(total)    // 3
```

`.clone()` does not exist:

```
let backup = scores.clone()
// COMPILE ERROR: `Array<Int>` does not have a method called `clone`.
```

Use `.copy()` instead.

Passing a value to a function is not a store: the signature decides, with `share`, `lend` or `give` as the Functions trap shows. The same hand-over rule applies when you store a shape value in an array:

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

Each topic has a full reference page in [`docs/reference/`](../reference/REF-language-overview.md). Start with [Variables](../reference/REF-variables.md), [Maybe Types](../reference/REF-maybe.md) and [Errors](../reference/REF-errors.md).
