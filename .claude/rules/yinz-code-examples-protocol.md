---
name: Yinz Code Examples Protocol
description: "Applies when writing a Yinz code example in docs, diagnostics, rules or plans."
trigger: relevance
---

An example teaches one idea in the smallest realistic code, and shows the result or the real compiler error. Why: a junior developer learns from a short example that does one thing and shows what happens.

**Real**

- **Use only operations that build with `ynz build`, or operations the example itself defines.** Why: readers copy examples, and an operation that does not build teaches a feature that does not exist.
- **Use realistic names** (`Player`, `score`, `health`), not `foo`, `bar` or `x`. Why: real names show what the code is for.
- **Show one idea per example.** Why: a second idea hides the first.

**Shows the outcome**

- **Show the output, or the real compiler error when the example shows a mistake.** Why: users will meet that exact text, and the result is what confirms the lesson.

**House style**

- **Write every non-type identifier in camelCase, constants included** (`const maxHealth = 100`). Never snake_case or SCREAMING_SNAKE. A distinct constants style needs Chief's confirmation, never an example introducing it. Why: capital letter means type, so a style the language lacks teaches users a mistake.

**Import paths**

- **Write import paths as backtick strings** (``import { Player } from `services/player` ``). A double-quoted path is not accepted. Why: backticks are the one string form this position takes.
- **Make them relative to the project root:** never start with `./` or `../`. Why: the path reads the same from any file.
- **Leave off the `.ynz` suffix.** Why: the extension is implied.
