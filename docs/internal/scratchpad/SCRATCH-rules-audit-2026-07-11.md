# Project Rules Audit — 2026-07-11

Scope: the project rule corpus — all 15 files in `.claude/rules/`, plus `CLAUDE.md` as their
always-loaded summary layer, cross-checked against each other, against the REF/IMP docs they
cite, against `registry/features.toml`, and against the shipped compiler behavior verified in
the sibling teaching audit
([SCRATCH-teaching-audit-2026-07-11.md](./SCRATCH-teaching-audit-2026-07-11.md)).
Audience: the doc-author who cleans this up. Findings ordered most-damaging-first per class.

**Headline verdict**: the rule *content* is strong — but the corpus has fossils. Two rules
(`naming.md`, parts of `inference.md`, `language-design.md`) still teach a **pre-lock design
that the rest of the corpus explicitly bans**, and the sibling audit found the downstream
artifacts (registry templates, hover text) drifted in exactly the direction the fossil rules
teach. The degradation Patrick suspects is real and traceable: **stale rules are training
data** — every AI session reads the contradiction and half of them follow the wrong half.

---

## Class 1 — Live contradictions between rules (fix first; these actively corrupt output)

### 1.1 `naming.md` teaches the dead ownership design (FLAGSHIP)
- `naming.md` "Renamed Concepts" table: `&T → `.share` — **Dot modifier on value**`, same for
  `.lend`, `.give`, and `.copy` (no parens).
- `vocabulary.md` Quick Reference: "`share` keyword in signature; compiler-inferred at call
  sites. **NO body-level `.share()` syntax**" (same for lend/give), and `.copy()` **with
  parens** as a body operation.
- `inference.md` + `dot-postfix.md` + graveyard Entry "Requiring Explicit Ownership Annotation
  at Call Sites" all agree with vocabulary.md.
- So one always-loaded rule teaches call-site dot-modifiers as current syntax while three
  others ban it. **Downstream damage already observed** (teaching audit): the registry template
  teaching `background fn(value.give)` (A1) and the `.copy`-without-parens strings (E3) are
  exactly what a writer produces after reading naming.md's table.
- **Fix**: rewrite naming.md's four ownership/copy rows to the signature-keyword model (or
  delete the rows and point at vocabulary.md — see Class 2).

### 1.2 `inference.md` contradicts itself on the ownership hover
- Its "Domains" table marks ownership-at-call-site **Informational — "no body-level syntax to
  insert"**, click jumps to the signature.
- Its "Hover Tooltip Format" canonical example, same file: "**WHAT INSTEAD**: You could write
  `foo(player.share)` to make it explicit. The behavior is identical."
- Both cannot be true; the example is a fossil of the pre-lock design. **This is the likely
  source of teaching-audit A2** (registry hover fallbacks telling users to type non-typeable
  text) — the rule's own canonical example models the mistake.
- **Fix**: rewrite the canonical hover example to an Informational-correct WHAT-INSTEAD
  ("nothing to type — the modifier lives on `foo`'s signature; click to jump there").

### 1.3 `language-design.md` vs `docs-checklist.md` on where decisions live
- `language-design.md` §Documenting Decisions: "Every decision goes in **`/docs/README.md`**
  with: what was decided, alternatives, which golden rule drove it."
- `docs-checklist.md` §One Rule No Exceptions: "**Never** add design content to
  `docs/README.md` — that file is an **index only**." CLAUDE.md agrees (decisions → `IMP-*.md`
  / `ADR-*.md`).
- `spec-writing.md` repeats the wrong pointer: "Don't explain WHY… — that belongs in
  `/docs/README.md`."
- **Fix**: point both at `docs/internal/implementation/IMP-<feature>.md` per docs-checklist.

