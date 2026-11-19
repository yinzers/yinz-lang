---
name: Yinz Inference Protocol
description: "Applies when designing IDE hints, hover tooltips or anything the compiler works out for the developer, or when choosing between \"infer\" and plain wording."
trigger: relevance
---

**If the compiler can figure it out from context, the developer does not type it; the IDE shows it as a muted hint with a hover that says what was decided and why.** Why: forcing everything explicit is noise that developers learn to ignore, hiding it is magic, and hints let them learn the rules by reading.

- **Make each hint informative at a glance:** what the compiler decided plus a one-clause why, short enough not to clutter. Why: the developer should not have to hover to learn from it.
- **Make click do the useful thing.** Where the hint has typeable Yinz source, click inserts it. Where it has none, click jumps to the relevant declaration (for call-site ownership, the function signature). Why: ownership modifiers exist only in signatures, so there is nothing to insert at a call site.

## Placement categories

Ask once at design time: "could the user have typed something to make this explicit?" The answer fixes the surface. Why: one renderer per category stays learnable, so never mix surfaces within a category.

- **Addition** (a specific position): muted text appears at that position, and click types it into source. Examples: `: Int` after a variable name, `wait` before a call, the capacity `64` inside empty `Channel<T>()` parens. Why: it has zero source-versus-render ambiguity and is visible without hovering.
- **Replacement** (replaces existing text): a dotted-underline decoration sits on the existing token, with the alternative form in the hover and click converting it. Examples: `let` to `const`, `Array<T>` to `Fixed<T>`. Why: inline replacement breaks cursor position and makes the screen differ from the file.
- **Informational** (no source form): a muted comment-style annotation near the expression. Examples: static dispatch, scheduler routing, call-site `share`/`lend`/`give`, auto-`Arc`. Why: these carry multi-clause reasoning that a hover would hide.

## Styling

- **Style benign decisions neutral gray** (types, lifetimes, allocators) **and decisions about mutation, ownership transfer or thread crossing red-tinted.** Why: the higher-stakes ones should stand out.
- **Never style a compile error as a hint.** Errors keep standard error styling. Why: a hint is information and an error is a stop.

## Hover and call sites

- **Give every hover three parts:** WHAT is happening, WHAT INSTEAD the developer could write, and WHY the compiler chose it, tied to this call site. The registry carries them as `hover_what`, `hover_what_instead` and `hover_why`. Why: a generic reason teaches nothing.
- **Never require an annotation at a call site that the signature already settles** (explicit `share` at every call, types on every variable, typed wait points). Signatures do declare `share`, `lend` and `give`. Why: the contract is stated once at the definition, and repeating it at every call is noise.

## Wording

- **Use "infer" and "inference" in rules, design docs and engineer-facing text; never in a diagnostic, hover or hint a user sees.** Say "figure out automatically" or "the compiler can tell". The banned list is the registry's `[[banned_jargon]]`. Why: engineers need the precise term, and users should not need compiler vocabulary.
