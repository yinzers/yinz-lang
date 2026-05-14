# Naming Conventions

## Golden Rule 13 — Capital Letter = Type

Capital letter = type. Everything else = lowercase. This is universal and absolute.

```
// Types — PascalCase
Player, Warrior, Config, Request, Response, Date, Duration, Database

// Modules — lowercase
file.read(), http.get(), date.now(), math.sqrt(), json.parse()

// Functions — lowercase camelCase
function fetchUser(), function processOrder()

// Variables — lowercase camelCase
let userName, let playerCount

// Keywords — lowercase
function, let, const, shape, wait, background, options, follows, extends
```

Scan any line. Capital letter = type. No capital = not a type. Zero ambiguity.

Note: Module and type can share the same base name with different casing:
- `Date` = the type (returned from date functions)
- `date` = the module (`date.now()`, `date.from()`)
- `Duration` = the type
- `duration` = the module (`duration.hours()`)

`Self` (capital S) is a reserved type keyword meaning "the implementing type" — used in `follows` contracts. `self` (lowercase) = the instance. `Self` (uppercase) = the type of the instance.

---

# Renamed Concepts

Always use Yinz terms. Never use the traditional term in spec files or user-facing content.

| Traditional term | Yinz term | Notes |
|---|---|---|
| `void` | `nothing` | Functions that don't return use `-> nothing` |
| `null` / `undefined` / `None` | `none` | The built-in absent value |
| `Optional<T>` | `maybe T` | Sugar for `T or none` — interchangeable |
| `struct` / `class` / `interface` / `type` | `shape` | One keyword for all data structures. `type` is banned because it's overloaded with the generic concept of "type." |
| `enum` | `options` | `options Status { active, inactive, banned }` |
| `abstract class` | `base shape` | Cannot be instantiated directly |
| `implements` | `follows` | `shape Player follows Damageable` |
| `\|` (union syntax) | `or` | `shape Result = Success or Failure` |
| `typeof` / `instanceof` / type guards | `is` | `if (shape is Circle)` |
| `fn` | `function` | Always spell it out |
| `&T` (shared reference) | `.share` | Dot modifier on value |
| `&mut T` (mutable reference) | `.lend` | Dot modifier on value |
| `move` / transfer ownership | `.give` | Dot modifier on value |
| `.clone()` | `.copy` | Dot modifier on value |
| `Result<T, E>` / `throws` | `errors` keyword | `-> string errors` |
| `match` / `switch` on types | `if (x is Type)` | Pattern matching via type narrowing |
| `T[]` / `Array<T>` | `array<T>` | Growable, heap-allocated |
| fixed-size array / stack array | `fixed<T>` | Stack-allocated, size-locked at creation |
| `HashMap<K, V>` / `Map<K, V>` | `map<K, V>` | Dynamic key-value |
