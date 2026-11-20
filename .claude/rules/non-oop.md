---
name: Yinz Non-OOP Protocol
description: "Applies when designing or reviewing anything touching shapes, methods, dispatch, inheritance, contracts or polymorphism."
trigger: relevance
---

**Yinz is not object-oriented: shapes hold data and contract signatures, and behavior lives in standalone functions.** Why: it gives Rust-level speed (no per-instance method storage), simpler ownership (each signature declares its own), and two concepts for a junior developer instead of six.

- **Shapes hold fields and contract signatures only; a method body inside a shape is a compile error.** Why: methods bound to instances bring back the OOP model.
- **Put functions at file or module level.** `value.method(args)` is parser sugar for `method(value, args)`; both forms are legal and equivalent, so pick the one that reads better. Why: dot-first autocomplete at no runtime cost.
- **Resolve `value.` by first parameter:** the IDE lists the fields plus the functions whose first parameter type matches the value. Why: that is what makes dot-first autocomplete accurate.
- **Show both call styles in a failed-call diagnostic:** list why each same-named candidate fails, then the functions callable on this value in dot and function form. Why: the first error teaches both forms.
- **`extends` shares data only:** the child inherits fields, with no `override` and no virtual dispatch. Why: behavior comes from functions, not inheritance.
- **Give overload-shaped functions distinct names** (`greet`, `greetWarrior`). Two functions with the same name in one file are a compile error, whatever their parameter types, because overloading by argument type is not built. Why: the compiler cannot yet pick the most specific one.
- **A shape `follows` a contract when standalone functions with matching signatures exist;** a missing function fails compilation at the `follows` declaration. Why: matching is structural, so no `implements` ceremony.
- **Polymorphism comes from two mechanisms:** generics (monomorphized, static) and `dynamic Contract` (one function-pointer table per shape-and-contract pair, looked up at run time). Why: dynamic dispatch is paid for only when asked.

| OOP pattern | Yinz |
|---|---|
| Class with methods | shape (data) plus standalone functions |
| Constructor, factory | a function returning the shape |
| Abstract class | `base shape`: data and contract signatures, not instantiable, must be extended |
| Interface | a contract shape plus `follows` |
| Virtual method | `dynamic Contract` |
| Private field | `hidden` field, visible only to functions in the same file |
