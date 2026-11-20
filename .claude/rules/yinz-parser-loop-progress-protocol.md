---
name: Yinz Parser Loop Progress Protocol
description: "Applies when writing or changing a parser loop, error recovery, or a `macro_rules!` that uses `continue` inside a loop."
paths:
  - crates/ynz-parser/src/**
---

- **Every parser loop consumes at least one token or exits (`break` or `return`) on every pass.** Why: a non-advancing recovery path hangs the compiler with no message.
- **Never put a `macro_rules!` that contains `continue` inside `loop { ...; break; }`.** Pull the body into a method returning `Option`, like `parse_struct_lit_one_field`. Why: the `continue` targets the inner loop, not the one you meant, so the pass does not advance.
