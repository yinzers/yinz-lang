---
name: "coming-from-cpp"
description: "A guide for C++ developers: what you write in C++, what you write in Yinz, and the habits the compiler will correct."
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

# Coming from C++

If you write C++, you already think about who owns a value, when it is copied and how fast the result runs. Yinz keeps that: there is no garbage collector, every value has one owner, and the compiler produces fast code. What changes is how much you write and how much the compiler checks for you. There is no `auto`, no `class`, no `virtual`, no `nullptr`, no `throw`, no `std::variant`, no `switch`, no `else`, no `#include`, and no header files. Mistakes that C++ leaves as undefined behavior, such as reading past the end of a `std::vector`, are either compile errors or errors that stop the whole program. This guide lists the differences, shows both sides, and shows the compiler message you get when an old habit slips in.

The compiler teaches as you go. Every message below says what is wrong, what to write instead, and why.

A note on the examples: C++ examples are C++20 and do not show the C++ compiler's messages; a comment says what C++ does.

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

## In C++ you write → In Yinz you write

| Concept | In C++ you write | In Yinz you write |
|---|---|---|
| Variables | `auto health = 100;` can change<br>`int health = 100;` can change<br>`const int maxPlayers = 10;` can never change | `let health = 100` can change<br>`const health = 100` can never change<br>`const maxPlayers = 10` |
| Text | `"Hi"` (a `const char*`), `std::string`<br>`std::format("Hi {}", name)` | `` `Hi ${name}` `` (backticks only) |
| Comments and line endings | `// note`, `/* note */`<br>`;` ends a statement | `// note`, `/// doc comment`, no `;` |
| Data structures | `struct Player { std::string name; };`<br>`class Player { public: void heal(int amount); };` | `shape Player { name: String }` |
| Functions | `void heal(Player& player, int amount) { }`<br>`std::string show(const Player& player) { }` | `function heal(lend self: Player, amount: Int) -> Nothing { }`<br>`function show(share self: Player) -> String { }` |
| Missing values | `nullptr`, `std::optional<std::string>`<br>`nickname.value_or("...")` | `Maybe<String>`, `none`<br>`nickname.value` after `if (nickname.exists()) { }` |
| Failures | `throw std::runtime_error("...")`<br>`try { } catch (const std::runtime_error&) { }` | `function load() -> String errors { }`<br>`content.failed()`, `content.or(...)` |
| Choosing between kinds of values | `std::variant<Circle, Square>`<br>`std::visit(visitor, shape)` | `shape Shape = Circle \| Square`<br>`if (shape) { is Circle => ... }` |
| Named value sets | `enum class Status { Active, Banned };` | `options Status { active, banned }` |
| Collections and reading by position | `std::vector<int>`, `std::array<int, 3>`, `std::map<std::string, int>`<br>`scores[5]` is undefined behavior; `scores.at(5)` throws | `Array<Int>`, `Fixed<Int>`, `Map<String, Int>`<br>`scores[5]` is a `Maybe<Int>` |
| Numbers | `int`, `long long`, `unsigned`, `double`<br>signed overflow is undefined behavior | `Int`, `Number`, `Float`<br>overflow stops the program |
| Operators | `&&`, `\|\|`, `!`, `==`<br>`score++`, `score += 5` | `&&`, `\|\|`, `!`, `==`<br>`score = score + 5` |
| Branching | `if { } else if { } else { }`<br>`switch (code) { case 200: ... break; default: ... }` | early `return`<br>`if (code) { 200 => ... else => ... }` |
| Chaining | `players \| std::views::filter(f) \| std::views::transform(g)` | a `for` loop, one named step per line |
| Files and imports | `#include "services/users.hpp"`<br>a header (`.hpp`) plus a source file (`.cpp`), `namespace`<br>`int main()` | `` import { fetchUser } from `services/users` ``<br>`export function`, one file<br>`function entrypoint()` |
| Slow work | `std::thread`, `std::async`, `std::future`<br>`std::mutex`, `join()` | plain `function`; `wait` and `background` only when you need them<br>`Channel<Int>` |
| Copies | `b = a` copies the whole value<br>`std::move(a)` leaves `a` in an unspecified state | `let b = a` hands the value over<br>`a.copy()` |

