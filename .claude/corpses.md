# Corpses

Failure patterns that PROVED they recur. An entry earns its place only after the same class of
defect has been found more than once in a real review loop — never from a self-narrated "this could
happen." Each entry names the producer, not the instance.

Companion to `.claude/graveyard.md` (which carries greppable Bouncer checks for specific shipped
instances). This file carries the diagnosis; the graveyard carries the detector.

---

## Enumerating syntactic sites instead of threading the whole-program ownership analysis

**Found:** 2026-09-03, v0.3-M8 Phase 1 (channel-close design), across three consecutive review rounds.

**The recurrence — same class, three rounds, escalating:**

| Round | Finding | What the analysis could not see |
|---|---|---|
| 1 | `send` freeing a payload it never took ownership of | that the caller still held it |
| 2 | consume not reaching the caller through an unannotated parameter | holders one call frame up |
| 3 | UFCS non-receiver args, non-ident args at `give` positions, and `for`-loop variables all bypassing the rule | holders reachable by any syntax the enumeration had not listed |

Each round's fix was correct for the instance it named and blind to the next syntactic form. Round 4
was already visible before it ran (`dynamic Contract` dispatch carries no ownership modifiers at
all).

**The producer.** The design derived "who else holds this value?" by **enumerating argument shapes at
call sites** — an ident here, a parameter there, a dot-call receiver over there — rather than
threading the one authoritative answer. Every round patched the enumeration; the enumeration itself
was the defect. A syntactic site list is unbounded and grows with the language, so it can never be
finished; each new expression form silently reopens the hole.

**What was already there, unused.** `crates/ynz-typeck/src/effective_ownership.rs` is a whole-program
Kleene fixpoint over every parameter. It converges under mutual recursion, runs before body checking
(`queries.rs:484-512`), and **already classifies "passed to a declared `give` position"** as `Writes`
(`:410-411`, `:676`, with a test at `:1391`). The design's stated reason for reporting one frame per
compile — that typeck lacks a callee-before-caller ordering — is factually false against this module.

**The aggravating detail, and the real lesson.** The SAME plan's Phase 2 exists specifically to reuse
`effective_ownership` rather than re-derive it, under
[`authoritative-derivation.md`](rules/authoritative-derivation.md). Phase 1 spent three review rounds
re-deriving a weaker version of that module, inside the same milestone, against the same rule, while
the rule's own canonical example sat in the next phase of the same document. Proximity to the rule
did not prevent the violation; nobody asked "does an analysis for this already exist?" until a third
reviewer went looking.

**Detection signature.** A design or diff that answers an ownership, escape, aliasing, or liveness
question by matching on `Expr::` variants at call sites. Ask immediately: is there an existing
whole-program analysis whose answer should be threaded here instead? In this repo, for anything about
who may read or write a value, the answer is `effective_ownership.rs` until proven otherwise.

**The cheap check that would have caught it in round 1.** Before writing a new predicate about
program-wide behavior, grep `crates/ynz-typeck/src/` for an existing analysis over the same subject.
One grep, round one, three rounds saved.
