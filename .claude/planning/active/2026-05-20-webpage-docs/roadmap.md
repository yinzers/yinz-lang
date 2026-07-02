---
name: "webpage-docs"
plan-id: "2026-05-20-webpage-docs"
status: "active"
roadmap-id: null
session-id: []
created_at: "2026-05-20"
updated_at: "2026-05-20"
metadata:
  type: "roadmap"
legacy:
  note: "Fields below are preserved verbatim from the pre-migration .claude/plans/ ledger-format frontmatter (2026-07-01 migration to .claude/planning/). session-id history was not tracked pre-migration."
  slug: webpage-docs
  type: roadmap
  owner: Patrick Rizzardi
  status: active
  created: 2026-05-20
  last_updated: 2026-05-20
  milestones:
    - webpage-foundation
    - webpage-landing
    - webpage-roadmap-registry
    - webpage-docs-pipeline
    - webpage-examples-diagnostics
    - webpage-blog
    - webpage-launch
    - webpage-educator
    - webpage-playground
    - webpage-search-tuning
    - webpage-community
---


# Roadmap: yinzlang.com — marketing + docs site for Yinz

## Vision

A single web property at **yinzlang.com** that does three jobs without compromise:

1. **Convince** four distinct audiences (senior devs, junior devs, educators, self-teachers) that Yinz is worth their time — each finds their angle on the landing page within 10 seconds.
2. **Teach** through structurally-honest documentation: every keyword, intrinsic, and error class comes from the same registry the compiler reads. The website is downstream of the compiler, never out of sync.
3. **Persist** the project's positioning across contributors, time, and platform shifts. When the project doubles in contributors at v1.0 launch, new arrivals understand the philosophy in under an hour by reading the site.

Aesthetic is locked: warm coal background, Pittsburgh gold, steel-cream text, ember red for errors only, Space Grotesk + JetBrains Mono. Pittsburgh themed without drowning in it (412, three rivers, "Forged in Pittsburgh"). Substantive code examples Pittsburgh-themed (bridges, steel mills, the incline); trivial examples theme-agnostic.

## Why Now

Three converging signals:

- **v0.2-M4 (watch daemon) ships imminently** — the v0.2 dev-loop track is most of the way through. After v0.2-M5 (LSP completion polish), the language is publicly demonstrable end-to-end.
- **No web presence currently exists.** The compiler README on GitHub is the entire surface. Casual evaluators bounce in seconds because there's no landing page, no "why this language" pitch, no roadmap visualization.
- **Educator track is a one-shot window.** University CS curricula get locked 12+ months out. To position Yinz for v1.0 (2027 target) curriculum adoption, the educator-facing content needs to be live and credible by mid-2026 — which means the site infrastructure needs to land first.

Building the site now (pre-v1.0) is the correct sequencing: it gives the language room to gather audience while it stabilizes, and the registry-driven content pipeline ensures the site rides along with every language change for free.

## Constraints

- **Mono-repo**: site lives at `website/` in `yinzers/yinz-lang` repo. Builds in CI alongside the compiler workspace.
- **Yinz snippet drift is unacceptable**: every Yinz code snippet inline on the site lives as a real `.ynz` file in `examples/website/` that CI builds + runs. Output captured via `insta` snapshots. Drift = red CI build. Same SSOT discipline as [`registry/features.toml`](../../../../registry/features.toml).
- **No hand-maintained roadmap rows**: roadmap status pills derive from `registry/roadmap.toml` + plan-file front-matter + GitHub release state. Hardcoded version strings (`v0.2.0-m3`) are prohibited — read from [`Cargo.toml`](../../../../Cargo.toml) or release tags at build time.
- **License**: Apache 2.0 (matches the language repo).
- **Pre-v1.0 honesty**: site cannot overpromise. Status indicators must accurately reflect "Shipped / In progress / Planned" per release-tag state. Educator-facing pages must say "v1.0 curriculum-readiness target 2027" — not "use Yinz to teach today."
- **English only** at MVP. Translations are out of scope.
- **No user accounts, no auth, no CMS.** All content is MD-in-repo via `@nuxt/content`. Anything that would require a backend is out of scope.
- **All four audience tracks visible from the homepage**: sr devs, jr devs, educators, self-teachers. The teaching mission ([`docs/reference/REF-teaching-mission.md`](../../../../docs/reference/REF-teaching-mission.md)) is the differentiating moat.

## Architectural Decisions Made

Locked before any execution plan starts. Each entry: decision + WHY + what was rejected.

### Stack