## Traps

### Variables: `let` can change, and `const` never changes

In C++, `auto health = 100;` makes a name that can change, and `const` in front of the type makes one that cannot. In Yinz, `let` makes a name that can change, and `const` makes one that never changes. Use `const` by default and `let` when the value needs to change.

A `const` value cannot change at all, fields included, as with `const Player player{...}` in C++.

C++:

```cpp
int main() {
    const int maxPlayers = 10;
    maxPlayers = 20; // does not compile: assignment to a const variable
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

`auto` and `int` do not exist as words in Yinz, and the compiler gives only general errors for them, so remember the rule instead: write `let health = 100`. Inside a function, `let` and `const` already work out the type, the way `auto` does. Write a type when you want one, after a colon: `let health: Int = 100`.

A capital letter always means a type, so every name that is not a type is camelCase. In C++, built-in types are lowercase (`int`, `double`) and constants are often `kMaxPlayers` or `MAX_PLAYERS`. In Yinz, `Int` is a type and `maxPlayers` is a constant.

### Text: backticks only, and `${}` replaces `std::string` and `std::format`

In C++, `"Hi"` is not a `std::string`. It is an array of characters that you convert to a `std::string` when you need to grow it, join it or compare it. In Yinz, there is one text type, `String`, and one way to write it: backticks. They put values in with `${}` and run over several lines.

C++:

```cpp
#include <format>
#include <iostream>
#include <string>

