# Ownership

Yinz manages memory automatically — without a garbage collector. No `malloc`. No `free`. No unpredictable pauses.

The rule: every value has exactly one owner. When the owner goes out of scope, the memory is freed automatically.

---

## The dot modifiers

When you pass a value to a function, you tell the compiler how you're sharing it:

```
name.share     // I'm letting you read it — I keep ownership
name.lend      // I'm letting you modify it — I get it back when done
name.give      // I'm handing it over permanently — I lose it
name.copy      // Make a full copy — you get the copy, I keep the original
name.freeze    // Make it permanently read-only
```

All of these appear in autocomplete when you type `.` after a variable.

---

## Smart defaults — you usually don't need to write anything

Most functions just read their inputs. The compiler assumes `.share` when you don't annotate:

```
greet(name)           // same as: greet(name.share)
print(message)        // same as: print(message.share)
```

Only annotate at the call site when you're escalating beyond reading:

```
rename(player.lend)   // explicit — granting write access
consume(data.give)    // explicit — giving it away
```

---

## Function signatures always declare intent explicitly

Even though callers can omit `.share`, function signatures always say what they need:

```
function greet(share name: string) -> nothing     // I just need to read this
function rename(lend player: Player) -> nothing   // I'm going to modify this
function consume(give data: Data) -> nothing      // I'm taking this, you lose it
```

The signature is a contract. Anyone reading the function knows exactly what it does with its inputs.

---

## What happens after .give

Once you give a value away, you can't use it anymore. The compiler catches this:

```
consume(data.give)
print(data)
// COMPILE ERROR: data was transferred via .give() on the previous line.
// It no longer exists here. Use .copy() if you need to keep it.
```

---

## .copy — when you need to keep the original

```
let backup = original.copy    // full deep copy — original is unchanged
consume(backup.give)          // give away the copy, keep the original
```

---

## .lend — temporary write access

After a `.lend`, you get the value back when the function returns:

```
function addBonus(lend player: Player, amount: number) -> nothing {
  player.score = player.score + amount
}

addBonus(player.lend, 50)    // explicit — granting write access
print(player.score)          // player is still yours — .lend returns it
```

---

## .freeze — make a value permanently read-only

```
let config = loadConfig()
let safe = config.freeze    // safe can now be passed anywhere without risk of modification
```