- **Framework = Nuxt 4** (not Nuxt 3). — Rejected: Nuxt 3 (maintenance ends Jan 2026, security-only through July 2026; new project should land on current stable). Rejected: Astro / SvelteKit / VitePress (Patrick prefers Vue; SEO requirements need SSG which Nuxt does first-class; VitePress too narrow for the marketing/landing surface).
- **Styling = Tailwind v4 via `@tailwindcss/vite` plugin** (NOT `@nuxtjs/tailwindcss` module). — Rejected: the `@nuxtjs/tailwindcss` module doesn't yet support Tailwind v4 (tracking issue nuxt-modules/tailwindcss#820). The direct Vite-plugin path is the documented 2026 install for Tailwind v4 with Nuxt.
- **Content pipeline = @nuxt/content v3.** — Frontmatter-slug routing has a known sharp edge (default routing is file-path-based; frontmatter-slug requires a `[slug].vue` catch-all that matches by frontmatter). Workaround is acceptable and baked into the docs-pipeline milestone scope. Rejected: hand-rolled MD parser (reinventing the wheel for no benefit).
- **State management = none (no Pinia).** — Rejected: Pinia adds complexity for state that doesn't exist (search filters, mobile menu, theme toggle are all local). Will revisit if a real cross-page shared-state need emerges (unlikely for a docs/marketing site).
- **Code highlighting = Shiki via @nuxt/content's built-in integration**, fed by `crates/ynz-tmgrammar/`'s output (`tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` — verified to exist and be a real 2228-byte TextMate grammar). — Rejected: separate Shiki module (redundant when Content has it built-in). Rejected: Prism / highlight.js (no shared-grammar story with VSCode extension).
- **Search = Pagefind**, run as postbuild step against `.output/public/`. — Rejected: Algolia DocSearch (overkill, requires external account, brings in third-party JS). Rejected: client-side fuzzy-search libs (poor relevance for technical content).
- **SEO toolchain = Nuxt SEO suite** (`nuxt-schema-org`, `nuxt-robots`, `nuxt-og-image`, `@nuxtjs/sitemap`). One ecosystem, actively maintained. — Rejected: hand-rolled meta tags (Schema.org structured data is too error-prone manually).
- **Image optimization = `@nuxt/image` with `ipx` provider** (build-time only, SSG-safe, no runtime server). — Rejected: external CDN (adds dependency for no benefit at our scale).
- **Markdown extensions = MDC (Markdown Components)** for custom Vue inside MD content. — The supported `@nuxt/content` pattern for callouts, comparison tables, code-with-output blocks.
- **Analytics = TBD between GoatCounter / Umami / Plausible.** — Locked: NOT Google Analytics (privacy concerns, GDPR friction, contradicts the project's transparency stance). Decision deferred to webpage-launch milestone when concrete hosting + traffic projections exist.

### Architecture & process

- **Anti-drift commitment**: every Yinz snippet on the site is a real `.ynz` file in `examples/website/` that CI builds + runs; output captured via `insta` snapshots. This is the load-bearing design move that solves Patrick's stated docs-drift concern at the source rather than via process discipline. — Rejected: hand-maintained snippets with periodic audit (the audit always lags).
- **Registry-driven roadmap component**: `registry/roadmap.toml` (new file) + plan-file front-matter scanner + `<YRoadmap>` Vue component. Status pill logic: no release tag → Planned, draft/pre-release tag → In progress, published → Shipped. — Rejected: hardcoded HTML rows (the entire reason this site exists is to NOT have drift between docs and language state).
- **Docs source = `spec/*.md` + `design/*.md` directly from the compiler repo.** — `@nuxt/content` reads them in place. No copy step. No separate `content/` directory. Frontmatter slugs decouple URL from filesystem path so file moves don't break URLs; redirect map generated at build time. — Rejected: separate content directory with sync script (drift surface).
- **Examples page = sliced from `examples/basics/entrypoint.ynz`.** Section markers (locked format TBD in examples-diagnostics milestone) split the single growing demo file into per-feature snippets. Source remains one file (preserves the plan-invariants invariant); website exposes it sliced. — Rejected: separate per-feature demo files (violates the one-growing-demo invariant).
- **Diagnostics page = per-milestone error gallery rendered with actual compiler stdout.** Each `examples/errors/m{N}_errors.ynz` paired with its snapshot. This is the killer marketing asset for the teaching mission — way more compelling than a single fabricated mockup error. — Rejected: fabricated diagnostic examples (the original design used `error[E0142]` which doesn't even match Yinz's actual diagnostic format).
- **Path = `website/` at repo root**, sibling to `crates/`, `tooling/`, `examples/`, `design/`. — Rejected: `tooling/website/` (the site has a different lifecycle and stakeholder than compiler-adjacent dev tooling; sibling placement reflects that).
- **Verification rule (per Patrick, locked in design-review chat)**: every Yinz code snippet in the site gets verified against shipped Yinz before implementation. This rule applies BEFORE the anti-drift CI mechanism — it's the manual gate during execution; CI is the automated gate after.
- **Framing rule (per Patrick, locked this chat)**: no "deferred" framing for site features. Everything is a future milestone in this roadmap. The roadmap is the commitment; execution plans materialize one milestone at a time.

### Design (visual)

- **Tokens from `shared.css` `:root` vars get extracted into Tailwind theme** at the foundation milestone — BUT with a carve-out: **color tokens, radii, and breakpoints transfer; font tokens DO NOT.** The design prototype used Space Grotesk; the locked brand uses Anton/Bebas Neue or similar athletic display + complementary body face (see "Aesthetic locked" below). The HTML/CSS in `/tmp/yinz-design/yinz/project/` is a visual reference for layout/spacing/color, not for typography. Component CSS in the prototype becomes Vue SFC styles or Tailwind classes.
- **Hosting = DigitalOcean App Platform** (free static site tier). Locked. — Supersedes Cloudflare Pages (was the original locked choice; switched in M1 execution due to simpler Dockerfile-based deploy + DO's static site tier being equivalent). Rejected: Vercel/Netlify (trial-cliff pricing risk for OSS), GitHub Pages (no Nuxt SSR/edge story if we ever need it), self-host (operational overhead unjustified for a docs site).
- **Pittsburgh theming applies to substantive code examples**. Trivial examples (simple math, type demonstrations) stay theme-agnostic. Locked substantive theme: **Pittsburgh Pirates baseball** (player stats, roster management, pitcher records). Compare example uses **Pirates roster with `Pitcher extends Player`** — demonstrates Yinz's data-inheritance flatness (`cole.name`, `cole.hits`) vs Rust's forced composition tax (`cole.player.name`, `cole.player.hits`). Hero example stays three-rivers themed (bridges with simple iteration) to provide thematic variety vs the compare's deep-Pirates dive.
- **Examples folder restructure is in-flight in a sibling chat** as of 2026-05-20. The roadmap references the artifacts semantically (the canonical growing demo file, the per-milestone error galleries, the new website-snippets directory) rather than by exact path — path lockdown happens at the relevant execution-plan stage once the restructure settles. Foundation milestone gates on knowing the final paths; later milestones inherit them.
- **Aesthetic locked — LOUD Pittsburgh industrial register, NOT polished-modern-tech** (pivoted from initial "confident-technical with subtle Pittsburgh" lock on 2026-05-20). Brand registers as stand-out, loud, place-based — deliberately distinct from the Rust/Go/TypeScript/Zig/Swift "quiet modern tech" cohort. Locks:
  - **Colors** (unchanged): `#14110d` background, `#FFD23F` + `#FCB514` golds, `#F2EBDC` text, `#C8442A` ember for errors only.
  - **Logo mark**: block-Y in Pittsburgh Pirates' "block P" visual lineage — chunky athletic letterform, Pirates gold on coal, NOT slab-serif sports varsity (that crosses into team-identity territory) but in the family of mid-century block-letter civic/industrial marks. Patrick generates final SVG via AI image-gen then vectorizes.
  - **Display typography**: athletic heavy display face — Anton, Bebas Neue, or similar (Google Fonts free). Exact face picked at foundation milestone. NOT Space Grotesk (that was the original lockdown; reversed here for the brand pivot).
  - **Body typography**: complementary face that pairs with the chosen display — Inter, Source Sans, or similar geometric body face. Pick at foundation milestone after display font lands.
  - **Code typography** (unchanged): JetBrains Mono.
  - **Pittsburgh touches**: still subtle in COPY (412, three rivers, "forged in Pittsburgh"), now LOUD in TYPOGRAPHY. The brand isn't quiet about being from Pittsburgh; it's loud about it.
  - **Forbidden**: no emoji, no AI-slop gradients, no bridge SVGs in the layout itself, no pierogi, no literal pirate imagery (block-Y is the only Pirates reference and it's typographic).
- **Brand voice on copy**: "confident technical with subtle Pittsburgh references" stays. The visual gets louder; the copy stays restrained. The contrast is intentional — loud-Pittsburgh-visuals + restrained-Pittsburgh-copy = a brand that knows what it is without being annoying about it.

## Open Architectural Questions

Decisions NOT yet made that block at least one execution plan.

- **Analytics tool**: GoatCounter / Umami / Plausible? Blocks `webpage-launch` (analytics integration). Defer to launch milestone — concrete hosting + privacy-policy template need to be in place first.
- **Logo SVG**: Patrick wants an AI-generated mark to complement the existing wordmark (the "yinz + golden-square dot" wordmark from the design is kept as the lockup typography; the AI-generated mark sits next to it). Blocks `webpage-foundation` (component needs the SVG asset). Claude is providing AI image-gen prompt drafts in the chat that produced this roadmap — Patrick generates, picks, vectorizes, drops the SVG into the foundation milestone.
- **Educator-page CTA + content depth**: when `webpage-educator` lands (post-blog), what's the conversion action? Mailing list signup? Syllabus PDF download? "Talk to Patrick" form? Needs decision once 3+ educator-track blog posts exist to anchor the page. Defer to that milestone.
- **OG image strategy**: per-page generated via `nuxt-og-image` (dynamic templates) vs hand-designed for landing + one fallback for everything else? Blocks `webpage-launch` aesthetic polish but not foundation. Defer to launch milestone.

## Milestones

Phase 1 (MVP) ships in order. Phase 2 milestones unblock after Phase 1 lands and proves direction.

### Phase 1 — MVP

#### Milestone 1: webpage-foundation
**Value delivered**: empty Nuxt 4 site deploys to a staging URL. Design tokens extracted from `shared.css` into Tailwind v4 theme. Core component primitives (`<YButton>`, `<YPill>`, `<YCard>`, `<YCallout>`, `<YCode>`, `<YNav>`, `<YFooter>`) render with locked aesthetic. Shiki + ynz-tmgrammar wired (a `.ynz` snippet in a stub page renders with Yinz syntax colors matching VSCode). SEO suite + `@nuxt/image` configured. Build pipeline added to repo CI.
**Execution plan**: `webpage-foundation` (status: planned)
**Depends on**: nothing — first up
**Rough scope**: scaffold Nuxt 4 in `website/`, configure Tailwind v4 via Vite plugin, extract tokens from `shared.css`, build the six primitive components, wire Shiki to consume `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json`, set up the SEO suite, configure `@nuxt/image` with `ipx`, add a CI job to build the site, deploy to staging. Hosting decision locked in this milestone.

#### Milestone 2: webpage-landing
**Value delivered**: yinzlang.com landing page complete — hero, pillars, vs-Rust compare, teaching error demo (using a real diagnostic), auto-promotion section, hardcoded-but-correct roadmap rows, install commands, footer. Every Yinz snippet on the page lives as a real `.ynz` file in `examples/website/`, builds in CI, snapshot-verified.
**Execution plan**: `webpage-landing` (status: planned)
**Depends on**: Milestone 1
**Rough scope**: build landing page sections one at a time per the design spec. Critical fixes from design verification: all strings to backticks, drop `import { print } from std`, replace fabricated `error[E0142]` with a real diagnostic from `m4_errors.ynz` or similar, rewrite compare example using steel-mill-worker `extends` theme (no `.where()`, no lambdas, no `.filter()` — pure shipped Yinz), add 13th Golden Rule (capital letter = type). Install commands use a placeholder URL marked "v1.0 install infra pending" rather than fabricating `install.yinz.dev`. Roadmap section uses hardcoded rows here — they get replaced by Milestone 3's component.

#### Milestone 3: webpage-roadmap-registry
**Value delivered**: roadmap rows on the landing page are no longer hardcoded — they read from `registry/roadmap.toml` + plan-file frontmatter + GitHub release tag state. Status pill logic implemented per Patrick's design-comment spec (no tag → Planned, draft/pre-release → In progress, published → Shipped). Adding a new version to the roadmap means editing `registry/roadmap.toml`, never the HTML.
**Execution plan**: `webpage-roadmap-registry` (status: planned)
**Depends on**: Milestone 2
**Rough scope**: design `registry/roadmap.toml` schema (one entry per version, with: version string, name, description, expected milestone slug list, optional notes). Write a build-time scanner that joins this with `.claude/plans/{active,paused,done}/*.md` front-matter (for in-progress signal) and `gh release list` output (for shipped signal). Build `<YRoadmap>` Vue component that consumes the joined data. Replace the hardcoded landing-page roadmap rows with this component. Update [`.claude/rules/feature-registry.md`](../../../rules/feature-registry.md) to mention the new TOML file.

#### Milestone 4: webpage-docs-pipeline
**Value delivered**: `/docs` route renders all `spec/*.md` and `design/*.md` files. Sidebar nav generated from file tree + frontmatter weighting. Pagefind search integrated and indexes docs. Frontmatter-slug routing implemented via `[slug].vue` (file moves don't break URLs). Redirect map generated at build time for any slug changes. MDC components available (callouts, comparison tables, code-with-output blocks).
**Execution plan**: `webpage-docs-pipeline` (status: planned)
**Depends on**: Milestone 1 (foundation primitives), independent of Milestones 2/3
**Rough scope**: configure `@nuxt/content` to read from `../spec/` + `../design/` (relative path from `website/`). Build the `[slug].vue` catch-all that resolves by frontmatter `slug:` field. Build sidebar nav generator. Wire Pagefind as a postbuild step. Implement redirect map: scan for changed slugs vs previous build, generate `_redirects` or equivalent. Build a small library of MDC components for use in MD content (`<Callout>`, `<Diff>`, `<CodeOutput>`).

#### Milestone 5: webpage-examples-diagnostics
**Value delivered**: `/examples` page renders sliced sections of `examples/basics/entrypoint.ynz` with their actual `insta` snapshot output. `/diagnostics` page renders each milestone's error gallery (`examples/errors/m{N}_errors.ynz`) paired with the real compiler stdout. Section-marker convention locked. Build-time slicer script lives in `website/build/`. Both pages literally cannot drift: changes to the source files or the snapshots propagate on next build.
**Execution plan**: `webpage-examples-diagnostics` (status: planned)
**Depends on**: Milestone 1
**Rough scope**: design section-marker comment convention (e.g., `// ============ M4: SHAPES ============`). Update `examples/basics/entrypoint.ynz` with markers (does NOT split the file — preserves the one-growing-demo invariant). Write build script that slices the file by markers, joins each slice with its snapshot stdout, and emits structured data the page consumes. For `/diagnostics`: build script reads each `m{N}_errors.ynz` + its snapshot, emits per-error-block data with the WHAT/WHAT-INSTEAD/WHY format preserved. Both pages get build-time-generated content; nothing dynamic at runtime.

#### Milestone 6: webpage-blog
**Value delivered**: `/blog` route works — index page with filter chips, post template with prose components (chevron bullets, callouts, blockquotes, tables, code-with-Yinz-highlighting), RSS feed. Two seed posts: a "why the package manager is late" rationale post (rewritten with verified-correct Yinz syntax) and a milestone announcement template. Blog separate from `/docs` in URL space.
**Execution plan**: `webpage-blog` (status: planned)
**Depends on**: Milestone 4 (`@nuxt/content` already integrated)
**Rough scope**: configure a separate `@nuxt/content` source for `content/blog/*.md`. Build `<PostIndex>`, `<PostTemplate>`, `<FilterChips>` components matching the design's prose styling. Generate RSS feed at build time. Migrate the two seed posts from the design HTML (with code corrections per the verification pass). Wire the seed posts' tags to the filter chips. Lock filter taxonomy: `milestone | design | rationale | forge-notes` (matches the design's chip set).

#### Milestone 7: webpage-launch
**Value delivered**: yinzlang.com goes live. DNS configured. Sitemap.xml + robots.txt + per-page OG images deployed. Analytics integrated (tool selected from GoatCounter/Umami/Plausible). Privacy policy + terms of service pages live (boilerplate appropriate for an OSS project with anonymous analytics). Launch checklist verified.
**Execution plan**: `webpage-launch` (status: planned)
**Depends on**: Milestones 2–6 (all MVP content must be live before public launch)
**Rough scope**: configure DNS at the registrar. Final analytics tool selection. Wire the analytics script (privacy-aware, no PII, no consent banner needed for the selected tools). Generate per-page OG images via `nuxt-og-image` with dynamic templates. Write privacy policy + ToS. Final pass on sitemap.xml, robots.txt, canonical URLs, OG meta on every page. Launch checklist (Schema.org validates, Lighthouse score targets met, all internal links resolve, all external links open in new tab where appropriate, mobile rendering verified on real devices, social-card previews validated on Twitter/Mastodon/LinkedIn).

### Phase 2 — Post-MVP (later milestones, not deferred)

These ship after Phase 1 lands and proves direction. None are speculative — each has a concrete trigger.

#### Milestone 8: webpage-educator
**Value delivered**: `/educators` (or `/teach`) landing page targeting university CS faculty, high school programming teachers, and vo-tech program coordinators. Anchors 3+ educator-track blog posts. CTA = curriculum-readiness mailing list signup. Sample syllabus outline downloadable.
**Execution plan**: `webpage-educator` (status: planned)
**Depends on**: Milestone 6 + 3+ educator-track blog posts published
**Rough scope**: cannot ship until there are 3+ educator-track blog posts to anchor the page (the "what would teaching Yinz actually look like" essays). Mailing list infrastructure picked when this milestone starts (current candidates: Buttondown, Listmonk self-hosted, or a simple form-to-email forwarder). Page content: why-Yinz-for-teaching pillar, concrete pedagogical advantages tied to specific compiler features (the diagnostic format, the muted-hint IDE surface, the lack of GC for "what is memory" lessons), sample curriculum outline (10-week intro CS, or 1-semester systems intro), instructor resource section (slides? exercises? grading rubrics?), mailing list signup.

#### Milestone 9: webpage-playground
**Value delivered**: in-browser Yinz playground. User types Yinz code, hits run, sees output or compiler diagnostics. No server roundtrip — compiler runs in-browser via WASM.
**Execution plan**: `webpage-playground` (status: planned)
**Depends on**: WASM compiler target shipping (Yinz M9+ language work — this is a language-level dependency, not a site-level one). Until the compiler can target WASM, this milestone is blocked at the language layer.
**Rough scope**: WASM-compile the Yinz compiler binary. Embed it in a Monaco / CodeMirror editor on the page. Wire run button to invoke the WASM compiler against the editor buffer and pipe stdout/stderr to a results panel. Add Yinz syntax highlighting in the editor using Shiki (or Monaco's TextMate-grammar support). Sample programs in a left-side picker. Permalink-by-URL-hash for shareable snippets.

#### Milestone 10: webpage-search-tuning
**Value delivered**: Pagefind search relevance tuned based on real-usage analytics. Common queries indexed correctly. Synonym handling added if needed (e.g., `class` → `shape`, `enum` → `options`).
**Execution plan**: `webpage-search-tuning` (status: planned)
**Depends on**: 3+ months of post-launch analytics data
**Rough scope**: review Pagefind config against real queries (which terms surface no results? which surface wrong results?). Tune index weights for headers vs prose vs code blocks. Add custom synonym layer for banned-jargon → Yinz-term redirects ("class" → suggest "shape", "enum" → suggest "options"). Surface "Searching for X? You probably want Y" banner on top of empty results for known jargon mismatches.

#### Milestone 11: webpage-community
**Value delivered**: contributor-facing pages — governance docs, expanded code of conduct, contributor showcase, "ways to help" page (issues to grab, features to claim, doc gaps), community spaces (GitHub Discussions link + any Discord/Matrix that emerges).
**Execution plan**: `webpage-community` (status: planned)
**Depends on**: Milestone 7 (launch) + organic community signal (people asking "how do I contribute?")
**Rough scope**: write governance doc (project lead, decision-making process, how proposals work, RFC process if one emerges). Expand code of conduct with reporting + enforcement. Contributor showcase auto-populated from `git shortlog` or GitHub API. "Ways to help" page links curated issues with `good-first-issue` and `help-wanted` labels. Community spaces section: at minimum GitHub Discussions; add others as they're spun up.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Anti-drift CI mechanism is slow (every `examples/website/*.ynz` build in CI on every site change) | Medium | Medium — slow CI = developer friction | Cache `cargo build` artifacts in CI; only rebuild `examples/website/` snippets if their source files changed (not every site-only edit) |
| `@nuxt/content` frontmatter-slug routing workaround is fragile (the `[slug].vue` catch-all might miss edge cases or conflict with file-path-based routes) | Medium | Medium — could ship broken docs URLs | Build a comprehensive routing test suite during docs-pipeline milestone; verify every `spec/*.md` and `design/*.md` resolves correctly; CI gate on routing tests |
| Pittsburgh theming reads as gimmicky to non-Pittsburgh audiences | Low | Medium — could undermine "serious systems language" positioning | Cap Pittsburgh references at 3-4 per page; never in code that demonstrates language semantics; user testing with non-Pittsburgh devs during landing milestone polish |
| Compare example doesn't dramatically enough show Yinz's readability win because shipped methods are limited (no `.filter()`, no lambdas) | High | High — compare section is the marketing core | Mitigation locked: use `extends`-based steel-mill-worker example where Rust's lack of struct inheritance forces composition (`yinzer.worker.shift_hours` vs `yinzer.shiftHours`) — the readability win is per-line, not just line-count |
| Roadmap registry diverges from what's actually in `.claude/plans/` (e.g., plan files moved without updating `roadmap.toml`) | Medium | Low | Registry milestone adds a Bouncer check: PRs that move plan files between `active/`, `paused/`, `done/` without touching `roadmap.toml` get flagged |
| Educator-page CTA underperforms because curriculum-readiness target year (2027) is too far out for educators to plan around | Medium | Medium — could mean educator mailing list never grows | Frame the CTA as "be first to know when curriculum-ready" rather than "use Yinz today" — sets accurate expectations; mailing list value is in early-warning, not immediate adoption |
| Site goes live before v0.2 v0.2-M5 (LSP completion) ships, and the install path doesn't work for new users yet | High | High — would torpedo first-impressions if highlighted on homepage | Install section gates on "v0.2 LSP + extension shipped" — until that lands, install section says "v0.2 final ships [date]; track progress on the roadmap" rather than offering a broken install command |
| Hosting provider price shock or feature regression mid-project | Low | Medium — could force migration | Pick hosting with multiple viable alternatives (DigitalOcean App Platform → can migrate to Netlify or self-host on Coolify with minimal config change); avoid platform-lock-in like Vercel proprietary edge functions |
| Tailwind v4 has unreported bugs that bite us late (it's been stable since Jan 2025 but the ecosystem is still catching up) | Low | Medium | Stick to Tailwind v4 utility classes only — avoid v4-specific features (`@theme` directive nuances, CSS-first config edges) until docs-pipeline milestone proves them stable in our context |

## Out of Scope

Explicitly NOT part of this roadmap. Pre-empts scope creep when a chat reads this 3 months from now and thinks "we should add X."

- **Interactive playground requiring runtime backend.** The future `webpage-playground` milestone is in-browser WASM only — no server roundtrip, no compiler-as-a-service. A hosted compiler service would require auth, sandboxing, rate limiting, abuse handling — all of which are out of scope for an OSS docs site.
- **User accounts / authentication / personalization.** No "save your favorite examples", no "your learning progress", no "your starred docs pages." Browser localStorage only if a feature genuinely needs persistence.
- **E-commerce or paid offerings.** Yinz is OSS. No "buy a hardcover book," no "enterprise support tier."
- **Translations into non-English languages.** A real maintenance burden with no community signal yet. If a translation community emerges, revisit.
- **Forum / community discussion platform hosted on yinzlang.com.** GitHub Discussions exists and is appropriate; no custom forum.
- **Custom CMS / WYSIWYG editing.** All content is MD-in-repo. Anyone who can't write Markdown can submit a PR through GitHub's web UI editor.
- **A/B testing infrastructure.** No experiments. Decide content strategy by talking to users and reading analytics, not by splitting traffic.
- **Cookie-consent banners.** The chosen analytics tools (GoatCounter/Umami/Plausible) all run consent-free under GDPR — no banner needed. If we ever add a tool that requires consent, that tool gets rejected.
- **Per-version docs / docs versioning.** Pre-v1.0, docs reflect current main. At v1.0, evaluate versioned docs (`v1.0`, `latest`) — but until then, one set of docs tracking HEAD is correct.
- **Yinz code embedded as live-runnable in docs** (different from the `/playground` page). Docs show static code with verified-correct snapshots; no inline "run this" buttons in prose docs. Playground page (Milestone 9) is the place for runnable code.
- **Hand-maintained code snippets anywhere.** Every Yinz snippet on the site is anchored to a real `.ynz` file in `examples/website/`. If a piece of marketing copy "needs" an inline snippet that can't be a real file, the marketing copy is wrong.

## Open Questions for Patrick

Strategic decisions needed before specific milestones can start. Separate from architectural questions above.

1. **Hosting platform** — ANSWERED: DigitalOcean App Platform (free static site tier, locked in M1 execution; supersedes original Cloudflare Pages choice).
2. **Logo lockdown** — ANSWERED: block-Y in Pittsburgh Pirates' "block P" visual lineage (Patrick's preference locked 2026-05-20 after AI iteration). Pittsburgh industrial register adopted across the whole brand — display typography swaps from Space Grotesk to Anton/Bebas Neue or similar athletic display face. Patrick generates final block-Y SVG via AI image-gen then vectorizes; drops into foundation milestone as the `logo-mark.svg` asset.
3. **Compare example concept** — ANSWERED: Pittsburgh Pirates roster with `Pitcher extends Player`. Demonstrates Yinz's flat-field-access win over Rust's composition tax.
4. **Hero example concept** — ANSWERED: Pittsburgh-themed, exact concept TBD at landing milestone; three-rivers Bridge iteration is the default unless a better Pittsburgh-themed hook emerges during execution.
5. **Pre-v1.0 educator-page strategy** — confirm: educator page is a future milestone (Milestone 8), unblocked when 3+ educator-track blog posts exist to seed it. Framing on the page when it ships: "v1.0 curriculum-readiness target 2027" — sets accurate expectations.
6. **Brand voice on landing page** — design copy reads "confident technical with subtle Pittsburgh references." Locking that unless Patrick pushes back.

---

## Manual review pass (roadmaps don't go through plan-reviewer)

Checked against Step 0b roadmap rules:

- ✅ Vision is outcome-focused (audiences served, capabilities unlocked) not implementation-focused
- ✅ Why Now names concrete triggers (v0.2-M4 nearing ship, no current web surface, educator window)
- ✅ Constraints are real (mono-repo, anti-drift, Apache 2.0) — not platitudes
- ✅ Architectural decisions each name the WHY and what was rejected
- ✅ Open architectural questions each block a specific milestone
- ✅ Milestones each deliver standalone value (foundation = deploys, landing = pitches the language, docs = docs work, etc.)
- ✅ Out-of-scope explicit with rationale for each
- ✅ No file:line anchors (correct for roadmap level)
- ✅ No phase-level acceptance criteria (correct for roadmap level)
- ✅ Risks table covers strategic risks (whole initiative could fail because…) not implementation risks
- ✅ Each child execution plan referenced will set `roadmap: webpage-docs` in its frontmatter (verified in milestone format)

Roadmap is ready for Patrick's approval.

## Capability Ledger

(merged from the pre-migration companion `capability-ledger.md` file — 2026-07-01)

Every capability the initiative will deliver maps to a milestone. The foundation milestone shipped (child `webpage-foundation` is `done`); Notes records what it delivered. The remaining ten milestones have no child execution plan yet — `NEEDS-PLANNED`, owned by their roadmap-sequenced milestone slug (run `/plan` when each is picked up).

| Capability | Owning milestone | Status | Notes |
|---|---|---|---|
| Site foundation — Nuxt 4 deploy to staging, design tokens → Tailwind v4 theme, core component primitives, Shiki + ynz-tmgrammar wiring, SEO + image config, CI pipeline | webpage-foundation | done | Delivered (child `webpage-foundation`, status: done). |
| Landing page — hero, pillars, vs-Rust compare, teaching error demo, auto-promotion section, roadmap rows, install commands; every snippet a real `.ynz` file built in CI | webpage-landing | NEEDS-PLANNED | No child plan yet. |
| Roadmap registry — landing-page roadmap rows read from `registry/roadmap.toml` + plan frontmatter + GitHub release state; status-pill logic; no HTML edits to add a version | webpage-roadmap-registry | NEEDS-PLANNED | No child plan yet. |
| Docs pipeline — `/docs` renders `spec/*.md` + `design/*.md`, file-tree sidebar nav, Pagefind search, frontmatter-slug routing, build-time redirect map, MDC components | webpage-docs-pipeline | NEEDS-PLANNED | No child plan yet. |
| Examples + diagnostics pages — `/examples` renders sliced `.ynz` with insta snapshots, `/diagnostics` renders per-milestone error galleries with real stdout; cannot drift | webpage-examples-diagnostics | NEEDS-PLANNED | No child plan yet. |
| Blog — `/blog` index with filter chips, post template with prose components, RSS, two seed posts | webpage-blog | NEEDS-PLANNED | No child plan yet. |
| Launch — yinzlang.com live, DNS, sitemap/robots/OG images, analytics, privacy + ToS, launch checklist | webpage-launch | NEEDS-PLANNED | No child plan yet. |
| Educator landing — `/educators` page targeting CS faculty/teachers, 3+ educator blog posts, curriculum-readiness mailing list, sample syllabus | webpage-educator | NEEDS-PLANNED | No child plan yet. |
| Playground — in-browser Yinz playground, compiler runs via WASM, no server roundtrip | webpage-playground | NEEDS-PLANNED | No child plan yet. |
| Search tuning — Pagefind relevance tuned on real analytics, common queries indexed, synonym handling | webpage-search-tuning | NEEDS-PLANNED | No child plan yet. |
| Community — governance docs, code of conduct, contributor showcase, "ways to help", community spaces | webpage-community | NEEDS-PLANNED | No child plan yet. |
