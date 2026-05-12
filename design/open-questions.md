# Open Questions

Design decisions that haven't been made yet. When one is resolved, move it to its dedicated design file and remove the entry here.

Resolved questions are NOT listed here — see `design/decisions.md` for the index of resolved topics.

Deferred features (decided not to ship in v0.1) are NOT listed here either — see `design/deferrals.md` for the ledger.

---

## HTTP Module Design — Three-Tier API

When v0.15 (`http` client) comes up for implementation, the module needs a focused design session. The general shape is locked but specifics need work:

**Tier 1 — High-level helpers:** `http.get(url)`, `http.post(url, body)`, `http.put`, `http.delete`, `http.websocket(url)`. The common-case API, dot-method-first.

**Tier 2 — Mid-level request builder:** `http.request().method("GET").header(...).timeout(5).send()` for cases that don't fit the simple helpers. Step-by-step style — every operation gets its own line.

**Tier 3 — Low-level socket access:** `net.tcp.connect(host, port)` returning a raw socket. The floor of the user-accessible network stack. Framework authors can build their own routing layer, their own protocol implementations, anything on top of this. Going lower means FFI (deferred to v2+).

**Open sub-questions for the v0.15 design session:**

- Exact dot-method names for the high-level tier (`http.get` vs `http.fetch` vs `http.request`?)
- WebSocket lifecycle — connect, message events, close handling. Builds on FallibleIterable contract per `design/iterables.md`?
- TLS configuration — defaults to validating certs; how to opt out for self-signed in dev?
- Streaming bodies — for large uploads/downloads, can the body be an iterable?
- Cookie handling — first-class or always manual headers?
- Proxy support — env var detection (`HTTP_PROXY`) automatic, or always explicit?

These get answered when v0.15 is up to design. Not blockers for v0.1-v0.14.

---

## Compiler Error Format — Full Spec + Audit

The spec shows example compiler error messages. The exact format, structure, and tone of all compiler errors needs a full spec of its own before implementation.

Two concrete pieces of work, both needed:

**(A) Write the error-message style rule.** A dedicated file (likely `design/compiler-errors.md` or section in `design/linting.md`) that pins down:

- **No programmer jargon.** Words like "propagate," "narrow," "discriminator," "monomorphize," "infer," "polymorphic," "covariant" are banned from user-facing error messages. Use plain-English equivalents:
  - "propagate" → "let the error pass up to the caller" or "bubble up"
  - "narrow" → "the compiler now knows it's a [type]"
  - "discriminator" → "the tag that says which kind it is"
  - "infer" → "figure out automatically"
  - "polymorphic" → "works with any type"
- **Test a jr dev can read it.** Every error message should be readable by a developer who just graduated high school, knows JavaScript, and has never done systems programming. If a sentence requires a CS degree to parse, rewrite it.
- **Required three-part format** per `design/teaching-mission.md`: WHAT happened, WHAT to do instead, WHY. Every diagnostic answers all three.
- **Visual structure** (header, location, source snippet with arrows, suggestion, optional reference link)
- **Multi-error reporting strategy** (always show all, or stop after N?)

**(B) Audit existing spec and design docs for error-message examples and rewrite any jargon.** Every example compiler error message in `spec/**/*.md` and `design/**/*.md` files needs review:
- Sweep all error message examples
- Flag every instance of programmer jargon
- Rewrite in plain English following the rule from (A)
- Cross-reference: error messages that appear in multiple files should be consistent

This is the meta-rule: **the compiler's character as a teacher (Rule 11) is set by its error messages. If those use jargon, the language fails its own promise — regardless of how good the syntax design is.**
