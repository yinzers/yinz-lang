---
name: Yinz Teaching Surfaces Protocol
description: "Applies when a change touches a diagnostic, hover, lint suggestion or muted hint (`Diagnostic::`, registry hover, what/why fields, `inlay_hint`), or the lexer's `lex_one` handlers for other languages' characters."
trigger: relevance
---

Every user-facing diagnostic, hover, lint suggestion and muted hint answers three slots. Grade each slot on its own. Why: a string that nails WHAT but fumbles WHY is still non-conformant.

- **WHAT states the problem the user actually hit, in plain English:** not a category label and never a raw tool-output dump (a linker's stderr as the WHAT or WHY). Why: tool output belongs in the message body, not the teaching slot.
- **WHAT-INSTEAD is a fix the user can do and copy** ("Rename one of them, e.g. `count2`"). A restated rule ("Each parameter must have a unique name") is a WHY in disguise. Why: the test is "can the user do this?"
- **WHY is contextual and non-circular, and cites no internals:** no runtime names, LLVM terms or restatement of the WHAT ("this fails because it's invalid"). Why: internals mean nothing to the user, and a circular WHY teaches nothing.

**Pass the audience bar:** "Would an 18-year-old JS developer understand this without Googling?" If a slot needs CS jargon, replace the term or drop the sentence. Why: do not gate the explanation behind vocabulary the audience does not have.

**Apply the registry's `[[banned_jargon]]` list to every teaching string.** "infer" and "inference" are the common miss. Why: they are correct in internal text and banned the moment they reach a user.

**Keep internal doc paths and milestone or version numbers out of user-facing text.** Cite a stable feature-registry name, or drop the citation, and say "ships in a later version". Why: the user has no repo to open, and a milestone number means nothing outside the project.

```
❌ WHY: "sleep requires the Tokio runtime (started by ynz_rt_init)."
✅ WHY: "sleep needs the scheduler runtime that Yinz starts for you automatically — this call can't run before that runtime exists."
```

**Give every character that means something in JS, Python, Rust, Go, TypeScript, PHP, Swift or Kotlin its own handler in the lexer's `lex_one`.** The handler consumes the whole construct, names what the character does in that language, and gives the Yinz equivalent. Each handler has a test in `crates/ynz-parser/tests/lex.rs`. Why: a newcomer who types a familiar character gets a lesson instead of a cascade of unrelated errors, and an untested handler regresses unseen.
