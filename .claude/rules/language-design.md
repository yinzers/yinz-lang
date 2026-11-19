---
name: Yinz Language Design Protocol
description: "Applies when proposing, designing or reviewing a Yinz language feature, keyword or syntax."
trigger: relevance
---

A design must satisfy every golden rule. These checks add what the golden rules do not say:

- **New keyword names:** a keyword with no plain English word needs Chief's confirmation. Why: a name that needs docs fails the readability bar.
- **Banned keywords:** never add `class`, `struct`, `interface`, `enum`, `abstract`, `new`, `this`, `instanceof` or `override`, or function-typed fields used as methods. Why: they pull in object-oriented habits.
- **No try/catch/recover syntax in any form.** Known failures use `errors`; unknown panics are isolated per task, with a supervisor at the task boundary. Why: catching mid-function hides failures from the reader, while `errors` and task isolation keep every failure visible and contained.
- **Kernel/runtime:** state what the feature needs at runtime and whether it works in the no-runtime `--kernel` mode. Why: Yinz must stay usable down to driver and kernel level.
- **Compile cost:** state what it costs the compiler and whether the analysis is incremental or release-only. Why: compile speed is a goal.
- **Performance baseline:** compare performance against a Rust baseline benchmark. Why: Rust-level speed is the positioning, and a benchmark that only compares Yinz with itself cannot show it.

A decision that no golden rule explains is reconsidered. Why: an unexplained decision is either a missing rule or a mistake.