### 1.4 Golden-rule count drift: "12" vs 13 — including inside CLAUDE.md itself
- CLAUDE.md's Golden Rules section lists **13** (R13 capital-letter added later).
- `language-design.md`: "check every proposed feature against all **12** golden rules" (×1,
  plus "against all 12 golden rules" in its intro). `CLAUDE.md` §When Working on This Project:
  "Check every proposed language feature against all **12** golden rules" — stale in the same
  file that lists 13.
- **Fix**: say "all golden rules" (count-free) everywhere except the canonical list, so the
  next added rule can't re-create this.

### 1.5 `stdlib-design.md` Rule 4 vs the shipped channel constructor
- Rule 4: "`channel<T>(capacity: int)` — bounded, **the only constructor**."
- Shipped (v0.3-M4, verified in check.rs + registry `channel_capacity` muted-hint domain):
  `channel<int>()` is valid — capacity defaults to 64, with an IDE hint making the default
  visible. Still bounded (the rule's intent holds) but the rule's letter is now false.
- **Fix**: amend Rule 4 to "bounded always; explicit capacity or the locked default (64);
  no unbounded constructor" and cite the registry domain.

### 1.6 `dot-postfix.md` vs `feature-registry.md` on the intrinsics SSOT
- `dot-postfix.md` (twice): "Cross-reference `crates/ynz-typeck/src/intrinsics.rs` … **source
  of truth** for which dot-postfix methods exist."
- `feature-registry.md` + `IMP-feature-registry.md`: `registry/features.toml` is the SSOT;
  code follows the registry. (`banned_jargon.rs` is already a thin adapter — same model.)
- Two rules naming two different SSOTs for the same inventory is exactly the disease
  `authoritative-derivation.md` exists to kill.
- **Fix**: dot-postfix points at `registry/features.toml` `[[primitive_intrinsic]]`.

---

## Class 2 — Duplication (parallel copies that WILL drift; 1.1 proves it)

### 2.1 The renamed-concepts table exists twice
- `naming.md` §Renamed Concepts (~25 rows) and `vocabulary.md` §Quick Reference (~28 rows)
  are the same table maintained by hand in two files. They have **already drifted** (the
  ownership rows, 1.1; `.copy` vs `.copy()`). One must become a pointer.
- **Recommendation**: vocabulary.md keeps the table (it's the richer, current one and already
  owns the banned-jargon and prose-usage guidance); naming.md shrinks to the casing rule +
  module/type case distinction and links vocabulary.md for term mappings. (Or merge the two
  files outright — their scopes overlap ~70%.)

### 2.2 Golden Rule 13 restated in four homes
- `REF-golden-rules.md` (canonical), CLAUDE.md header list, `naming.md` §Golden Rule 13
  (full section with examples), `vocabulary.md` §Capital Letter Rule (another full section).
  Four hand-maintained copies of one rule. Keep the canonical + CLAUDE.md one-liner; the two
  rule files should state it in one line + link.

### 2.3 Non-OOP drift-signals list in three homes
- `non-oop.md` §Banned Anti-Patterns (canonical), `language-design.md` §OOP Drift Test
  (re-spelled bullet list), CLAUDE.md bullet (summary — acknowledged). The language-design
  copy is long enough to drift; cut to one line + link.

### 2.4 Auto-promotion surface-criterion table in two homes
- `inference.md` §Two Surfaces for the Same Decision and `auto-promotion.md` §The Pattern both
  define the typeability criterion + the same worked examples (array→fixed, let→const,
  auto-SoA). Cross-referenced but re-spelled. Pick the owner (inference.md owns *when a muted
  hint applies*; auto-promotion owns *the three-surface pattern*) and de-duplicate the
  criterion table to one home.
- Related: `inference.md`'s copy-points example string "`.copy (8 bytes, trivially copyable)`"
  is the jargon source flagged in teaching-audit E5 — fix it in whichever file survives.

### 2.5 `vocabulary.md` banned-jargon table vs registry `[[banned_jargon]]`
- vocabulary.md carries a prose banned-terms table (+ "M7 additions" list); the registry has
  55 `[[banned_jargon]]` entries as the machine SSOT. Two homes; vocabulary's is a partial
  snapshot and its pointer ("banned via `crates/ynz-diagnostics/src/banned_jargon.rs`") is
  stale — that file is a thin re-export; the SSOT moved to `registry/features.toml`.
- **Fix**: vocabulary.md keeps a handful of illustrative rows + "full list: registry
  `[[banned_jargon]]`" and corrects the mechanism pointer.

### 2.6 Acknowledged duplications (keep, but mark)
- plan-invariants §Demo & Error Gallery ↔ CLAUDE.md bullet — deliberate and self-documented
  ("also stated in CLAUDE.md"); fine. Consider a one-line "SSOT: plan-invariants.md" marker on
  the CLAUDE.md side so a future edit knows which to change first.

---

## Class 3 — Staleness (rot from the 2026-07-01 docs migration and shipped milestones)

### 3.1 Pre-migration paths still cited as live
- `vocabulary.md` line 1: "All user-facing docs (**`spec/`**), design docs (**`design/`**)…"
- `plan-invariants.md` §Design-Doc Alignment: "checked against the governing **`/design/`**
  docs — especially **`design/future/`**…" (the correct spellings appear in CLAUDE.md).
- `language-design.md` §Spec Updates: "update the relevant **`/spec/`** file immediately."
- `spec-writing.md`: "open questions live in **the design folder**."
- `stdlib-design.md` Rule 7: "cross-referenced into a future **`design/stdlib/regex.md`**"
  (new taxonomy: `docs/internal/scratchpad/SCRATCH-stdlib-*.md`); §Cross-References cites
  `lockin-stdlib-and-syntax.md` / `lockin-build-and-crossplat.md` with no path — dead
  references (nothing by that name in the repo tree).
- **Fix**: one mechanical sweep; the migration note in CLAUDE.md is the map.

### 3.2 `spec-writing.md`'s canonical example teaches banned syntax
- Its §Compiler Error Format example: "`fixed[number]` is size-locked. Use `array[number]`…" —
  **square-bracket generics**, which the parser explicitly bans ("M5 unified all generic
  syntax on `<>`"). The rule that teaches how to write spec examples contains a
  won't-compile example.
- Same file: "Arrow functions only inside method calls (**`.where()`**, `.map()`…)" —
  `.where()` does not exist (registry: `filter`/`find`/`map`). Violates dot-postfix.md's own
  examples-must-use-real-operations rule.
- **Fix**: `fixed<number>` / `array<number>`; `.filter()`.

### 3.3 `maybe T` in rules vs `maybe<T>` in the compiler
- `naming.md` + `vocabulary.md` tables say `maybe T`; the parser requires `maybe<T>` (teaching
  audit A5 has the full evidence). Whichever way the language decision lands, both rule tables
  must move with it.

### 3.4 `inference.md` domains table missing shipped domains
- The registry defines 13 muted-hint domains; inference.md's "Domains This Applies To" table
  predates v0.3 and lacks `background_routing`, `parallel_groups`, `channel_capacity`,
  `auto_arc` (some appear only as stray examples in the placement-categories section). The
  rule says "if a new domain emerges, it joins this list" — four emerged; none joined.

### 3.5 `examples-structure.md` gallery inventory stale
- "primantis-orders — one file per **M1–M8 + v0.2-M1-M3**" — the directory now runs through
  v0_2_m5 and v0_3_m4 (22 files). Phrase it open-endedly ("one file per milestone") instead of
  enumerating.

### 3.6 `vue-website.md` cites a `/tmp` path as a source of truth
- §Cross-References: "`/tmp/yinz-design/yinz/project/shared.css` — design token
  source-of-truth." A machine-local, ephemeral path as SSOT in a committed rule. The tokens
  were already extracted into the `@theme` block (per the same file) — make the `@theme` block
  the declared SSOT and drop the /tmp pointer.

### 3.7 Graveyard entries cited by number, but the file has no numbers
- `plan-invariants.md` cites "graveyard Entries 1, 3, 4"; `inference.md` cites "Entry 2". The
  graveyard's 18 entries are date-titled sections with no stable numbering — insertion or
  reordering silently re-points every citation. Cite by entry *title* (the corpse name), as
  `authoritative-derivation.md` already does.

---

## Class 4 — Loading architecture ("Load when" is fiction; the whole corpus rides every turn)

- **Only `vue-website.md` has `paths:` frontmatter** (`website/**`) — the one rule that
  actually scopes. The other 14 (~1,900 lines) load unconditionally on every turn of every
  session, while five of them open with prose like "**Load when**: adding any new example
  directory…" (examples-structure), "**Load when**: any milestone plan or code change that…"
  (feature-registry), "Loaded when designing or reviewing any new stdlib module…"
  (stdlib-design), etc. The load-condition is a declaration with no mechanism — pure fiction.
- Two costs: (a) **context weight** — ~2k lines of rules before any work, diluting attention
  on the rules that DO apply (a plausible contributor to the "we slip all the time" pattern:
  the binding rule is buried under 13 non-applicable ones); (b) **false confidence** — a rule
  that believes it's conditionally loaded is written verbose ("it'll only load when
  relevant"), which is exactly backwards for an always-on rule.
- **Scoping candidates** (mechanism proven by vue-website.md):
  - `examples-structure.md` → `examples/**` + `.claude/planning/**`
  - `spec-writing.md` → `docs/reference/**`
  - `docs-checklist.md` → `docs/**`
  - `feature-registry.md` → `registry/**`, `crates/ynz-registry/**`,
    `crates/ynz-{diagnostics,typeck,parser}/**`, `.claude/planning/**`
  - `stdlib-design.md` → design-time only; scope to `.claude/planning/**` +
    `docs/internal/scratchpad/**` (stdlib design happens in plans/scratch, v0.5+)
  - `plan-invariants.md` → `.claude/planning/**`
  - `authoritative-derivation.md` → `crates/**`
- **Must stay always-on** (they bind chat-level design conversation, not file edits):
  `vocabulary.md`, `non-oop.md`, `language-design.md`, `naming.md` (post-merge), and the
  golden-rules summary in CLAUDE.md. `inference.md`/`dot-postfix.md`/`auto-promotion.md` are
  arguable — they bind design talk too; if kept always-on they should be *tightened* (Class 2
  de-dup shrinks them substantially).
- Caveat: this is a **prove-before-optimize** target (`~/.claude/rules/prove-before-optimize.md`)
  — scoping an always-loaded artifact changes what binds every future turn. The cleanup plan
  should run the falsification protocol on the scoping change, not just ship it on this
  audit's hunch.

## Class 5 — Gaps (rules that should exist and don't)

1. **A consolidated teaching-surfaces rule** — the six-document smear (GR11/12,
   teaching-mission, inference, vocabulary, spec-writing, dot-postfix) with no single
   checklist a diagnostic-writer or reviewer can load. Full spec: teaching audit §H2.7. This
   is the highest-value NEW rule.
2. **Constant naming convention** — `naming.md` defines camelCase variables and the capital
   rule, but nothing ratifies or bans `MAX_HEALTH`-style constants; meanwhile parser
   diagnostics teach `const MAX_HEALTH = 100` (teaching audit E2). GR13's "capital = type,
   zero ambiguity" is silently violated either way. One paragraph in naming.md settles it.
3. **Import-path canon** — backtick-quoted, project-root-relative, no `.ynz` suffix lives only
   in scattered parser/typeck error strings (which contradict each other — teaching audit A3).
   No rule or REF doc owns it. A short entry in vocabulary.md or the module-system REF/IMP doc
   ends the drift.
4. **Rule-header convention** — nothing specifies what a `.claude/rules/*.md` header must
   declare (load scope, SSOT-or-pointer status, cross-ref style). Cheap to add to
   docs-checklist.md; would have prevented most of Class 3/4.

## Class 6 — What's healthy (don't break it in the cleanup)

- `authoritative-derivation.md` — tight, current, cites corpses by title, names its global
  parent. The model rule.
- `plan-invariants.md` — the invariants structure + Design-Doc Alignment section is the
  strongest process rule in the repo (one stale `/design/` path aside).
- `feature-registry.md` — clear SSOT statement, carve-out policy, enforcement pointer.
- `vue-website.md` — the ONLY correctly-scoped rule; its "extends global, deltas only" header
  and "What's NOT in this stack (and why)" table are patterns worth copying corpus-wide.
- `non-oop.md`, `dot-postfix.md` bodies — current and precise (their defects are pointer-level,
  not content-level).

## Degradation thesis — confirmed, with the causal chain

Patrick's worry ("could this be degrading the project in small spots?") is not hypothetical.
The chain observed across the two audits:

> `naming.md` table + `inference.md` hover example still teach call-site `.share`/`.give`
> (pre-lock design) → every session loads both the fossil and the correction → some sessions
> follow the fossil → the registry `BackgroundLargeStructCopy` template teaches
> `background fn(value.give)` (won't compile), `.copy`-no-parens strings ship in three
> surfaces, and the hover-fallback bug models the fossil's example.

Contradictory always-loaded rules don't average out — they fork output 50/50 and the reviewer
fleet can't flag a diff that matches *one* of the two loaded standards. Fixing Class 1 is
therefore worth more than everything else in this document combined.

## Cleanup constraint — conform to the global documentation standard (Patrick, 2026-07-11)

Every artifact the cleanup produces or rewrites — rule files, the new teaching-surfaces rule,
any IMP/REF amendments — must follow the **global documentation standard**
(`~/.claude/docs/internal/implementation/IMP-documentation-system.md`, operationalized by the
global `documentation-authoring` rule). The scratchpad docs themselves are exempt (sandbox);
the deliverables are not. Concretely for this cleanup:

- **Frontmatter on every rewritten rule file** — valid YAML, every string scalar
  double-quoted, booleans/numbers bare. Today the 14 project rules open with a bare `# Title`
  and carry NO frontmatter at all. This is the same edit as Class 4: the frontmatter block is
  where `paths:` scoping lives (vue-website.md is the in-repo model), so standard-conformance
  and load-scoping land together.
- **Relative markdown links only** — no bare-text references (fixes stdlib-design's pathless
  `lockin-*.md` citations), no absolute/machine-local paths (fixes vue-website's
  `/tmp/yinz-design/...` SSOT pointer; its `~/.claude/rules/vue.md` extends-reference should
  be named-not-linked per the standard).
- **Wording standard — match force to status** (`REF-wording.md`): real gates keep
  imperative+consequence phrasing; judgment calls get reasoned-soft phrasing. Several rules
  currently inflate MUST/NEVER onto judgment-tier guidance — the de-dup pass (Class 2) is the
  natural moment to re-grade each surviving sentence.
- **Placement laws — no append-drift** during the merges: the naming.md/vocabulary.md
  consolidation (2.1) and the inference.md domain-table refresh (3.4) integrate into each
  topic's existing home and reconcile contradictions in place — never tack corrections onto
  the end.
- **One home, referenced** (Law 2): the de-dup direction in Class 2 (table lives in ONE file,
  everyone else links) is exactly the standard's model — cite the law in the rewrites so the
  next author knows the pointer is deliberate.

## Suggested order for the doc-author

1. Class 1 (contradictions) — small edits, immediate stop-loss. 1.1 and 1.2 first.
2. Class 3.1/3.2 (stale paths + banned-syntax examples) — mechanical sweep.
3. Class 2 (de-dup: one table one home) — moderate restructuring; do 2.1 with 1.1.
4. Class 5.1 (teaching-surfaces rule) — pairs with the teaching-audit remediation plan.
5. Class 4 (paths-scoping) — last, behind a prove-before-optimize falsification run.
