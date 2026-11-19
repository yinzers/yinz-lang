---
name: Yinz Vocabulary Protocol
description: "Applies when writing or reviewing Yinz docs, diagnostics or design text: the official user-facing terms and the banned legacy ones."
trigger: relevance
---

Use these terms in user docs, design docs, diagnostics and chat output, never legacy terms from other languages. Why: one word per concept keeps every doc consistent. A concept with no official term needs Chief's confirmation before anyone invents one. Why: the vocabulary is curated, not crowdsourced.

The banned legacy words are the `[[banned_jargon]]` entries in `registry/features.toml`; do not copy that list into text. The word "type" is fine in prose (a keyword ban applies only to `type Foo { }` as a declaration). Why: the registry is the one list, and the compiler enforces it.

| Concept | Yinz term | Not |
|---|---|---|
| Declare a data structure | `shape` | type, struct, class, interface |
| An instance of a shape | value ("a Player") | object, instance, struct |
| Key-value collection | `Map<K, V>` | dictionary, HashMap, object |
| Growable list | `Array<T>` | Vec, list |
| Stack list, size set at creation | `Fixed<T>` | static array, stack array |
| Enum replacement | `options` | enum |
| Non-instantiable base | `base shape` | abstract class |
| Contract | `follows` | implements |
| A-or-B type | `union`, written `\|` | sum type, variant |
| Maybe-absent value | `Maybe<T>` | Optional, Option, nullable |
| Absent value | `none` | null, undefined, nil |
| No return value | `Nothing` | void, unit |
| Function declaration | `function` | fn, func, def |
| Implementing type / instance | `Self` / `self` | this |
| Borrow or transfer | `share`, `lend`, `give` in signatures only | &T, &mut T, move |
| Copy / freeze a value | `.copy()` / `.freeze()` | clone |
| Fallible function | `errors` keyword | Result, throws, exceptions |
| Type narrowing | `is` | typeof, instanceof |
| Wait point / spawn task | `wait` / `background` | await, async, go, thread |
| Block compiler safety | `verified { }` | unsafe |

Distinctions that cause mistakes:

- **A `shape` is the declaration; a value is an instance.** Write "a Player value", never "object", "instance" or "struct". Why: those words import another language's model.
- **Never call a value a "map" unless it is a `Map<K, V>`.** A shape's field names are known at compile time and read at a direct offset; a map's keys are known at runtime and read by hash lookup. Why: JavaScript conflates the two, and Yinz does not.
- **Prefer `Fixed<T>` when the size is known and `Array<T>` when it can grow.** Why: `Fixed<T>` lives on the stack.
- **`Maybe<T>` means "no value" is a normal outcome.** Use `errors` when the caller must handle a failure. Why: absence and failure are different promises.