int main() {
    std::string name = "Alice";
    std::string greeting = std::format("Hello {}", name);
    std::cout << greeting << '\n'; // Hello Alice
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

There is no `std::string`, no `std::format` and no `{}` placeholder. The compiler gives only general errors for `std::format`, so remember the rule: put the value inside the text with `${}`, as in `` `Hello ${name}` ``.

`std::cout << a << b` joins pieces with `<<`. In Yinz, `print` is a function that takes exactly one value, so build the text first:

```
print(`Health:`, health)
// COMPILE ERROR: `print` takes 1 argument, but 2 were given.
// Call it with one value: `print(value)`
```

Write ``print(`Health: ${health}`)`` instead.

### Comments and line endings: `//`, `///`, and no `;`

`//` works as in C++. A doc comment is written with `///` directly above a declaration, and it shows in the editor when you hover over the name. There is no `/* */` block comment.

```
/// Adds `amount` to the player's health.
function heal(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health + amount
}
```

In C++, `;` ends a statement, and a class needs one after its closing brace. In Yinz, a newline ends a statement, and a `;` is an error:

```
let health = 100;
// COMPILE ERROR: Semicolons are not used in Yinz.
// Remove the `;` — Yinz uses newlines to end statements.
```

A function gives back a value with `return value`, as in C++.

### Data structures: `shape` holds the fields, and behavior goes in functions

A `shape` lists fields and nothing else, one per line. There is no `class`, no `public:` or `private:`, no constructor, no destructor, and no `virtual`. A method is a standalone function, and `player.heal(20)` is shorthand for `heal(player, 20)`, so you still get autocomplete after the dot.

You build a value by writing its fields. The type goes after the name, and field names are camelCase.

C++:

```cpp
#include <iostream>
#include <string>

class Player {
public:
    std::string name;
    int health;

    void heal(int amount) {
        health += amount;
    }
};

int main() {
    Player player{"Alice", 50};
    player.heal(20);
    std::cout << player.name << " has " << player.health << '\n'; // Alice has 70
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

`class` is not valid in Yinz. Writing `class Player {` gives errors, and among them this warning:

```
class Player {
// COMPILE WARNING: `class` is not a keyword in Yinz.
// `class` not valid here
```

Its hint says: "Use `shape` for data and standalone functions for behavior".

A `virtual` function lets one call run different code depending on the value's real kind. Yinz picks the code when it compiles, unless you ask for the dynamic form with a contract. Contracts, written with `follows`, are a separate topic that this guide does not cover. For a value that is one of a few known kinds, use a union (see "Choosing between kinds of values"). The compiler gives only general errors for `virtual`, so remember the rule: leave it out.

### Functions: `share`, `lend` and `give` replace `const&`, `&` and by-value

A parameter needs a type, and the function needs a return type. `-> Nothing` is C++'s `void`. Inside the body you write no types.

In C++ you pick how a value is passed by how you write the type. In Yinz you write how the function uses the value, in front of the parameter, in the signature only:

| C++ | Yinz | The function |
|---|---|---|
| `const Player& player` | `share player: Player` | only reads the value |
| `Player& player` | `lend player: Player` | changes the value |
| `Player player` plus `std::move(...)` at the call | `give player: Player` | takes the value for good |

Nothing is written at the call. As in C++, `heal(player, 20)` has no `&` at the call, and your editor shows a muted note there saying which of the three the function uses. There are no pointers to pass, so you write no `*` and no `->`.

C++:

```cpp
#include <iostream>
#include <string>

struct Player {
    std::string name;
    int health;
};

void heal(Player& player, int amount) {
    player.health += amount;
}

int main() {
    Player player{"Alice", 50};
    heal(player, 20);
    std::cout << player.name << " has " << player.health << '\n'; // Alice has 70
}
```

Yinz:

```
shape Player {
  name: String
  health: Int
}

function heal(lend player: Player, amount: Int) -> Nothing {
  player.health = player.health + amount
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 50 }
  heal(player, 20)
  print(player.health)    // 70
}
```

In C++, forgetting the `&` compiles and silently changes a copy:

```cpp
void heal(Player player, int amount) {
    player.health += amount; // changes a copy; the caller never sees it
}
```

In Yinz, the compiler stops you when a function that only reads tries to change a value:

```
function heal(share player: Player, amount: Int) -> Nothing {
  player.health = player.health + amount
}
// COMPILE ERROR: `player` is declared `share` — read-only — so its fields cannot be changed.
// Change the parameter to `lend player: Player` if this function needs to modify it.
```

`&` and `const&` in a signature, as in `Player&` or `const Player&`, are not valid. The compiler gives only general errors for them, so remember the rule: write `share`, `lend` or `give` in front of the parameter, and no `&`.

### Missing values: `Maybe<T>` and `none` replace `nullptr` and `std::optional`

A value that might be missing has the type `Maybe<T>`. It is either a `T` or `none`. You cannot use it until you handle the `none` case. In C++, `*nickname` on an empty `std::optional`, or `*pointer` on a `nullptr`, is undefined behavior, and `.value()` throws. In Yinz, reading without a check does not compile.

C++:

```cpp
#include <iostream>
#include <optional>
#include <string>

int main() {
    std::optional<std::string> nickname; // empty: no nickname
    if (nickname.has_value()) {
        std::cout << *nickname << '\n';
    }
    std::string display = nickname.value_or("Anonymous");
    std::cout << display << '\n'; // Anonymous
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

`has_value()` becomes `.exists()`, `*nickname` becomes `nickname.value` (safe inside the block), and `value_or(x)` becomes `.or(x)`.

Skipping the check does not compile:

```
let nickname: Maybe<String> = none
print(nickname.value)
// COMPILE ERROR: `Maybe.value` requires you to first check `m.exists()`.
// Add a check: `if (m.exists()) { print(m.value) }`. Or use a default: `m.or(0)`.
```

`nullptr` and `std::optional` do not exist. The compiler gives only general errors for them, so remember the rule: write `none`, as in `let nickname: Maybe<String> = none`.

`Maybe` means "no value" is a normal outcome. When the caller must deal with a failure, use `errors`.

### Failures: `errors` replaces `throw`, `try` and `catch`

A function that can fail says so in its signature with the word `errors`. In C++, a function does not say whether it throws, so a caller can forget to catch. In Yinz, the signature says it, and the caller supplies a fallback with `.or(...)`, checks with `.failed()`, or passes the failure up by marking its own function `errors` too. Using the result of an `errors` function without handling it is a compile error. There is no error type to name.

C++:

```cpp
#include <iostream>
#include <stdexcept>

int battingAverage(int hits, int atBats) {
    if (atBats == 0) {
        throw std::runtime_error("no at-bats");
    }
    return hits * 1000 / atBats;
}

int main() {
    int average = 0;
    try {
        average = battingAverage(127, 400);
    } catch (const std::runtime_error&) {
        average = 0;
    }
    std::cout << average << '\n'; // 317
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

The C++ version throws when `atBats` is zero, and the `catch` sets `average` to 0. Yinz does not work that way. Dividing by zero is not a failure you can catch: it stops the program with `RUNTIME ERROR: division by zero (Int)`. Check the number before you divide.

To look at the failure yourself, use `.failed()`:

```
let checked = battingAverage(50, 200)
if (checked.failed()) {
  print(`battingAverage failed`)
}
```

`throw`, `try` and `catch` are not valid. The compiler gives only general errors for them, so remember the rule: mark the function `errors`, and let the caller use `.or(...)` or `.failed()`. There is nothing to throw by hand.

A failure left unhandled is a compile error, so it cannot slip through:

```
let attempt = battingAverage(127, 400)
print(attempt)
// COMPILE ERROR: This function can fail, but the failure is not handled here.
// Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.
```

`errors` is for failures the caller can expect, such as a file that is not there. A bug, such as an overflowing `Int`, stops the whole program, and it never turns into undefined behavior.

### Choosing between kinds of values: a union of shapes replaces `std::variant` and `std::visit`

In C++, `std::variant<Circle, Square, Triangle>` holds one of several types, and `std::visit` runs one function for whichever it holds. The visitor is a struct with one `operator()` per kind, a function-call operator that lets the struct be called like a function. In Yinz, each kind is its own `shape`, and a union joins them with `|`. You check the kind with `is` inside an `if` with one arm per kind. Inside an arm, the value already has the right kind, so you read its fields directly.

C++:

```cpp
#include <iostream>
#include <variant>

struct Circle { int radius; };
struct Square { int side; };
struct Triangle { int baseLength; int height; };

using Shape = std::variant<Circle, Square, Triangle>;

struct Describer {
    void operator()(const Circle& circle) const {
        std::cout << "circle, radius " << circle.radius << '\n';
    }
    void operator()(const Square&) const {
        std::cout << "square\n";
    }
    void operator()(const Triangle&) const {
        std::cout << "triangle\n";
    }
};

void describe(const Shape& outline) {
    std::visit(Describer{}, outline);
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

As in C++, a forgotten kind is an error, and the Yinz compiler names it in a short message:

```
if (outline) {
  is Circle => { print(`circle`) }
  is Square => { print(`square`) }
}
// COMPILE ERROR: Non-exhaustive union multi-case — 1 variant is not handled: Triangle.
// Add the missing arms (e.g. `is Triangle =>`) or add an `else =>` default arm.
```

`std::variant` and `std::visit` do not exist. The compiler gives only general errors for them, so remember the rule: write a union with `|` and check the kind with `is`, as above.

### Named value sets: `options` replaces `enum class`

An `options` declaration lists the allowed names. Names are camelCase, and you pick one with `Status.active`, where C++ writes `Status::Active`. There is no underlying type to choose.

C++:

```cpp
#include <iostream>

enum class Status { Active, Banned };

int main() {
    Status status = Status::Active;
    switch (status) {
        case Status::Active:
            std::cout << "online\n";
            break;
        case Status::Banned:
            std::cout << "banned\n";
            break;
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

This prints `online`. In C++, `static_cast<Status>(7)` is a valid `Status`, and a `switch` that skips a name is only a compiler warning, if that. In Yinz, the compiler checks that your `if` handles every name in the set.

`enum` and `enum class` do not exist. Writing `enum Status {` gives errors, and among them this warning:

```
enum Status {
// COMPILE WARNING: `enum` is not a keyword in Yinz.
// Use `options` to declare a set of named values: `options Status { active, inactive, banned }`
```

Writing `enum class Status { Active, Banned };` gives the same `enum` warning and the `class` warning from the Data structures trap.

### Collections and reading by position: reading never gives undefined behavior

Every Yinz type starts with a capital letter, built-in collections included:

| C++ | Yinz |
|---|---|
| `std::vector<int>`, grows | `Array<Int>`, grows |
| `std::array<int, 3>`, size fixed, on the stack | `Fixed<Int>`, size set at creation |
| `std::map<std::string, int>` | `Map<String, Int>` |

In C++, `scores[5]` on a vector with three items is undefined behavior: it might crash, or it might read garbage and carry on. `scores.at(5)` throws instead. A map is worse: `hitLeaders["stargell"]` for a key that is not there adds the key with the value `0`, so reading changes the map. In Yinz, reading by position or by key gives a `Maybe`, so the plain `[ ]` is already the safe form, and reading never changes the collection.

C++:

```cpp
#include <iostream>
#include <map>
#include <string>
#include <vector>

int main() {
    std::vector<int> scores{70, 85, 92};
    int score = 0;
    if (scores.size() > 5) {
        score = scores[5]; // scores[5] without this check is undefined behavior
    }
    std::cout << score << '\n'; // 0

    std::map<std::string, int> hitLeaders{{"clemente", 3000}, {"wagner", 3420}};
    int hits = hitLeaders["stargell"]; // adds "stargell" with the value 0
    std::cout << hits << ' ' << hitLeaders.size() << '\n'; // 0 3
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

| C++ | Yinz |
|---|---|
| `scores.size()` | `scores.count()` |
| `scores.push_back(88)` | `scores.add(88)` |
| `std::ranges::find(scores, 88) != scores.end()` | `scores.contains(88)` |
| `scores.back()`, undefined behavior when empty | `scores.last()`, a `Maybe` |

C++'s collection names are not valid. `std::vector<Int>` gives only general errors, so remember the rule: write `Array<Int>`. A C++ method name gives an error that names the Yinz type:

```
let total = scores.size()
// COMPILE ERROR: `Array<Int>` does not have a method called `size`.
```

Use `scores.count()` instead.

A value written with `{ ... }` is a single shape value, not a collection. Use `[...]` to make one:

```
let team: Array<Player> = { name: `Alice`, health: 100 }
// COMPILE ERROR: `{ ... }` creates a single `Player` value, not an `Array<Player>`.
// Put it inside `[...]` to make an array: `[{ ... }]`
```

Writing past the end by position stops the whole program with no message, so use `.add` to grow an `Array`.

### Numbers: one `Int`, an exact `Number`, and overflow stops the program

C++ makes you choose a size and sign. `int` is at least 16 bits and is usually 32, `long long` is at least 64, and `unsigned` wraps around on overflow. In Yinz, `Int` is the whole number type for everyday use, and it holds about ±9.2 × 10^18. A result that does not fit stops the whole program with `RUNTIME ERROR: Int overflow in '<op>' at …`. In C++, overflow of a signed `int` is undefined behavior, so the compiler may assume it never happens:

```cpp
#include <climits>
#include <iostream>

int doubleScore(int score) {
    return score * 2;
}

int main() {
    std::cout << doubleScore(INT_MAX) << '\n'; // undefined behavior: signed overflow
}
```

A C++ size or sign is not a Yinz type:

```
let health: unsigned = 100
// COMPILE ERROR: `unsigned` is not a known type.
```

Use `Int`.

Whole-number division works as in C++: `/` on two `Int` values gives an `Int`, and the decimals are cut off. `//` starts a comment, so it is not an operator:

```
let half = 7 / 2
print(half)    // 3
```

For a result with decimals, use decimal numbers: `7.0 / 2.0`.

C++'s `double` is binary, so `0.1 + 0.2` is not exactly `0.3`. `std::cout` hides it by printing only six digits, and you see it when you ask for more. Yinz has two decimal types. `Number` stores decimal digits exactly, so you get the answer you expect. `Float` is fast binary arithmetic like `double`, with the same tiny rounding differences:

```cpp
#include <iomanip>
#include <iostream>

int main() {
    double a = 0.1;
    double b = 0.2;
    std::cout << a + b << '\n';                         // 0.3
    std::cout << std::setprecision(17) << a + b << '\n'; // 0.30000000000000004
}
```

```
let total = 0.1 + 0.2
print(total)    // 0.3
```

Use `Int` for whole numbers, `Number` for anything where the exact value matters, and `Float` only when you want speed and can accept rounding differences. C++'s `double` is not a Yinz type:

```
let price: double = 24.5
// COMPILE ERROR: `double` is not a known type.
```

Use `Float` or `Number`.

### Operators: `&&`, `||`, `!` and `==` match C++, and `++` and `+=` do not exist

The logic operators are the same as in C++:

```
function canEnter(score: Int, banned: Boolean, isAdmin: Boolean) -> Boolean {
  if (isAdmin) {
    return true
  }
  return score >= 10 && !banned
}
```

A single `|` is for unions, not for "or".

C++ converts types silently when it compares, so `5 == 5.0` is `true`, and comparing a signed number with an unsigned one can give a wrong answer. In Yinz, comparing different types is an error:

```
let input: String = `5`
let count: Int = 5
if (input == count) { print(1) }
// COMPILE ERROR: `==` cannot be used with `String` and `Int`.
// Check that both sides have the same type.
```

C++'s `std::stoi(input)` throws when the text is not a number. In Yinz, `.toInt()` gives a `Maybe<Int>`:

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

The same goes for `++score`.

### Branching: no `else`, no `switch`

In Yinz, handle one case with an early `return`, then carry on. This replaces an `if` / `else if` / `else` chain. As in C++, the condition has parentheses. Unlike C++, the block always has braces: a one-line `if (x) return y;` is not valid.

C++:

```cpp
#include <string>

std::string describe(int health) {
    if (health <= 0) {
        return "dead";
    } else if (health < 30) {
        return "in danger";
    } else {
        return "healthy";
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

When you cannot return, set a value first and change it. This replaces the `?:` operator, as in `health > 50 ? "healthy" : "in danger"`:

```
let message = `in danger`
if (health > 50) {
  message = `healthy`
}
print(message)
```

The `?:` operator gives a misleading error, because the compiler reads the `?` as a `Maybe` marker. Remember the rule: there is no `?:`, so use the `let` and `if` form above.

To compare one value against several cases, use a multi-case `if`. Each arm is its own block, so you write no `break`, and one arm never runs into the next. `else =>` is the catch-all, like C++'s `default:`:

```cpp
#include <iostream>

void describeStatus(int statusCode) {
    switch (statusCode) {
        case 200:
            std::cout << "ok\n";
            break;
        case 404:
            std::cout << "not found\n";
            break;
        default:
            std::cout << "unexpected: " << statusCode << '\n';
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
switch (statusCode) {
// COMPILE WARNING: `switch` is not a keyword in Yinz.
// Use multi-case `if` with `=>` arms: `if (value) { 1 => ...; 2 => ...; else => ... }`
```

### Chaining: a ranges pipeline becomes named steps

C++20 ranges join steps with `|`. Yinz has no chaining and no `|` pipeline: a single `|` is for unions. Each step goes on its own line with a name, so you can inspect every value. A `for` loop builds the result one item at a time.

C++:

```cpp
#include <iostream>
#include <ranges>
#include <string>
#include <vector>

struct Player {
    std::string name;
    int health;
};

int main() {
    std::vector<Player> players{{"Alice", 100}, {"Bob", 0}, {"Cara", 80}};
    auto active = players | std::views::filter([](const Player& player) { return player.health > 0; });
    auto names = active | std::views::transform([](const Player& player) { return player.name; });
    std::cout << std::ranges::distance(names) << '\n'; // 2
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

### Files and imports: no `#include` and no headers; paths start at the project root

In C++, `#include` pastes a header file into yours before compiling. A header holds the declarations, a `.cpp` file holds the code, and you keep the two in step by hand. In Yinz, there is no preprocessor, no header and no `#pragma once`: each file is one file, and a single `import` brings in the names you list. You call them without a prefix such as `services::`. Import paths start at your project root and are written in backticks. A leading `./` or `../` is an error:

```
import { fetchUser } from `../services/users`
// COMPILE ERROR: Import paths must be project-root relative, not relative paths.
// Write `services/users` instead of `../services/users`.
```

C++:

```cpp
// services/users.hpp
#pragma once
#include <string>

namespace services {
std::string fetchUser(int userId);
}
```

```cpp
// services/users.cpp
#include "services/users.hpp"

namespace services {
std::string fetchUser(int userId) {
    return "user " + std::to_string(userId);
}
}
```

```cpp
// main.cpp
#include <iostream>
#include "services/users.hpp"

int main() {
    std::cout << services::fetchUser(7) << '\n'; // user 7
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

Names are private unless you mark them with `export`. There are no default exports. Every Yinz program starts at `function entrypoint()`, where C++ has `int main()`.

`#include` and `namespace` are not valid. `#include` gives a misleading error, because the compiler reads the `#` as the start of a comment. Remember the rule: leave out the include, the header and the `namespace`, and let the file's path be its name.

Two imports cannot use the same name. Yinz stops you and asks you to rename one:

```
import { Player } from `models/game`
import { Player } from `external/legacy`
// COMPILE ERROR: Import name `Player` is already bound by a previous import.
// Use a different local name: `import { Player as otherName } from "..."`
```

### Slow work: no `std::thread` or `std::async`; the compiler overlaps independent calls

In C++, you start work on another thread with `std::thread` or `std::async`, collect the result from a `std::future`, and guard shared values with a `std::mutex`. Two threads that write the same value without a lock are undefined behavior. In Yinz, you mark nothing for slow calls. When a call has to pause, such as a network call, the compiler pauses your function by itself. Independent calls run at the same time without extra code, so there is no future to collect.

C++ (on Linux, build with `-pthread`):

```cpp
#include <chrono>
#include <future>
#include <iostream>
#include <thread>

double fetchLowerBowlPrice() {
    std::this_thread::sleep_for(std::chrono::milliseconds(60));
    return 24.50;
}

double fetchUpperDeckPrice() {
    std::this_thread::sleep_for(std::chrono::milliseconds(60));
    return 31.75;
}

int main() {
    std::future<double> lowerBowl = std::async(std::launch::async, fetchLowerBowlPrice);
    std::future<double> upperDeck = std::async(std::launch::async, fetchUpperDeckPrice);
    std::cout << "lower bowl ticket: " << lowerBowl.get() << '\n'; // lower bowl ticket: 24.5
    std::cout << "upper deck ticket: " << upperDeck.get() << '\n'; // upper deck ticket: 31.75
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

The two fetches have nothing to do with each other, so they overlap: the wait is about 60 milliseconds, not 120. The `sleep` takes milliseconds, as `std::chrono::milliseconds(60)` does.

Your editor shows a muted `wait` hint where a call pauses. Write `wait` yourself only when two calls must finish in a set order and the compiler cannot see why. Write `background` to start a task that keeps running after your function returns, which is the closest match to `std::thread`. A value you hand to a background task must be given away or copied, so two tasks never write the same value. A `Channel<T>` is the one value both sides can use, and it takes care of the waiting for you. See [Concurrency](../reference/REF-concurrency.md).

The C++ words are not valid. `std::thread` gives only general errors, so remember the rule: write `background fetchLowerBowlPrice()`. Writing `std::async(...)` gives errors, and among them this warning:

```
let lowerBowl = std::async(fetchLowerBowlPrice)
// COMPILE WARNING: `async` is not a keyword in Yinz.
// `async` not valid here
```

Its hint says: "Write `wait foo()` at the call site — no declaration-level annotation needed." A `std::future` has no match: the call gives the value directly.

### Copies: `let b = a` hands the value over, and `.copy()` replaces the copy constructor

In C++, `b = a` copies the whole value, and the old name stays usable. `std::move(a)` hands the contents over, and `a` stays usable in an unspecified state. You write copy constructors and move constructors to control both. In Yinz, the default is the other way round: storing a value in another name, an array or a shape hands it over, and the old name cannot be read afterward. Asking for a second, separate value is `.copy()`. There is no constructor to write, because `.copy()` copies every field.

C++:

```cpp
#include <iostream>
#include <utility>
#include <vector>

int main() {
    std::vector<int> scores{70, 85, 92};
    std::vector<int> backup = scores;            // a full copy
    std::vector<int> taken = std::move(scores);  // hands the items over
    std::cout << scores.size() << '\n';          // compiles; scores is valid but unspecified (usually 0)
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

`std::move` and copy constructors do not exist. The compiler gives only general errors for them, so remember the rule: storing a value already hands it over, so `let taken = scores` replaces `std::move(scores)`, and `.copy()` replaces a copy constructor.

Passing a value to a function is not a store: the signature decides, with `share`, `lend` or `give` as the Functions trap shows. The same hand-over rule applies when you store a shape value in an array. In C++, `path.push_back(start)` copies `start`, and a later change to `start` does not reach the vector. In Yinz, you ask for the copy:

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
