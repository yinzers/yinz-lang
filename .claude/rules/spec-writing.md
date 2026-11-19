---
name: Yinz Spec Writing Protocol
description: Applies when writing or editing a user-facing docs/reference/REF-*.md language-spec file.
paths:
  - docs/reference/REF-*.md
---

- **Write for a high-school graduate who knows JavaScript and has no systems background.** Test every sentence: "Would an 18-year-old JS developer understand this without Googling?" Why: the language promises it needs no CS jargon.
- **Order each section:** a one-sentence summary, the simplest example, a plain explanation, variations, then compiler errors. Why: the reader sees the simple case before the edge cases.
- **Use short, active sentences and "you".** Define a term the first time you use it. Never explain by comparing to another language. Why: readers should learn Yinz fresh, not through languages they may not know.
- **Do not chain methods in examples.** Why: examples teach the house style of one named step per line.
- **Leave out implementation details, benchmarks, design reasons, open questions and TODOs.** Design reasons go in the feature's IMP doc and open questions in SCRATCH-open-questions. Why: a spec shows how to use the feature, and unsettled text there reads as fact.
