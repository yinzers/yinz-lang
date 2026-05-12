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
