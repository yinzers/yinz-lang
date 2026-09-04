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
Kleene fixpoint over every parameter. It converges under mutual recursion, runs in the same query as
body checking (`queries.rs:484-512` — today AFTER `check(...)` at `:~423`, but it depends only on the
parse and the signature tables, so hoisting it above the body check is a reorder; ordering verified
on a direct read, `git log --grep=m8-p2`), and **already classifies "passed
to a declared `give` position"** as `Writes` (`:410-411`, `:676`, with a test at `:1391`). The
design's stated reason for reporting one frame per compile — that typeck lacks a callee-before-caller
ordering — is factually false against this module.

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

---

## Minting a `git log --grep` token that no commit carries

**Found:** 2026-09-03, v0.3-M8 Phase 2, twice in one review loop.

| Instance | Pointer written | What actually resolved |
|---|---|---|
| Phase 1 fix rounds (caught as parked item 26) | `--grep=m8-p1-fix2`, `--grep=m8-p1-fix3` | nothing — the seal commit's subject never carried the dispatch ids |
| Sign-off fix round (caught by the confirmation seat) | `--grep=FRAGO-009` | nothing — FRAGO 009 existed only in `audit.md` prose |

**The producer.** `decision-records.md` says docs carry current state plus a `git log --grep`
anchor, so authors reach for a grep token by reflex — and mint one against a NAME (a dispatch id, a
FRAGO number) that lives in a planning file, not in any commit message. An executor cannot commit,
so at write time the token is guaranteed dead; whether it ever resolves depends on a conductor
remembering to carry it in the seal. The sign-off round's executor saw this and correctly refused to
mint `--grep=m8-p2-signoff`; the fix round right after it minted `--grep=FRAGO-009` anyway.

**Detection signature.** Any `git log --grep=<token>` written in the same diff that introduces the
token, or citing a FRAGO/dispatch/session id. Run `git log --all --grep=<token>` before the pointer
ships; zero hits means it's dead.

**The rule.** An executor cites the DURABLE record it can see (`audit.md`'s entry by heading, a
SHA that already exists). The CONDUCTOR owns grep tokens: every round-seal and phase-boundary
commit body carries every FRAGO number, dispatch id and session id the round's docs cite, so the
pointer resolves the moment the commit lands. If a doc must name a future token, it says so
("resolves once the Phase N boundary commit lands") instead of pretending it already does.

---

## Sending a tree-mutating seat into an UNSEALED checkout

**Found:** 2026-09-04, v0.3-M8, twice — Phase 3 round 1 and Phase 4 round 3.

| Instance | What the seat did | What it cost |
|---|---|---|
| Phase 3 r1 | `test-quality` reverted `runtime.rs` while `code-reviewer` was reading it, both on the unsealed round-1 tree | `code-reviewer` observed the ladder arm in its REVERTED order mid-grade; graded the restored tree by luck of timing |
| Phase 4 r3 | `test-quality` restored its reverts with `git checkout -- check.rs types.rs` on the unsealed round-3 tree | HEAD was round 2's seal, so the restore **wiped round 3's uncommitted typeck work** (the `.failed()` guard, `restore_ec_receiver_ty`, the bignum split); its "sha256 matched before/after" was true of a diff that had just been zeroed |

**The producer.** The conductor graded a round BEFORE sealing it, and let a seat that mutates
source (revert-proofs) loose in that checkout. `git checkout -- <file>` restores to HEAD, not to
"the state before my experiment" — on a sealed tree those are the same; on an unsealed tree the
seat destroys the very work it is grading and reports a clean restore. The execute-plan skill's
order ("seal each round once reviewers have returned") is what makes this possible: the seal
comes AFTER grading, so grading always runs on an unsealed tree.

**Detection signature.** A reviewer brief containing "revert", "restore", `git checkout --`,
`git stash`, or "apply the swap" dispatched while `git status --porcelain` is non-empty. Or a
post-grade `git status` that lists FEWER modified files than the executor's manifest.

**The rule.** (1) A seat that mutates source runs ONLY on a sealed tree: seal the round's
executor output first (a WIP seal is still a seal — the grading seats' verdicts ride the next
commit's body), THEN dispatch revert-proof seats, THEN fix-round on top. (2) A mutating seat
restores from a copy it saved (`cp` to the scratchpad, or `git stash push -- <file>` + `pop`),
never `git checkout --`. (3) Before any commit after a mutating seat, diff `git status` against
the executor's touched-paths manifest; a missing file is a wiped file.
