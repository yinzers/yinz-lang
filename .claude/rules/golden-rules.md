---
name: Yinz Golden Rules Protocol
description: The 13 rules every Yinz design and code change follows.
---

Project docs under `docs/` hold the detailed guidelines.

When two rules conflict, first find a design that satisfies both, usually by having the compiler do the work and show it. If none exists, the lower number wins. Readability against speed, and any safety opt-out for kernel code, need Chief's confirmation. Why: most conflicts disappear once the compiler absorbs the cost, so the number only settles what cannot be reconciled, and trading one goal for another is Chief's call.

## Not object-oriented

- Shapes hold data plus contract signatures, never method bodies. A shape `follows` a contract when standalone functions with matching signatures exist (structural matching). `dynamic Contract` gives runtime dispatch on a contract; otherwise dispatch is static. Why: no per-instance method storage, ownership stays simple, and dynamism is paid for only when asked.
- Methods are standalone functions, and `value.method()` is sugar for `method(value)`. Why: dot-first autocomplete without methods bound to shapes.
- `extends` shares data only, and there is no `override`. Why: behavior comes from functions, not inheritance.
- Read "method" and "dispatch" in the rules below as functions.

## The 13 rules

1. **No garbage collector.** Ownership-based memory only, one memory model. Why: it is deterministic, has no pauses, and has no second mental model.
2. **Compile-time safety.** Every check that can run at compile time does; a failure that can't (I/O, bugs like division by zero) is explicit: `errors` or a task-isolated panic. No silent runtime failure. Why: a compile error costs far less than a 3am production crash.
3. **The compiler is a teacher.** Every diagnostic and IDE hint answers WHAT, WHAT INSTEAD, and WHY, and the WHY is specific to the user's code, not generic. This holds on every surface: compiler diagnostics, IDE hints and hovers, lint suggestions, doc generation. One concept gets one wording on every surface. Why: feedback is the moment learning happens.
4. **Self-documenting syntax.** Every keyword, function and pattern reads clearly to a junior developer who has never seen Yinz. Why: if a line needs docs, the design failed.
5. **Human-readable over jargon.** `options` not `enum`, `follows` not `implements`, `Nothing` not `void`. Why: every term that needs CS background is a barrier to junior developers, and plain words are guessable. Exception: union types use `|`. Why: `or` is a prose word, while `|` is unambiguous and familiar from TypeScript.
6. **Zero-cost abstractions.** High-level syntax compiles to what a careful Rust/C++ programmer would write by hand; the features themselves cost what they inherently cost. A design that costs more than that needs Chief's confirmation, recorded in the commit and the feature's IMP design doc. Why: if convenience costs speed, developers bypass the readable path.
7. **Efficiency first, dynamic after.** The default is the fastest option; slower or flexible behavior is an explicit opt-in. Why: junior developers do not know which choices are fast. When the compiler proves a stricter or faster form fits, it uses it (codegen promotion) and shows the user why (a muted IDE hint and a lint suggestion). Why: code stays fast by design and the user learns the explicit form.
8. **Compiler does the hard work.** Smart defaults everywhere: inference inside function bodies, operation fusion, optimal layout. Expensive whole-program analysis runs incrementally or only in release builds. Why: developers should not hand-optimize what the compiler can work out, and compile speed is a goal.
9. **Step-by-step over chaining.** No method chaining: one operation per line, each result named. Why: every intermediate value can be inspected, and the compiler fuses the steps at no cost.
10. **Capital letter = type.** Every type is PascalCase, built-ins included (`Int`, `String`, `Array<T>`); modules, functions, variables and keywords are lowercase. A name can exist in both casings (`Date` the type, `date` the module). Why: casing alone tells a reader what category a name is, with no exception to learn.
11. **Dot-first design.** Anything that can be `.method()` with autocomplete is. Why: typing `.` shows the usable functions, so nobody looks up docs.
12. **Familiar syntax.** Borrow from TypeScript/JavaScript wherever possible. Why: tens of millions of developers feel at home on day one.
13. **Fast to type.** Quick to write without losing readability. Why: `function` over `fn` earns its characters, while ceremony like `Optional<T>` is cut.
