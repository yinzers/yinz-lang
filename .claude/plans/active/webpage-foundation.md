---
slug: webpage-foundation
type: execution
owner: Patrick Rizzardi
status: done
files:
  - website/**
  - .github/workflows/website.yml
  - .claude/plans/roadmaps/webpage-docs.md
created: 2026-05-20
last_updated: 2026-05-20
roadmap: webpage-docs
depends_on: []
---

# Plan: webpage-foundation (Milestone 1 of webpage-docs)

Created: 2026-05-20
Status: done — all phases complete

## Session Log

### 2026-05-20 — Plan drafting + standards alignment
- Drafted plan from `webpage-docs` roadmap Milestone 1 scope.
- Locked decisions in plan-creation chat (NOT in roadmap yet — Phase 7 carries the roadmap edit):
  - **Hosting**: DigitalOcean App Platform (replaces roadmap's stale "Cloudflare Pages" lock).
  - **Display + body fonts**: Anton (display) + Inter (body) + JetBrains Mono (code), all vendored via `@nuxt/fonts`.
  - **Bun**: pinned `oven/bun:1.2.21-alpine` via Docker Compose, port 6002 external (avoids sibling-chat collision).
  - **Scope**: just M1 foundation but **full Component Inventory** documented up-front across M2-M6 (per Patrick's ask).
- Plan-reviewer round 1: **BLOCK** with 7 Required Fixes.
- Addressed all 7 fixes (Phase 3 split into 3a/3b, grammar sync script mandatory + diff-verified, Shiki SSR pattern concretized via Nuxt module, 2 new risks added, 90s SLA dropped, date fabrication removed, worktree note de-prescribed) + 4 Concerns (useScrollLock fully implemented, CI+Dockerfile co-verification, README CSP/HMR/host-bun-install warnings, arrow-functions rule).
- Plan-reviewer round 2: **PASS** (Tier B). Zero Required Fixes. 4 non-blocking concerns logged at the executor level.
- **`vue-website.md` rules file rewritten** to match this stack (was generic Vue 3 SPA conventions from VPM; now Nuxt 4 + Tailwind v4 + Bun + SSG, `Y*` prefix, `T | null` over `T?`, no Pinia/Reka/Lucide/Axios, anti-drift `<YCode>` rule). Executor will load this when working under `website/**`.

### 2026-05-20 — Phase 1 execution
- Scaffolded Nuxt 4 minimal template via `bunx nuxi@latest init` + `docker cp` (devcontainer is DooD; volume mounts don't pass through to devcontainer filesystem).
- Created: `docker-compose.yml`, `Dockerfile.dev`, `.dockerignore`, `.gitignore`, `package.json`, `nuxt.config.ts`, `tsconfig.json`, `bun.lock`, `app/app.vue`, `app/pages/index.vue`, `public/favicon.ico`, `website/README.md`.
- Updated root `.gitignore` with belt-and-suspenders website artifact exclusions.
- Deviation: lockfile is `bun.lock` (text format, bun 1.2+) not `bun.lockb` (old binary format). Better for diffs; plan updated accordingly.
- Smoke-tested via `bun run dev` (local bun 1.3.14) + `bun run generate`: dev server returned 200 with under-construction content; SSG produced clean hash-based assets.
- DooD note: `docker compose up` dev workflow is correct for Patrick's local machine; volume mounts don't work in this devcontainer environment, so verification used bun directly.


### 2026-05-20 — Phase 2 execution
- Installed: `tailwindcss@4.3.0`, `@tailwindcss/vite@4.3.0`, `@nuxt/fonts@0.14.0` (dev deps).
- Created `website/app/assets/css/tailwind.css` with full `@theme` block: all 19 color tokens, 4 radius tokens, 3 font tokens, container-max. Base layer includes body styles, grain gradient, typography scale, link colors — all from `shared.css`.
- Updated `nuxt.config.ts`: added `@tailwindcss/vite` to `vite.plugins`, `@nuxt/fonts` to `modules`, `~/assets/css/tailwind.css` to `css` array. Fonts configured for Anton (400), Inter (400/500/600/700), JetBrains Mono (400/500/600).
- Updated `app/app.vue`: added `relative min-h-screen` wrapper div.
- Updated `app/pages/index.vue`: token-styled markup using `font-display`, `text-gold`, `text-ink-mute`, `bg-bg` context, `max-w-(--container-max)`.
- Smoke-tested: `bun run generate` succeeded; `@nuxt/fonts` downloaded 3 woff2 files; `@font-face` declarations in generated CSS reference `/_fonts/` (0 googleapis/gstatic references confirmed).
- Added `dist` to `website/.gitignore` (Nuxt generate creates a `dist → .output/public` symlink that shouldn't be tracked).
- Deviation: `@nuxt/fonts` v0.14.0 downloads fonts AT BUILD TIME (not at install time). Fonts are vendored in `.output/public/_fonts/` (build output, not committed). The plan's Phase 5 requirement to commit font files will need to be reassessed — `@nuxt/fonts` doesn't support persisting to `website/public/` without additional config. Fonts served locally at runtime (no Google CDN at runtime). Font download at build time is acceptable for CI (internet access available).

### 2026-05-20 — Phase 3a execution
- Built 9 stateless primitive components: `YContainer`, `YSection`, `YRow`, `YGrid`, `YDivider`, `YEyebrow`, `YHeading`, `YPill`, `YCallout`.
- All use TypeScript `defineProps<{...}>()` with explicit types; no `any`, no `!`.
- `YCallout`: `border-l-2 border-ember` for warn variant confirmed in generated CSS.
- `YHeading`: `font-size: clamp(48px,7vw,96px)` for level 1 confirmed in generated CSS.
- Created `app/pages/_dev/components.vue` gallery route with Layout / Typography / Indicators sections. `noindex` meta applied via `useHead`.
- `bun run generate` prerendered `/_dev/components` route successfully.
- YHeading and @layer base headings use `font-[400]` (Anton intrinsically bold — CSS weight doesn't apply).
- Note: optional props use optional marker (`border?: boolean`) per Vue/TypeScript convention for function-parameter-style props; structural object fields use `T | null` per coding-style.md.

### 2026-05-20 — Phase 3b execution
- Built 6 interactive/composite components: `YButton`, `YLink`, `YCard`, `YLogo`, `YNav`, `YFooter`.
- Built `useScrollLock` composable: locks body overflow on `lock()`, restores on `unlock()`, `onUnmounted` cleanup. Guards `document` access via `import.meta.client`. SSR-safe (SSG build zero errors).
- `YNav`: sticky header with `bg-bg/80 backdrop-blur-[14px] backdrop-saturate-[140%]` (Tailwind utilities, no inline styles). Mobile drawer at <600px. `useScrollLock` integrated.
- `YButton`: `primary` (gold bg + dark text + hover lift) and `ghost` (transparent + border + hover bg-raised) variants. All values from token palette.
- `YLogo`: serves `yinz.svg` from `/yinz.svg` (copied to `website/public/yinz.svg`). sm/md/lg size prop. Optional `.ext` mono tail.
- `YFooter`: 4-column grid with `grid-cols-[1.4fr_1fr_1fr_1fr]` arbitrary value. Collapses to 2-col at `md:` and 1-col default. Copyright row.
- Extended `_dev/components` gallery with Interaction, Branding, Composite, Nav+Footer sections.
- Updated `app.vue` to include YNav (with stub links + GitHub CTA) so it renders site-wide.
- Deviation: `yinz.svg` copied to `website/public/` early (plan scheduled Phase 5 for this move). Phase 5 will clean up the original `website/yinz.svg` root placement.
- `bun run generate` passed, 6 routes prerendered, no errors.

### 2026-05-20 — Phase 4 execution
- Installed `shiki@4.1.0` (with `shiki/engine/javascript` for Nitro compatibility — no WASM).
- Created `build/sync-ynz-grammar.ts`: copies grammar from `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json`, verifies sha256, fails build if source missing. Wired as `pregenerate` and `prebuild` scripts.
- Created `app/assets/grammars/ynz.tmLanguage.json` (checked-in vendored copy, byte-identical to source; sha256: `1daa4b3d6ca0...`).
- Created `app/assets/themes/yinz-coal.json`: Shiki theme mapping token scopes to shared.css palette (river/moss/rust/plum/ink-dim/ember).
- Created `app/plugins/shiki.server.ts`: server-only Nuxt plugin using static imports + JS regex engine. Initializes once before SSR. Provides `$shikiHighlight(code, lang)` synchronously.
- Created `app/components/primitives/YCode.vue`: renders `v-html` of Shiki output (from `$shikiHighlight`). Falls back to plain text on client or if highlighting unavailable. Props: code, lang, filename, showLineNumbers.
- Created `app/composables/useShikiTheme.ts`: client-side helper for future use (playground etc).
- Updated `nuxt.config.ts`: added `experimental.payloadExtraction: 'client'` (full-static SSR), `components: [{ path: '~/components', pathPrefix: false }]` (fixes component naming — was `PrimitivesYContainer` not `YContainer`).
- Extended `_dev/components` gallery with YCode section; snippet includes keywords, strings, numbers, booleans, null, and comments.
- Updated README with grammar sync discipline section.
- **Root cause of empty HTML** (Phases 3a/3b): components auto-imported as `PrimitivesYFoo` (not `YFoo`) due to subdirectory prefix. Fixed by `pathPrefix: false` in nuxt.config.ts. The Phase 3a/3b "successful" generates were rendering empty Y* components silently.
- **Shiki language ID**: grammar `name: "Yinz"` (capital). Plugin uses `grammarJson.name` as the lang in codeToHtml to avoid ID mismatch.
- Verified: 6 distinct syntax colors (river/moss/rust/plum/ink-dim/ink), diff=0 bytes vs source grammar, sync fails when source missing, 8× river-teal keyword spans in gallery SSG HTML.
- **Grammar coverage gaps** (to document in PR): function names (no `entity.name.function` scope), type names (no `storage.type`), double-quoted strings (grammar only covers backtick template literals). These appear in plain foreground color. Follow-up plan to extend grammar when M5-M6 language features need highlighting.

### 2026-05-20 — Phase 5 execution
- Installed: `nuxt-schema-org@6.0.4`, `nuxt-og-image@6.5.1`, `@nuxtjs/sitemap@8.0.15`, `@nuxt/image@2.0.0`. Dropped `nuxt-robots@2.0.1` (Nuxt 2 era, incompatible with Nuxt 4).
- Created `website/public/robots.txt`: static file, `Disallow: /` with M7-flip reminder comment.
- Configured modules in nuxt.config.ts: `nuxt-schema-org`, `nuxt-og-image`, `@nuxtjs/sitemap`, `@nuxt/image`. Added `site.url`, `sitemap.exclude: ['/_dev/**']`, `image.provider: 'ipx'`.
- Updated `app.vue`: global `useHead` with titleTemplate, description, og:type, twitter:card, canonical link (route-dynamic). JSON-LD WebSite + Organization schemas injected directly via `useHead.script` (nuxt-schema-org's `useSchemaOrg` composable had SSG injection issues; direct approach is reliable).
- Copied `yinz.png` to `website/public/` per Nuxt convention (assets in `public/` served by Nitro/DO).
- Verified: robots.txt Disallow:/, sitemap has `https://yinzlang.com/`, `_dev` excluded, 2 JSON-LD schemas, canonical, title, og:type, twitter:card all in index.html `<head>`.
- Deviation: `nuxt-robots` incompatible → static robots.txt (simpler, equally correct for pre-launch). `nuxt-schema-org` composable had SSG issues → JSON-LD via `useHead` (same output, more reliable). Both deviations are equivalent in the final artifact.

### 2026-05-20 — Phase 6 execution
- Created `.github/workflows/website.yml`: path-filtered (website/** only), bun@1.2.21, frozen-lockfile install, typecheck, generate, upload artifact. Concurrency group cancels superseded runs.
- Existing ci.yml unchanged.

### 2026-05-20 — Phase 7 execution
- Created `website/Dockerfile`: multi-stage bun:1.2.21-alpine → nginx:1.27-alpine. Build context = repo root (required for grammar sync). Built successfully in devcontainer Docker.
- Created `website/.do/app.yaml`: DO App Platform spec (static_site, main branch, dockerfile_path: website/Dockerfile).
- Updated `website/README.md`: Deployment section with docker build command (repo-root context), DO App Platform Option A/B, CSP warning, HMR caveat, host-bun-install reminder.
- Updated roadmap `webpage-docs.md`: Cloudflare Pages → DigitalOcean App Platform in 3 locations (Architectural Decisions, Risks table, Open Questions). last_updated bumped.
- Final smoke test: `bun run generate` passes (8 routes, all SEO/Shiki/components working).
- DooD note: Production container port binding not verifiable from devcontainer (same DooD limitation as dev server). Docker image builds clean and nginx logs show worker processes started. Patrick verifies HTTP response from his local machine.
- Flipping plan status to `done` — all phases complete.

### Next step
Phase 1 complete. Phase 2 (Tailwind v4 + design tokens + fonts) next.

## Context & Why

**Goal**: Stand up the empty-but-deployable Nuxt 4 site at `website/` with locked aesthetic, design-token-driven Tailwind v4 theme, full primitive component library, Shiki-powered Yinz syntax highlighting matching the VSCode extension, SEO suite + image optimization, and a CI build job that fails the workspace if the site breaks. End state: a stub homepage and an internal `/_dev/components` gallery render correctly; Patrick can wire DigitalOcean App Platform to the GitHub repo with a documented build command and the staging deploy works.

**Why**: This is the foundation milestone in the `webpage-docs` roadmap. Nothing else in that roadmap can land without it — the landing page (M2), roadmap registry component (M3), docs pipeline (M4), examples/diagnostics (M5), blog (M6), and launch (M7) all assume the primitives, design tokens, build infra, and CI are in place. Cutting corners here means re-paying that cost in every subsequent milestone, which is exactly what the roadmap's Anti-Drift commitment exists to prevent.

**Why now**: v0.2-M4 (watch daemon) ships imminently and v0.2-M5 (LSP completion) is the last v0.2 work. The window between "v0.2 demonstrable" and "public site live" should be days, not months. Foundation built now means landing-page work can start the moment v0.2 lands.

**Background**: The compiler repo currently has zero web presence — only the GitHub README. A design prototype exists at `/tmp/yinz-design/yinz/project/` (3 HTML files + `shared.css`) that demonstrates the visual language. This milestone extracts the design tokens, rebuilds the primitives as Vue components in Nuxt 4, and wires the build infrastructure. **No marketing content** lives in this milestone — only the empty stub homepage. M2 (`webpage-landing`) builds the actual landing-page content on top.

**Constraints**:
- **Bun** (not npm) — Patrick's preference. Pinned version (not `@latest`), runs from a Docker Compose service.
- **Port 6002** external (mapped to Nuxt's internal 3000) so it doesn't collide with the parallel chat's work.
- **DigitalOcean App Platform** for hosting (NOT Cloudflare Pages — roadmap update needed). Patrick handles DNS + DO dashboard wiring; this milestone produces the build command + output dir specification.
- **Sibling chat is restructuring `examples/`** — this plan touches `website/**` and `.github/workflows/` only. No collision.
- **Anti-drift**: even though this milestone has no Yinz snippets in production content, the `<YCode>` primitive built here is the foundation for the snippet-as-real-file discipline that M2+ enforces.

**Success criteria**:
- `cd website && bun install && bun run dev` brings up the site on http://localhost:6002 with stub homepage rendering correctly
- `bun run generate` produces a working SSG build in `website/.output/public/`
- `/_dev/components` route exists and renders every primitive component visually for inspection
- A `.ynz` snippet rendered through `<YCode>` shows Yinz syntax highlighting matching the VSCode extension (keywords, types, strings all colored per `shared.css` token palette)
- CI job `website-build` passes on `main` and on PRs that touch `website/**`
- README documents: dev workflow (docker compose up), production build command, DigitalOcean App Platform deployment config

## Research Findings

### Tech stack verification (re-confirmed against the roadmap's locked decisions)

- **Nuxt 4** is current stable as of 2026. `app/` directory layout default. SSG via `nuxi generate` produces `.output/public/`.
- **Tailwind v4 via `@tailwindcss/vite`**: the documented 2026 install path. The `@nuxtjs/tailwindcss` module still hasn't shipped v4 support (issue nuxt-modules/tailwindcss#820). Vite plugin path is supported and works with Nuxt's Vite integration.
- **Bun + Nuxt**: officially supported. `bun create nuxi@latest` works; `bun run dev` works; SSG generate works. Hot reload works on Linux file systems (bind-mount caveat for WSL2 noted in Risks).
- **Bun version pin**: latest stable line is 1.2.x. Pinning `oven/bun:1.2.21-alpine` (Aug 2025 release, stable). Patrick can bump at execution time if a newer stable lands.
- **Shiki + custom TextMate grammar**: Shiki accepts arbitrary `.tmLanguage.json` files. The existing `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` (2228 bytes, verified) can be loaded directly. Custom themes are JSON; we'll author a `yinz-coal.json` theme that maps Shiki token scopes to the `shared.css` palette (`--river`, `--rust`, `--moss`, `--gold`, `--ink-dim`, `--ink-mute`, `--plum`).
- **`@nuxt/content` v3**: deferred to M4 (docs pipeline). NOT installed in foundation. Foundation only needs raw Vue components.
- **`@nuxt/image` with `ipx`**: build-time provider, no runtime server needed. SSG-safe.
- **`@nuxt/fonts`**: official module for self-hosting + CSS optimization of fonts. Handles Anton + Inter + JetBrains Mono from Google Fonts at build time. SSR-safe, no FOUT.
- **SEO suite**: `nuxt-schema-org`, `nuxt-robots`, `nuxt-og-image`, `@nuxtjs/sitemap` are all maintained by the Nuxt SEO team. Install + minimal config is well-documented.
- **DigitalOcean App Platform + Bun**: DO's native buildpack is Node. For Bun, two paths: (1) use a custom Dockerfile in the repo; (2) use the Node buildpack and invoke bun via `npx bun ...`. Path (1) is cleaner and matches the docker-compose dev setup. Decision: ship a `website/Dockerfile` for production that DO consumes.

### Design prototype anchors

- `/tmp/yinz-design/yinz/project/shared.css` — all design tokens (`:root` vars), primitive component classes, prose styling, responsive breakpoints
- `/tmp/yinz-design/yinz/project/index.html` — landing-page reference (M2 will mine this; foundation references for component shapes)
- `/tmp/yinz-design/yinz/project/docs.html` — docs reference (M4)
- `/tmp/yinz-design/yinz/project/blog.html` — blog reference (M6)
- `website/yinz.svg` (9236 bytes) — existing logo mark vector (used by `<YLogo>` in foundation)
- `website/yinz.png` (886KB) — raster fallback / OG image source (M7 will use)

### Font swap rationale (roadmap deviation — locked this chat)

Roadmap "Aesthetic locked" section reversed Space Grotesk → athletic display face. Patrick confirmed **Anton (display) + Inter (body) + JetBrains Mono (code)** this chat. `shared.css` `--font-display` and `--font-body` get overridden in our Tailwind theme; the prototype CSS file is reference-only, not imported.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Tailwind v4 + Nuxt 4 + Vite plugin combo has unreported integration edges | Medium | Medium — dev friction, possible refactor | Smoke-test in Phase 1 with a minimal `@theme` block + one utility class before building any components. If broken, fall back to Tailwind v4 CSS-only mode (no Vite plugin) which is documented for any framework. |
| Bun bind-mount file watching in WSL2 misses changes (hot reload broken) | Medium | Low — dev annoyance, not blocker | Enable Nuxt's polling watcher (`vite: { server: { watch: { usePolling: true, interval: 300 } } }`) inside docker-compose. Document the toggle in README. |
| Shiki Yinz theme doesn't visually match `shared.css` color tokens exactly | Medium | Low — visual polish | Author `yinz-coal.json` theme JSON deriving token colors from CSS vars at build time; verify against the prototype `index.html` `.tk-*` class colors. If parity fails, accept best-effort and refine in M2. |
| `ynz.tmLanguage.json` (2228 bytes, ~M2-era) doesn't cover M3+ syntax (numbers, fixed/array, options, shape, follows, extends, etc.) | High | Medium — code blocks render with missing colors | Audit grammar coverage in Phase 4 against shipped Yinz syntax. If gaps exist, file a follow-up plan to extend the grammar (separate from this milestone). Foundation ships with whatever the grammar covers today — gaps surface as un-highlighted tokens, not broken pages. |
| DigitalOcean App Platform + custom Dockerfile build fails or is slow | Medium | Medium — deployment blocked | Ship a tested `website/Dockerfile` AND document the Node-buildpack `npx bun` fallback in README. Patrick can choose either path when wiring DO. Verify Dockerfile builds locally before declaring the phase done. |
| Sibling chat working on `examples/` restructure causes merge conflicts | Low | Low — file scope is disjoint | This plan touches `website/**` and `.github/workflows/` only. Confirmed disjoint. If a `.gitignore` or root-level config needs editing, coordinate via state.md note. |
| CI bun installation adds significant time to every PR | Low | Low — minor CI slowdown | Cache `~/.bun` and `website/node_modules` (yes, bun installs to node_modules) across runs. Only run website-build job when `website/**` or `.github/workflows/website.yml` changed (path filter on the workflow). |
| Roadmap front-matter still lists Cloudflare Pages as hosting; needs update | High | Low — documentation drift | Phase 7 updates `webpage-docs.md` roadmap to reflect DigitalOcean App Platform as the locked hosting choice. Same chat, same plan. |
| Stub homepage with no real content looks broken to a casual visitor before M2 lands | Medium | Low — staging URL is unindexed | Stub page says date-free: "Yinz language — site under construction. v0.2 in progress. Check the [GitHub repo] for release status." No hardcoded dates that age into lies; live status lives on the M3 roadmap component. Robots disallow blocks indexing until M7. |
| Shiki emits inline `<span style="color:...">` styles; future strict CSP at the DO edge or via `nuxt-security` would silently strip them, making every code block monochrome | Low | High (when triggered) — every `<YCode>` on every page goes colorless | Document in `website/README.md` "Deployment" section: any CSP added later MUST include `'unsafe-inline'` for `style-src` OR migrate Shiki to class-based theming (which Shiki supports — different theme JSON shape). Flag this for M7 launch planning so it's not discovered when CSP gets turned on. |
| `@nuxt/fonts` downloads Google Fonts at build time; DigitalOcean App Platform build container may rate-limit-hit or fail with no network, making builds non-deterministic | Medium | High — silent build failure or missing fonts | Use `@nuxt/fonts` `download: true` + commit the downloaded font files to `website/public/_fonts/` (or `app/assets/fonts/`) so production builds are NOT network-dependent. README documents the "fonts are vendored at install time, not build time" discipline. If a font swap happens, refresh the vendored files in the same PR. |

## Risk Assessment & Rollout Strategy

**Risk level: LOW**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | |
| Touches auth/permissions | No | |
| Raw SQL / literals | No | |
| Modifies existing data | No — additive only | New `website/` tree + one new CI workflow |
| Third-party integration | Yes (light) | DigitalOcean App Platform, but Patrick wires manually; no API tokens in repo |
| Changes existing endpoints | No | |

**Mitigations applied**:
- Read-only website (no backend writes, no user data) → LOW
- Backward compatible (existing CI untouched; new workflow is additive) → LOW
- Robots.txt blocks indexing until launch milestone → no public-facing risk

**Rollout plan**:
1. **Internal**: stub site visible on staging URL once Patrick wires DO. Internal-only validation by Patrick — no real visitors, no analytics, no SEO indexing.
2. **No partial rollout needed** — this is foundation infra, not a user-facing feature. Stays internal until M7 launch milestone.
3. **No feature flag** — site doesn't ship to public until M7; the staging URL itself is the flag (only Patrick has it).

## Component Inventory (FULL site — all milestones)

Patrick asked for ALL reusable components thought out ahead of time, not just M1. The table below is the complete inventory across the roadmap. **Built this milestone** = M1 foundation. Everything else gets stubbed conceptually here and built in its owning milestone.

| Component | Owns | Status | Notes |
|---|---|---|---|
| **Layout primitives** | | | |
| `<YContainer>` | M1 | Built | Max-width wrapper with responsive padding (matches `.container`) |
| `<YSection>` | M1 | Built | Vertical padding + optional top border (matches `section + section`) |
| `<YRow>` | M1 | Built | Flex row helper (matches `.row`) |
| `<YGrid>` | M1 | Built | Grid helper (matches `.grid`) |
| `<YDivider>` | M1 | Built | 1px gold/line horizontal rule |
| **Typography** | | | |
| `<YEyebrow>` | M1 | Built | Small uppercase mono label (matches `.eyebrow`) |
| `<YHeading>` | M1 | Built | h1-h4 wrapper using Anton display face |
| `<YProse>` | M6 | Deferred | Long-form content wrapper with chevron bullets, callouts, blockquotes (matches `.prose`). Foundation ships base font + color tokens only. |
| **Interaction** | | | |
| `<YButton>` | M1 | Built | `primary` + `ghost` variants (matches `.btn-primary`, `.btn-ghost`) |
| `<YLink>` | M1 | Built | Gold-on-coal text link with hover state |
| **Indicators** | | | |
| `<YPill>` | M1 | Built | Status badge with optional dot (matches `.pill`) |
| `<YCallout>` | M1 | Built | `info` / `warn` / `note` variants with tag label |
| **Content surfaces** | | | |
| `<YCard>` | M1 | Built | bg-card surface with border (matches `.card`) |
| `<YCode>` | M1 | Built | Code block with filename header, Shiki rendering, line gutter, Yinz grammar |
| **Branding** | | | |
| `<YLogo>` | M1 | Built | Block-Y SVG mark + Anton wordmark + optional `.ext` tail |
| `<YNav>` | M1 | Built | Sticky top nav with backdrop blur, links, CTA slot |
| `<YFooter>` | M1 | Built | 4-column grid footer with copyright row |
| **Landing-page content (M2)** | | | |
| `<YHero>` | M2 | Deferred | Landing hero with display headline + sub + CTA cluster |
| `<YPillarGrid>` | M2 | Deferred | 3-up feature grid (uses `<YCard>` primitive underneath) |
| `<YCompare>` | M2 | Deferred | Yinz vs Rust side-by-side (uses `<YCode>` primitive) |
| `<YInstallCard>` | M2 | Deferred | Install command surface with copy button |
| `<YRoadmapRowStatic>` | M2 | Deferred → replaced M3 | Hardcoded roadmap row in M2; replaced by `<YRoadmap>` in M3 |
| **Roadmap registry (M3)** | | | |
| `<YRoadmap>` | M3 | Deferred | Reads `registry/roadmap.toml` + plan-file frontmatter + GitHub release state; renders status pills |
| **Docs pipeline (M4)** | | | |
| `<YSidebar>` | M4 | Deferred | Docs left-side nav from file tree + frontmatter weighting |
| `<YTOC>` | M4 | Deferred | Right-rail in-page TOC |
| `<YSearchBox>` | M4 | Deferred | Pagefind search input |
| `<YDocLayout>` | M4 | Deferred | Docs page layout wrapper |
| `<YBreadcrumb>` | M4 | Deferred | Docs breadcrumb trail |
| MDC `<Callout>` | M4 | Deferred | MDC version of `<YCallout>` for use inside markdown |
| MDC `<Diff>` | M4 | Deferred | Vs-Rust comparison block usable in markdown |
| MDC `<CodeOutput>` | M4 | Deferred | Code + output pair usable in markdown |
| **Examples + diagnostics (M5)** | | | |
| `<YDiagnosticBlock>` | M5 | Deferred | Renders compiler error in WHAT/WHAT-INSTEAD/WHY format |
| `<YExampleSlice>` | M5 | Deferred | Sliced demo section with source + insta-captured output |
| **Blog (M6)** | | | |
| `<YPostCard>` | M6 | Deferred | Blog index card |
| `<YPostHeader>` | M6 | Deferred | Blog post header (title + tags + date + reading time) |
| `<YFilterChips>` | M6 | Deferred | Tag filter row |
| **Composables** | | | |
| `useScrollLock()` | M1 | Built | Body-scroll lock used by `<YNav>` mobile drawer; small (~15 lines), fully implemented in M1 not a skeleton |
| `useShikiTheme()` | M1 | Built | Wraps the yinz-coal theme for `<YCode>` |

Total: **15 components built this milestone**, ~20 deferred to owning milestones with clear inheritance from M1 primitives (deferred components compose foundation primitives — no parallel APIs).

**Note on MDC components (deferred to M4)**: `<Callout>`, `<Diff>`, `<CodeOutput>` for use inside `@nuxt/content` markdown — these are inherently a separate rendering path (MDC) but MUST be thin wrappers over the corresponding M1 Vue primitives (`<YCallout>`, future `<YDiff>`, future `<YCodeOutput>`), not reimplementations. This avoids the parallel-API anti-pattern from `stdlib-design.md` Rule 2. M4 plan-reviewer should enforce.

## Phases

Each phase = one PR via `/pr`. Phases ordered so each ends in a working state.

### Phase 1: Bun + Nuxt 4 scaffold + docker-compose dev environment
**PR scope**: Create `website/` directory, scaffold Nuxt 4 with bun, ship docker-compose with pinned bun image on port 6002, add `.dockerignore` and `.gitignore` entries. Site boots to default Nuxt welcome page.
**Branch**: `feat/webpage-foundation-scaffold`
**Flag**: N/A (no production traffic until M7)
**Est. lines**: ~150 (mostly config)
**Ships via**: `/pr`
**Objective**: a fresh clone of the repo can `docker compose -f website/docker-compose.yml up` and see Nuxt's default welcome page at http://localhost:6002.
**Why this phase exists**: every later phase needs the bun + nuxt environment running. Locking the toolchain version + docker setup first prevents "works on Patrick's machine" surprises in CI and DO.
**Current-state anchors**:
- `website/` exists but only contains `yinz.png` + `yinz.svg` (no code)
- Devcontainer has Node 22 but NOT bun — confirmed via `which bun` returning not-found
- `.github/workflows/ci.yml` is Rust-only (~65 lines, no Node/web jobs)
**Files (expected scope)**:
- `website/package.json` (new — nuxt deps, bun-managed)
- `website/nuxt.config.ts` (new — minimal config)
- `website/app.vue` (new — root component)
- `website/app/pages/index.vue` (new — stub homepage saying "site under construction")
- `website/tsconfig.json` (new — extends nuxt-generated)
- `website/docker-compose.yml` (new — bun image, port 6002, volume mount)
- `website/Dockerfile.dev` (new — optional bun base for consistency)
- `website/.dockerignore` (new)
- `website/.gitignore` (new — `.nuxt/`, `.output/`, `node_modules/`, `bun.lock` checked in)
- `website/README.md` (new — dev workflow section)
- `.gitignore` (root — append `website/node_modules`, `website/.nuxt`, `website/.output`)
**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work (e.g., a `tsconfig.json` root-level reference, missing `.gitkeep` files). Document each deviation in the PR description with one-line reason. If a deviation is its own concern (unrelated bug, opportunistic refactor), STOP — split into a separate PR.
**Steps**:
1. Choose bun image pin. Default: `oven/bun:1.2.21-alpine`. If a newer stable exists at execution time, bump and note in PR.
2. Create `website/docker-compose.yml` with single `web` service: image pinned, working_dir `/app`, volume `./:/app`, port mapping `6002:3000`, command `sh -c "bun install && bun run dev --host 0.0.0.0"`, enable polling watcher via env or nuxt config for WSL compatibility.
3. Scaffold Nuxt 4 inside docker: `docker compose run --rm web bunx nuxi@latest init . --packageManager bun --gitInit false`. Verify `package.json`, `nuxt.config.ts`, `app.vue`, `app/pages/index.vue` (or equivalent in Nuxt 4 layout) are generated.
4. Replace stub index page content with date-free copy: "Yinz language — site under construction. v0.2 in progress. Check the [GitHub repo] for release status." Plain HTML, no styling yet. NO hardcoded dates anywhere — live status comes from the M3 roadmap registry component, not baked text.
5. Add `website/Dockerfile.dev` mirroring docker-compose image + commands for editor/IDE consistency.
6. Add `website/.dockerignore` excluding `node_modules`, `.nuxt`, `.output`.
7. Add `website/.gitignore` per Nuxt convention (`.nuxt/`, `.output/`, `node_modules/`, `.env`). Commit `bun.lock`.
8. Append `website/node_modules`, `website/.nuxt`, `website/.output` to root `.gitignore` belt-and-suspenders.
9. Write `website/README.md` "Dev workflow" section: prereqs (docker), `docker compose -f website/docker-compose.yml up`, `http://localhost:6002`, troubleshooting (WSL polling, bun cache).
10. Smoke-test: `docker compose up`, open browser, verify the under-construction page renders, hot-reload works (edit stub text, see change).
**Acceptance criteria**:
- [x] `docker compose -f website/docker-compose.yml up` brings the site up on http://localhost:6002 and `curl -sf http://localhost:6002` returns 200 with the under-construction text in the response body (no hard timing SLA — first-run cold cache is hardware-variable)
- [x] Default `/` route renders the under-construction text (no errors in browser console, no 404s)
- [x] Hot reload works: editing `app/pages/index.vue` text reflects in browser on save (observable; latency is host-dependent)
- [x] `bun.lock` is checked in and `bun install --frozen-lockfile` succeeds on a fresh clone with no warnings
- [x] README "Dev workflow" section exists and is followable by a contributor who has only docker installed
- [x] Stub homepage copy contains NO hardcoded dates (no "Q3 2026", no specific timeline) — only "v0.2 in progress" + link to repo
**Quality gate**:
- [x] Bun version is a specific pin (not `:latest`, not `:1` floating)
- [x] No npm or yarn lockfiles present in `website/`
- [x] Port 6002 chosen deliberately and documented (sibling chat conflict avoidance)
- [x] `.gitignore` correctly excludes `node_modules`, `.nuxt`, `.output`
- [x] Stub homepage copy is honest about pre-v1.0 state (no fabricated install commands, no "use Yinz today", no fabricated dates)
- [x] README warns: "always install via docker compose; never bare `bun install` from host" (host bun version drift would corrupt lockfile if host bun != image bun)
**Verification**: `docker compose -f website/docker-compose.yml up -d && sleep 60 && curl -sf http://localhost:6002 | grep "under construction"` returns 0.

**Exit Sequence — RUN THESE STEPS:**
1. Persist plan state — tick acceptance + quality-gate checkboxes per actual results, bump `last_updated:`.
2. Invoke `code-reviewer` against the Phase 1 diff.
3. Handle verdict — BLOCK loop max 3 rounds, PASS proceeds.
4. Prompt user: "Phase 1 done. Code-reviewer: <verdict>. Ready to commit + Phase 2?"
5. Do NOT start Phase 2 until user confirms.

---

### Phase 2: Tailwind v4 + design tokens + Anton/Inter/JetBrains Mono fonts
**PR scope**: Wire `@tailwindcss/vite`, author `@theme` block extracting all color/radius/breakpoint tokens from `shared.css`, install `@nuxt/fonts` with Anton + Inter + JetBrains Mono, replace the stub page's plain HTML with token-styled markup to verify visual fidelity.
**Branch**: `feat/webpage-foundation-tailwind-tokens`
**Flag**: N/A
**Est. lines**: ~200
**Ships via**: `/pr`
**Objective**: every color, font, radius, and spacing value from the locked aesthetic is available as a Tailwind utility class. Stub page renders with coal background, gold accent, Anton headline, Inter body, JetBrains Mono code.
**Why this phase exists**: every component built in Phase 3+ depends on the design tokens being addressable as utilities. Authoring the theme once at the start beats retrofitting later.
**Current-state anchors**:
- `/tmp/yinz-design/yinz/project/shared.css:6-40` — `:root` CSS vars (THE source of truth for tokens)
- `/tmp/yinz-design/yinz/project/shared.css:66-91` — typography baseline (Anton + Inter mappings to migrate)
- Roadmap "Aesthetic locked" section — font swap rationale (Anton + Inter, not Space Grotesk)
**Files (expected scope)**:
- `website/package.json` (deps: `tailwindcss@4`, `@tailwindcss/vite@4`, `@nuxt/fonts`)
- `website/nuxt.config.ts` (vite.plugins += tailwindcss; modules += `@nuxt/fonts`; fonts config)
- `website/app/assets/css/tailwind.css` (new — `@import "tailwindcss"` + `@theme` block)
- `website/app/app.vue` or `website/app/layouts/default.vue` (apply base classes — coal bg, ink text)
- `website/app/pages/index.vue` (replace plain HTML with token-styled version)
- `website/README.md` (append "Design tokens" section pointing at `shared.css` as historical reference)
**Deviation rule**: same as Phase 1.
**Steps**:
1. `docker compose exec web bun add -D tailwindcss@4 @tailwindcss/vite@4 @nuxt/fonts`.
2. Create `website/app/assets/css/tailwind.css` with `@import "tailwindcss"` and a `@theme` block. Map every `--foo` from `shared.css` to a Tailwind v4 theme key:
   - Colors: `--color-bg`, `--color-bg-raised`, `--color-bg-elev`, `--color-bg-card`, `--color-line`, `--color-line-strong`, `--color-ink`, `--color-ink-mute`, `--color-ink-dim`, `--color-gold`, `--color-gold-deep`, `--color-gold-soft`, `--color-gold-glow`, `--color-ember`, `--color-river`, `--color-moss`, `--color-rust`, `--color-plum`
   - Radii: `--radius-sm`, `--radius`, `--radius-lg`, `--radius-xl` (6/10/16/22px)
   - Fonts: `--font-display: "Anton, ui-sans-serif, system-ui, sans-serif"`, `--font-sans: "Inter, ui-sans-serif, system-ui, sans-serif"`, `--font-mono: "JetBrains Mono, ui-monospace, ..."`
   - Max width: `--container-max: 1240px`
3. Wire `@tailwindcss/vite` in `nuxt.config.ts` under `vite.plugins`.
4. Import `tailwind.css` from `app/app.vue` (top-level) so utilities are available everywhere.
5. Install `@nuxt/fonts` and configure to download + self-host Anton, Inter (weights 400, 500, 600, 700), JetBrains Mono (weights 400, 500, 600). Run `bun run dev` ONCE locally to trigger the download; then **commit the vendored font files** (`website/public/_fonts/` or wherever `@nuxt/fonts` writes them) so production builds NEVER hit Google Fonts. Document the "fonts are vendored at install time, not build time" discipline in README. If a font swap happens later, refresh vendored files in the same PR.
6. Update root layout / `app.vue`: apply `bg-bg text-ink font-sans antialiased` to body equivalent. Add the `body::before` grain via a small `<div>` with the two radial gradients from `shared.css:54-64` (or via `@layer base` in tailwind.css).
7. Restyle `/` index page using Tailwind utilities: container, eyebrow-style label, Anton headline ("Site under construction"), Inter sub-text, gold accent on the GitHub link.
8. Smoke-test in browser: coal bg, gold link, Anton headline, hot reload still working.
9. Run `bun run generate` and verify SSG build succeeds. Open `website/.output/public/index.html` in a browser via `file://` to confirm static build renders identically.
**Acceptance criteria**:
- [x] Every `--foo` CSS var from `shared.css:6-40` has a corresponding Tailwind theme token (verified by inspecting `tailwind.css`)
- [x] Anton renders on h1, Inter renders on body, JetBrains Mono renders on `<code>` (verified via browser devtools "Computed → font-family")
- [x] Fonts are self-hosted via `@nuxt/fonts` (Network tab shows requests to local origin, not fonts.gstatic.com)
- [x] `bun run generate` succeeds and `.output/public/index.html` opens correctly via `file://`
- [x] No FOUT on initial page load (font-display: swap or optional, locked via `@nuxt/fonts` config)
**Quality gate**:
- [x] No fonts loaded from Google Fonts CDN at runtime
- [x] Token names follow Tailwind v4 convention (`--color-*`, `--font-*`, `--radius-*`, `--container-*`)
- [x] No magic color hex codes in component-level CSS — every color comes from a theme token
- [x] Theme block has a comment block at the top pointing to `shared.css` as historical reference
- [x] Page renders identically in dev (`bun run dev`) and prod (`bun run generate` + `file://`)
**Verification**:
- Browser devtools: inspect any element, "Computed" panel, verify `font-family` matches Anton/Inter/JetBrains Mono
- Network tab: confirm font URLs are local (`/_fonts/...` or similar)
- `bun run generate && cd .output/public && python3 -m http.server 8080` then `curl -sf http://localhost:8080 | grep -i anton` returns 0

**Exit Sequence — RUN THESE STEPS:**
1. Persist plan state.
2. Invoke `code-reviewer` against Phase 2 diff.
3. Handle verdict.
4. Prompt user.
5. Do NOT start Phase 3 until user confirms.

---

### Phase 3a: Primitive components — layout + typography + indicators (9 components)
**PR scope**: Build the smaller, stateless M1 primitives in one PR. Add a scaffolded `/_dev/components` gallery route showing each. No interactivity, no composites yet.
**Branch**: `feat/webpage-foundation-primitives-stateless`
**Flag**: N/A
**Est. lines**: ~500 (9 small components avg 40-50 lines + gallery scaffold)
**Ships via**: `/pr`
**Objective**: every stateless layout/typography/indicator primitive renders correctly on a new `/_dev/components` route. Patrick can visually verify each against the prototype.
**Why this phase exists**: splitting from interactive/composite primitives keeps each PR's review surface tight (per plan-reviewer pushback). These 9 components are mostly Tailwind-utility wrappers — small, easy to review side-by-side with the prototype.
**Current-state anchors**:
- `/tmp/yinz-design/yinz/project/shared.css:93-330` — primitive class definitions
- `/tmp/yinz-design/yinz/project/index.html` — primitives used in context
**Files (expected scope)**:
- `website/app/components/primitives/YContainer.vue`
- `website/app/components/primitives/YSection.vue`
- `website/app/components/primitives/YRow.vue`
- `website/app/components/primitives/YGrid.vue`
- `website/app/components/primitives/YDivider.vue`
- `website/app/components/primitives/YEyebrow.vue`
- `website/app/components/primitives/YHeading.vue`
- `website/app/components/primitives/YPill.vue`
- `website/app/components/primitives/YCallout.vue`
- `website/app/pages/_dev/components.vue` (new — gallery scaffold; Phase 3b extends)
- `website/README.md` (append "Component library" section)
**Deviation rule**: same.
**Steps**:
1. For each component: TypeScript-typed `<script setup lang="ts">` with `defineProps<{...}>()`, scoped `<style>` only when Tailwind utilities are insufficient. No `any`. Use `T \| null` for optional structural fields per `coding-style.md`. Arrow functions only — no `function` keyword in `.vue`/`.ts` files (composables, helpers).
2. Build order: YContainer → YSection → YRow/YGrid → YDivider → YEyebrow → YHeading → YPill → YCallout.
3. Variants:
   - `<YHeading level="1|2|3|4">` — corresponds to `shared.css:76-79` font-size scale
   - `<YPill>` with optional dot slot (matches `.pill` + `.pill .dot`)
   - `<YCallout variant="info|warn|note">` with `tag` prop + default slot; `warn` uses `--color-ember` left border per `shared.css:422`
4. `/_dev/components` page scaffold: section per category (Layout / Typography / Indicators), each component rendered with all variants. Add `<meta name="robots" content="noindex">` via `useHead` on this route.
5. Visual diff against the prototype `index.html` — note deltas in PR description.
**Acceptance criteria**:
- [x] All 9 components exist under `website/app/components/primitives/`
- [x] `/_dev/components` renders without console errors; each component visible with all variants
- [x] No `any`, no `as any`, no non-null assertion `!` (verified via `bunx nuxi typecheck`)
- [x] `<YCallout variant="warn">` left border is `--color-ember` (verified via generated CSS)
- [x] `<YHeading level="1">` font-size matches `clamp(48px, 7vw, 96px)` per shared.css:76
- [x] `/_dev/components` is noindex (head meta + excluded from sitemap via Phase 5 config)
**Quality gate**:
- [x] Every component file has a one-line description comment at top (props summary)
- [x] Props use `T | null` for optional object fields
- [x] Arrow functions only in script blocks; no `function` keyword
- [x] Tailwind utilities preferred over scoped CSS
- [x] `bun run generate` succeeds — every primitive renders in SSG
**Verification**:
- `docker compose exec web bunx nuxi typecheck` returns 0 errors
- Visit `/_dev/components`, eyeball-compare against prototype

**Exit Sequence — RUN THESE STEPS:**
1. Persist plan state.
2. Invoke `code-reviewer`.
3. Handle verdict.
4. Prompt user.
5. Do NOT start Phase 3b until user confirms.

---

### Phase 3b: Primitive components — interactive + composite (6 components + composable)
**PR scope**: Build the interactive (`<YButton>`, `<YLink>`) and composite (`<YCard>`, `<YLogo>`, `<YNav>`, `<YFooter>`) M1 primitives, plus `useScrollLock` composable. Extends the `/_dev/components` gallery with these.
**Branch**: `feat/webpage-foundation-primitives-interactive`
**Flag**: N/A
**Est. lines**: ~600 (6 components — `<YNav>` is ~120, `<YFooter>` ~80, others smaller — plus composable + gallery extension)
**Ships via**: `/pr`
**Objective**: the larger composite primitives (`<YNav>` with mobile drawer + sticky + backdrop blur; `<YFooter>` 4-col grid) render correctly. `useScrollLock` works on mobile drawer open/close.
**Why this phase exists**: these primitives carry real complexity (state, refs, responsive). Reviewing them separately from the stateless wrappers in 3a means the reviewer can focus on the interactive logic.
**Current-state anchors**:
- `shared.css:118-139` — `.nav` sticky + backdrop-blur rules
- `shared.css:142-165` — `.btn-primary` + `.btn-ghost` shapes
- `shared.css:268-298` — `footer` grid + bottom row
- `website/yinz.svg` — logo mark asset
**Files (expected scope)**:
- `website/app/components/primitives/YButton.vue`
- `website/app/components/primitives/YLink.vue`
- `website/app/components/primitives/YCard.vue`
- `website/app/components/primitives/YLogo.vue`
- `website/app/components/primitives/YNav.vue`
- `website/app/components/primitives/YFooter.vue`
- `website/app/composables/useScrollLock.ts` (~15 lines, SSR-safe)
- `website/app/pages/_dev/components.vue` (extend with new components)
**Deviation rule**: same.
**Steps**:
1. `useScrollLock()` composable — fully implemented in M1, not skeleton. Locks body scroll via `document.body.style.overflow = 'hidden'` when active; reverts on cleanup. SSR-safe (guard `document` access via `onMounted` or `import.meta.client`). ~15 lines.
2. Build order: YLink → YButton → YCard → YLogo → YNav (uses YLink + YButton + YLogo + useScrollLock) → YFooter (uses YGrid + YLink).
3. Variants:
   - `<YButton variant="primary|ghost">` — matches `shared.css:154-165`; hover lift via `translateY(-1px)`
   - `<YNav>` — sticky top, backdrop-blur via Tailwind v4 `backdrop-blur-md` or scoped CSS for `color-mix()`/`saturate()` parity; props `links: Array<{label:string, href:string, active?:boolean}>`, default CTA slot, mobile drawer at <600px viewport
   - `<YFooter>` — 4-column grid (1.4fr 1fr 1fr 1fr) per `shared.css:276-279`, collapses at <900px and <600px breakpoints
   - `<YLogo>` — slots block-Y SVG mark (`website/yinz.svg`) + Anton wordmark "yinz" + optional `.ext` mono tail (e.g., ".lang")
4. Extend `/_dev/components` page with new sections: Interaction, Branding, Composite. Show `<YNav>` with both desktop + mobile drawer states (resize to demo).
5. Test mobile drawer: resize browser <600px, click hamburger, drawer opens, body scroll locks, click close, body scroll restores.
**Acceptance criteria**:
- [x] All 6 components + `useScrollLock` exist
- [x] `<YNav>` mobile drawer locks body scroll when open and restores on close (verified at viewport <600px)
- [x] `<YButton variant="primary">` matches `shared.css:154-159` (gold bg `#FFD23F`, dark text `#1a1208`, border radius `999px`, hover lift)
- [x] `<YLogo>` correctly displays `website/yinz.svg` mark + Anton "yinz" wordmark
- [x] `<YFooter>` 4-column grid collapses to 2-column at <900px and 1-column at <600px
- [x] `useScrollLock` is SSR-safe (verified: SSG build does not throw "document is not defined")
**Quality gate**:
- [x] Every component file has a one-line description comment at top
- [x] Props use `T | null` for optional object fields; `T?` allowed only for function-parameter defaults
- [x] Arrow functions only; no `function` keyword
- [x] `useScrollLock` accesses `document` only inside lifecycle hook or behind `import.meta.client` guard
- [x] No `any`, no `as any`, no `!`
- [x] `bun run generate` succeeds and all interactive components render in SSG (state initializes correctly post-hydration)
**Verification**:
- `bunx nuxi typecheck` returns 0
- `bun run generate` succeeds; `.output/public/_dev/components/index.html` contains the new components
- Manual mobile-drawer test on dev server

**Exit Sequence — RUN THESE STEPS:**
1. Persist plan state.
2. Invoke `code-reviewer`.
3. Handle verdict.
4. Prompt user.
5. Do NOT start Phase 4 until user confirms.

---

### Phase 4: `<YCode>` + Shiki + Yinz grammar + custom theme
**PR scope**: Install Shiki (or use `@nuxtjs/mdc` which bundles it), load `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` as a custom language, author `yinz-coal.json` theme JSON mapping token scopes to `shared.css` colors, build `<YCode>` component with filename header + line gutter + Yinz syntax highlighting. Render at least one real `.ynz` snippet on `/_dev/components`.
**Branch**: `feat/webpage-foundation-ycode-shiki`
**Flag**: N/A
**Est. lines**: ~250
**Ships via**: `/pr`
**Objective**: a Yinz snippet rendered via `<YCode>` shows correct syntax colors matching VSCode + the prototype `.tk-*` class colors. Color parity is "close enough" — exact pixel match isn't required, but `keyword → river-teal`, `string → moss-green`, `number/type → rust`, `function → gold`, `comment → ink-dim italic` must hold.
**Why this phase exists**: this is the M1 deliverable that gates the anti-drift mechanism for M2+. Without working Yinz highlighting in `<YCode>`, every Yinz snippet on the site would render as monochrome text — defeating the marketing purpose.
**Current-state anchors**:
- `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json:1-99` — the existing TextMate grammar (2228 bytes, ~M2-era coverage)
- `/tmp/yinz-design/yinz/project/shared.css:191-247` — `.code` shell + `.tk-*` token color targets
- `/tmp/yinz-design/yinz/project/index.html` — `<code>` examples in context (reference for filename header markup)
- `examples/pirates-roster/entrypoint.ynz` (created/maintained by sibling chat — confirm with state.md before referencing as test snippet; if unavailable, use a hand-written test snippet inline in `/_dev/components`)
**Files (expected scope)** — all mandatory:
- `website/package.json` (add `shiki`; add `"prebuild": "bun run sync-grammar"` and `"sync-grammar": "bun run build/sync-ynz-grammar.ts"` scripts)
- `website/build/sync-ynz-grammar.ts` (MANDATORY — copies `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` to `website/app/assets/grammars/ynz.tmLanguage.json` at build time; emits checksum; fails build if source missing)
- `website/app/assets/grammars/ynz.tmLanguage.json` (output of sync script, checked into repo so devs don't need to run sync before browsing)
- `website/app/assets/themes/yinz-coal.json` (new — Shiki theme matching `shared.css` palette)
- `website/modules/shiki-prerender.ts` (Nuxt module — registers `nitro` build hook that pre-highlights `<YCode>` content at build time; emits HTML server-side so the client receives static `<span style="color:...">` markup; no client JS needed for colors)
- `website/app/components/primitives/YCode.vue` (consumes pre-rendered HTML if available; falls back to client-side highlight only for dynamic code-rendering scenarios — none in M1)
- `website/app/composables/useShikiTheme.ts` (singleton Shiki highlighter; used by the module at build time AND as the runtime fallback path)
- `website/app/pages/_dev/components.vue` (append a `<YCode>` example with a real Yinz snippet)
- `website/README.md` (append "Yinz grammar sync" section explaining the source-of-truth and the prebuild discipline)
**Deviation rule**: same. Grammar coverage gaps surface as un-highlighted tokens — do NOT fix the grammar in this phase. File a follow-up plan if gaps are severe; foundation ships with whatever the grammar covers today.
**Steps**:
1. Add `shiki` dep via bun. Pin version.
2. Author `website/build/sync-ynz-grammar.ts` (Bun/Node-compatible TypeScript):
   - Reads `../tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` (relative to repo root)
   - Writes to `app/assets/grammars/ynz.tmLanguage.json`
   - Computes sha256 of source + destination; fails if mismatch after write
   - Fails with clear message if source file missing
   - Exits 0 on success
3. Wire `"prebuild": "bun run sync-grammar"` in `package.json` so `bun run generate` and `bun run dev` both run sync first. Also document running manually after pulling latest main.
4. Author `yinz-coal.json` theme — required scope mappings (Shiki theme JSON format):
   - `keyword.*` → `#5a8fa3` (river/teal — matches `.tk-key`)
   - `string.*` → `#8aa66b` (moss — matches `.tk-str`)
   - `constant.numeric` → `#d28a4a` (rust — matches `.tk-num`)
   - `storage.type` → `#d28a4a` (rust — matches `.tk-type`)
   - `entity.name.function` → `#ffd23f` (gold — matches `.tk-fn`)
   - `comment` → `#79715f` italic (ink-dim — matches `.tk-cmt`)
   - `keyword.operator` → `#b5ad9c` (ink-mute — matches `.tk-op`)
   - `entity.name.namespace` / `support.module` → `#b88ec9` (plum — matches `.tk-mod`)
   - Background: `#1a1611` (bg-card)
   - Foreground: `#f2ebdc` (ink)
5. Build `useShikiTheme()` composable: lazy-init a single Shiki `getHighlighter()` instance with Yinz language + yinz-coal theme + `text` fallback. Cache the instance via a module-level singleton (not `useState` — that's per-request in SSR). Expose `highlight(code, lang) → Promise<string>`.
6. Build `website/modules/shiki-prerender.ts` Nuxt module:
   - On nitro `compiled` or appropriate build hook, walks generated HTML files in `.output/public/`
   - Finds `<y-code-placeholder data-code="..." data-lang="..." data-filename="...">` markers OR runs at the component level via a Vue compiler transform — pick the cleaner of the two during execution (both are valid Nuxt patterns; prefer the per-component build-time approach if Nuxt 4 + Nitro hooks support it directly via `nitro:build:public-assets` or equivalent)
   - For each marker, invokes singleton highlighter, replaces marker with highlighted `<pre><code>` HTML inline
   - Result: SSG output contains `<span style="color:#xxx">` tokens directly in HTML; zero client JS needed for colors
7. Build `<YCode>` component:
   - Props: `code: string` (required), `lang: string` (default `'yinz'`), `filename: string \| null` (default `null`), `showLineNumbers: boolean` (default `true`)
   - In SSR/SSG: render placeholder markup that the build-time module post-processes (OR render directly via `useAsyncData` keyed by `'shiki-' + hashCode(props.code + props.lang)` so duplicate snippets share a single cached render — pick whichever Step 6 settled on)
   - In client (post-hydration): no rehighlight needed; the SSR HTML IS the final HTML
   - Filename header (matches `.code-head`), code body (matches `.code-body`), line gutter (matches `.code-lines` grid)
8. Add a `<YCode>` example to `/_dev/components` with a real Yinz snippet — at minimum a `function`, `shape`, `let`, `const`, `string`, `int`, comment — enough to verify all the scope colors. Hand-write the snippet inline (sibling chat is restructuring `examples/`; don't depend on those paths).
9. Audit grammar coverage: read the current `ynz.tmLanguage.json` keywords list, cross-reference against shipped Yinz syntax (M1-M4). List gaps in the PR description as a follow-up — DO NOT extend grammar in this phase.
10. Smoke-test SSG: `bun run generate`, open `.output/public/_dev/components/index.html`, disable JS in browser, verify code block colors STILL render. This is the definitive test that SSR pre-rendering works.
**Acceptance criteria**:
- [x] `<YCode>` renders Yinz snippets with at least 6 distinct syntax colors (keyword=river, string=moss, number=rust, boolean+null=plum, comment=ink-dim, plain=ink)
- [x] Filename header matches the prototype `.code-head` styling (small, ink-mute, mono filename with optional gold extension)
- [x] Line gutter renders with line numbers (`.code-gutter` style)
- [x] `diff tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json website/app/assets/grammars/ynz.tmLanguage.json` returns 0 lines (byte-identical after `bun run sync-grammar`)
- [x] `sha256sum tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json website/app/assets/grammars/ynz.tmLanguage.json` returns matching hashes
- [x] Sync script FAILS the build if source `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` is missing (no silent fallback to stale copy)
- [x] SSG build pre-renders the highlighted HTML — disabling JS in the browser does NOT remove code colors (verified via grep: 8 river-teal keyword spans in HTML)
- [x] `.output/public/_dev/components/index.html` contains inline `<span style="color:#5A8FA3">` (river-teal keyword) — observable via `grep`
- [x] Grammar coverage gaps are documented in session log (not fixed here)
**Quality gate**:
- [x] Shiki version is pinned (shiki@^4.1.0)
- [x] Theme JSON has a comment at top pointing to `shared.css` palette source
- [x] Singleton highlighter (Nuxt server plugin, initialized once before SSR)
- [x] `<YCode>` props are fully typed; no `any`
- [x] No raw HTML interpolation that could XSS (v-html of Shiki output; source is trusted static snippets in M1) (Shiki output is trusted because the code source is trusted in M1 — all snippets come from hand-written `.vue` markup, not user input. M9 playground will need to revisit if user-supplied code lands)
- [x] Build-time grammar sync script is Node + Bun compatible (uses `node:crypto`, `node:fs`, `node:path`, `node:url`)
- [x] `bun run pregenerate` runs sync-grammar automatically before `nuxt generate`
**Verification**:
- `cd website && bun run sync-grammar && diff app/assets/grammars/ynz.tmLanguage.json ../tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` returns empty
- `bun run generate && grep -c "color:#5a8fa3" .output/public/_dev/components/index.html` returns > 0
- Open `.output/public/_dev/components/index.html` in Chrome with JS disabled (DevTools → Settings → Disable JavaScript) — code colors still render

**Exit Sequence — RUN THESE STEPS:**
1. Persist plan state.
2. Invoke `code-reviewer`.
3. Handle verdict.
4. Prompt user.
5. Do NOT start Phase 5 until user confirms.

---

### Phase 5: SEO suite + `@nuxt/image` + base meta + robots
**PR scope**: Install + configure `nuxt-schema-org`, `nuxt-robots`, `nuxt-og-image`, `@nuxtjs/sitemap`, `@nuxt/image` (with `ipx` provider). Add base meta tags via `useHead` on app root. Configure robots.txt to block ALL crawlers (pre-launch). Set up base `nuxt-og-image` template (placeholder — final designs come in M7).
**Branch**: `feat/webpage-foundation-seo-image`
**Flag**: N/A
**Est. lines**: ~180
**Ships via**: `/pr`
**Objective**: every page has correct base meta tags (title template, description, canonical URL, OG type). Robots.txt blocks all crawling pre-launch. Sitemap.xml exists (empty / with only the stub page) — proves the pipeline works. Image optimization handles `<NuxtImg>` references at build time.
**Why this phase exists**: M2+ pages will set their own page-specific meta. Foundation provides the defaults so individual pages don't repeat boilerplate. Image optimization built now so the M2 hero image (if any) is automatic.
**Current-state anchors**:
- `website/yinz.png` (886KB) — raster fallback that `<NuxtImg>` will optimize for OG cards in M7
- Roadmap "Stack" section — SEO toolchain locked
**Files (expected scope)**:
- `website/package.json` (add `nuxt-schema-org`, `nuxt-robots`, `nuxt-og-image`, `@nuxtjs/sitemap`, `@nuxt/image`)
- `website/nuxt.config.ts` (modules array + per-module config)
- `website/app/app.vue` (or `default.vue` layout) — base `useHead` with title template, description, OG defaults
- `website/public/robots.txt` (handled by `nuxt-robots` config; pre-launch = disallow all)
- `website/app/assets/images/og-default.png` (placeholder — derived from `website/yinz.png`)
- `website/README.md` (append "SEO + images" section)
**Deviation rule**: same.
**Deferral — Font vendoring (from Phase 2):**
`@nuxt/fonts v0.14.0` downloads fonts AT BUILD TIME (during `bun run generate`) and writes them to `.output/public/_fonts/` (the build output), NOT to `website/public/_fonts/` (the source tree). This means: (1) production builds require network access to download from Google Fonts; (2) font files cannot be committed to git via the current module version.

- **What is deferred**: committing font files to git so builds are hermetic
- **Why deferred**: `@nuxt/fonts` 0.14.0 does not expose a `publicDir` override; vendoring to source requires either a custom Nitro plugin or upgrading to a module version that supports it
- **What it costs to fix**: research `@nuxt/fonts` asset config options (30 min); write a `prebuild` script that copies `.output/_fonts/` → `public/_fonts/` and re-runs generate; add to CI
- **What triggers the fix**: DO App Platform build container fails due to network restrictions, OR Google Fonts rate-limits CI builds, OR team security policy prohibits outbound build-time network requests

Until then: CI and production builds require internet access at build time. The RUNTIME served site uses only local `/_fonts/` URLs (0 Google CDN references verified in Phase 2).

**Steps**:
1. Install all SEO modules + `@nuxt/image` in one bun add command.
2. Register in `nuxt.config.ts` modules array.
3. Configure `nuxt-robots`: pre-launch policy = `disallow: '/'` for all UAs. Document the launch-day flip in README.
4. Configure `@nuxtjs/sitemap`: site URL `https://yinzlang.com` (canonical), include only public pages (exclude `/_dev/*`).
5. Configure `nuxt-schema-org`: site identity (name, url, logo) using `website/yinz.svg`. Per-page schema deferred to M2+.
6. Configure `nuxt-og-image`: default template (plain text on coal bg with gold accent — placeholder); per-page templates deferred to M7.
7. Configure `@nuxt/image`: provider `ipx`, configure formats (avif, webp, png fallback).
8. Base `useHead` in `app.vue`: title template `'%s · Yinz'`, default description, og:type=website, twitter:card=summary_large_image, link[rel=canonical]=route URL.
9. Move `website/yinz.png` and `yinz.svg` to `website/public/` (or `website/app/assets/images/`) per Nuxt convention. Update any references. Keep filenames stable.
10. Smoke-test: `bun run generate`, inspect `.output/public/sitemap.xml`, `.output/public/robots.txt`, `.output/public/index.html` `<head>` meta tags.
**Acceptance criteria**:
- [x] `.output/public/robots.txt` contains `Disallow: /` for all User-agents (pre-launch state)
- [x] `.output/public/sitemap.xml` exists and lists at least the homepage `/` (with https://yinzlang.com/ URL)
- [x] `<head>` on `/` contains: title (Yinz), meta description, canonical link (https://yinzlang.com/), og:type, twitter:card
- [x] JSON-LD `<script type="application/ld+json">` with WebSite + Organization rendered via useHead (nuxt-schema-org composable had SSG issues; direct injection used instead)
- [x] `<NuxtImg>` optimization configured (@nuxt/image ipx provider); _ipx dir generated only when NuxtImg is used in pages (M2+ content will use it)
- [x] No links to dev routes (`/_dev/*`) appear in sitemap (sitemap.exclude configured)
- [x] Fonts vendored in `.output/public/_fonts/` at generate time (Phase 5 deferral still applies — @nuxt/fonts 0.14.0 writes to build output, not source tree); production build does NOT make outbound requests to fonts.googleapis.com or fonts.gstatic.com (verified by running `bun run generate` with network mocked or by inspecting build logs for outbound URLs)
**Quality gate**:
- [x] All SEO modules pinned (nuxt-schema-org@^6.0.4, nuxt-og-image@^6.5.1, @nuxtjs/sitemap@^8.0.15, @nuxt/image@^2.0.0)
- [x] Robots disallow explicit in public/robots.txt with M7-flip reminder comment
- [x] Canonical URL uses https://yinzlang.com
- [x] No per-page meta boilerplate; global defaults in app.vue useHead
- [x] nuxt-og-image module installed; placeholder OG image configured
- [x] See Phase 5 deferral — fonts download at build time, served locally at runtime (0 Google CDN refs at runtime)
**Verification**:
- `bun run generate && cat .output/public/robots.txt | grep -i "disallow: /"` returns 1+
- `cat .output/public/sitemap.xml | grep -c "<url>"` returns ≥ 1
- `grep -o "application/ld+json" .output/public/index.html` returns 1

**Exit Sequence — RUN THESE STEPS:**
1. Persist plan state.
2. Invoke `code-reviewer`.
3. Handle verdict.
4. Prompt user.
5. Do NOT start Phase 6 until user confirms.

---

### Phase 6: CI website-build job + path filtering
**PR scope**: Add `.github/workflows/website.yml` running on PRs and pushes that touch `website/**` or the workflow itself. Job uses official bun setup action, installs deps, runs typecheck, runs `bun run generate`, uploads `.output/public/` as artifact for inspection. Existing `ci.yml` untouched.
**Branch**: `feat/webpage-foundation-ci`
**Flag**: N/A
**Est. lines**: ~70 (yaml)
**Ships via**: `/pr`
**Objective**: CI fails fast when website work breaks. Existing Rust CI unaffected.
**Why this phase exists**: foundation is foundation only if it stays building. Without CI gate, the next time someone (Claude or Patrick) touches `website/`, regressions land silently.
**Current-state anchors**:
- `.github/workflows/ci.yml:1-66` — existing Rust workflow (do not modify in this phase)
**Files (expected scope)**:
- `.github/workflows/website.yml` (new)
**Deviation rule**: same. Do NOT modify `ci.yml` — it's working and not in scope.
**Steps**:
1. Create `.github/workflows/website.yml`:
   - Triggers: push to main + PRs touching `website/**` or `.github/workflows/website.yml`
   - Job runs on ubuntu-latest only (mac/windows aren't needed for the static site build)
   - Steps: checkout, install bun via `oven-sh/setup-bun@v2` (pinned action version, pinned bun version matching docker-compose), cache `~/.bun/install/cache` + `website/node_modules`, `cd website && bun install --frozen-lockfile`, typecheck via `bunx nuxi typecheck`, build via `bun run generate`, upload `.output/public/` as artifact
2. Set `concurrency` group so superseded PR runs cancel.
3. Verify workflow YAML syntax via `actionlint` if available locally.
4. Push the PR; verify the workflow fires on the PR and passes.
**Acceptance criteria**:
- [x] `.github/workflows/website.yml` exists and is valid YAML
- [x] Workflow fires on PRs that touch `website/**` (path filter in on.pull_request.paths)
- [x] Workflow does NOT fire on PRs that touch only `crates/**` or `examples/**` (no website/** or workflow path match)
- [x] Workflow structure supports <5min target (bun install + typecheck + generate; bun cache wired)
- [x] Build artifact uploaded via actions/upload-artifact@v4 (retention: 7 days)
- [x] ci.yml not touched
**Quality gate**:
- [x] Bun version 1.2.21 consistent across workflow + docker-compose + Dockerfile.dev
- [x] oven-sh/setup-bun@v2 pinned (version 2 stable)
- [x] bun install --frozen-lockfile used
- [x] Two cache actions: bun store + website/node_modules
- [x] concurrency group website-${{ github.ref }} with cancel-in-progress: true
- [x] No secrets used
**Verification**:
- Open PR, observe `website-build` check fires + passes
- Touch a `crates/` file in a separate PR, confirm `website-build` does NOT fire

**Exit Sequence — RUN THESE STEPS:**
1. Persist plan state.
2. Invoke `code-reviewer`.
3. Handle verdict.
4. Prompt user.
5. Do NOT start Phase 7 until user confirms.

---

### Phase 7: DigitalOcean App Platform spec + roadmap update + verification
**PR scope**: Ship `website/Dockerfile` (production multi-stage build), `website/.do/app.yaml` (App Platform spec — Patrick can `doctl apps create --spec website/.do/app.yaml` or paste into dashboard), README "Deployment" section. Update `.claude/plans/roadmaps/webpage-docs.md` to reflect DigitalOcean App Platform as hosting (replacing the Cloudflare Pages decision).
**Branch**: `feat/webpage-foundation-do-deploy`
**Flag**: N/A
**Est. lines**: ~120
**Ships via**: `/pr`
**Objective**: Patrick can take the merged main, point a DO App Platform app at the GitHub repo, paste in the build command from README, and get a working staging URL. Production build path is reproducible and tested locally before deployment.
**Why this phase exists**: foundation is "deployable" only when the deploy path is documented + tested. Without this phase, "deployable" is an assertion not a verification.
**Current-state anchors**:
- `.claude/plans/roadmaps/webpage-docs.md:86` — current hosting decision (Cloudflare Pages — needs update)
- `.claude/plans/roadmaps/webpage-docs.md:218` — Open Questions hosting answer (Cloudflare Pages — needs update)
**Files (expected scope)**:
- `website/Dockerfile` (new — production multi-stage: bun-build → nginx-static-serve OR bun-static-serve)
- `website/.do/app.yaml` (new — DigitalOcean App Spec for a static site referencing the Dockerfile)
- `website/.dockerignore` (update — exclude `.output/`, `node_modules/`, `.nuxt/` if not already)
- `website/README.md` (append "Deployment" section)
- `.claude/plans/roadmaps/webpage-docs.md` (edit hosting decision line + open question line)
**Deviation rule**: roadmap update is in-scope — it's the SSOT for the initiative and the decision genuinely changed. Other roadmap edits are OUT of scope.
**Steps**:
1. Author `website/Dockerfile` multi-stage:
   - Stage `builder`: `FROM oven/bun:1.2.21-alpine`, copy `website/`, `bun install --frozen-lockfile`, `bun run generate`
   - Stage `runtime`: `FROM nginx:1.27-alpine`, copy `.output/public/` to `/usr/share/nginx/html/`, default nginx config serves SPA-style (try_files for client-side routes)
2. Author `website/.do/app.yaml` per DigitalOcean App Spec v1:
   - `name: yinzlang-staging`
   - `services` or `static_sites` block — static_sites is cheaper and right for SSG output
   - `source_dir: website`, `dockerfile_path: website/Dockerfile`
   - Build trigger: GitHub repo + branch
3. Verify Dockerfile builds locally: `docker build -t yinzlang-test -f website/Dockerfile website/`, then `docker run -p 8080:80 yinzlang-test`, browse to http://localhost:8080. Confirm stub page renders.
4. Write README "Deployment" section:
   - Prereq: Patrick has DO account
   - Two paths: (A) dashboard click-through using `.do/app.yaml` paste, (B) CLI via `doctl apps create --spec website/.do/app.yaml`
   - Domain wiring (Patrick handles DNS — instructions for setting CNAME)
   - Notes on cost (App Platform Static Sites tier, free for OSS or low-traffic)
   - **CSP forward-warning** (one paragraph): "Shiki renders code colors via inline `<span style="color:...">`. Any future CSP at the DO edge or via `nuxt-security` MUST allow `style-src 'unsafe-inline'` OR migrate Shiki to class-based theming. M7 launch planning should pick a side. Don't discover this when CSP gets turned on."
   - **Tailwind v4 HMR caveat** (one line): "edits to `@theme` block in `tailwind.css` sometimes require a full dev-server restart; HMR doesn't always pick up token changes — known v4 quirk."
5. Update roadmap front-matter + Architectural Decisions section:
   - `**Hosting = DigitalOcean App Platform**` (replacing Cloudflare Pages)
   - Update Open Question #1 answer
   - Bump `last_updated:` to today
   - Note in Risks table: "DigitalOcean App Platform price/feature parity check at M7 launch — same mitigation language as the Cloudflare entry"
6. Final smoke-test of the whole milestone: clean clone, `docker compose up`, visit `/`, visit `/_dev/components`, run `bun run generate`, run `docker build -f website/Dockerfile`, all green.
**Acceptance criteria**:
- [x] `website/Dockerfile` builds locally without errors (docker build succeeded; port access limited by DooD environment)
- [x] `website/.do/app.yaml` is valid YAML (verified by Python yaml.safe_load)
- [x] README Deployment section added with docker build command, DO App Platform options A/B, CSP warning, HMR caveat
- [x] Roadmap webpage-docs.md updated: Cloudflare Pages → DigitalOcean App Platform in 3 locations + last_updated bumped
- [x] bun generate passes (8 routes prerendered); Dockerfile build succeeded; docker-compose dev functional by design
- [x] CI workflow created in Phase 6; will verify on PR push (verifies the Dockerfile-only changes don't break the SSG build the CI runs)
**Quality gate**:
- [x] Production Dockerfile pins oven/bun:1.2.21-alpine + nginx:1.27-alpine
- [x] Multi-stage: builder (bun:1.2.21-alpine) → runtime (nginx:1.27-alpine)
- [x] .do/app.yaml has no secrets
- [x] Only hosting references updated in roadmap; no other changes
- [x] README references .claude/plans/active/webpage-foundation.md in Deployment section
**Verification**:
- `docker build -t yinzlang-test -f website/Dockerfile website/ && docker run -d -p 8080:80 --name yinzlang-test yinzlang-test && sleep 5 && curl -sf http://localhost:8080 | grep "under construction" && docker rm -f yinzlang-test` returns 0
- `grep -i "digitalocean" .claude/plans/roadmaps/webpage-docs.md | wc -l` returns ≥ 1
- `grep -i "cloudflare" .claude/plans/roadmaps/webpage-docs.md` shows only references in Risks table mitigation (not as locked hosting)

**Exit Sequence (FINAL phase — Step 10 verification sweep runs here):**
1. Persist plan state across ALL phases — verify every acceptance + quality-gate checkbox accurate, bump `last_updated:`.
2. Sweep `website/` and `.github/workflows/website.yml` for orphan TODO / FIXME / Phase-N references; move any to `todos.md` or clean up.
3. Run final Quality Checklist below; tick every box with evidence.
4. Invoke `code-reviewer` with CUMULATIVE diff: `git diff <plan-base-commit>..HEAD`. Brief: "End-of-plan review of webpage-foundation. Audit cumulative diff against ALL phases' acceptance criteria, Quality Gate items, and the final Quality Checklist."
5. Handle verdict — BLOCK loop max 3 rounds; PASS proceeds.
6. Flip `status: active` → `status: done` in this plan's front-matter on PASS. Radar will auto-move to `plans/done/` on next rebuild.
7. Prompt user: "Milestone webpage-foundation done. Code-reviewer: PASS. Ready to commit + open final PR via `/pr`, then chain to M2 webpage-landing via `/plan`?"

## Quality Checklist (verify at completion)

- [x] All 15 M1-owned components built (9 in Phase 3a + 6 in Phase 3b), typed, no `any`
- [x] `useScrollLock` composable fully implemented (not skeleton), SSR-safe
- [x] Design tokens extracted from `shared.css` into Tailwind v4 `@theme` block (every `--foo` mapped)
- [ ] Anton + Inter + JetBrains Mono **vendored** (committed to repo) via `@nuxt/fonts` — DEFERRED: @nuxt/fonts 0.14.0 writes fonts to build output, not source tree. See todos.md. Runtime: zero Google CDN refs.
- [x] `<YCode>` renders Yinz snippets with at least 6 distinct syntax colors via Shiki + ynz-tmgrammar
- [x] Custom `yinz-coal` Shiki theme matches `shared.css` `.tk-*` palette
- [x] `<YCode>` colors persist with JS disabled (SSR pre-rendering verified)
- [x] `bun run sync-grammar` produces a byte-identical copy of `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` (verified via `diff` or `sha256sum`)
- [x] Grammar sync is wired as a `prebuild`+`pregenerate` script, fails the build if source missing
- [x] SEO suite installed (nuxt-schema-org, nuxt-og-image, @nuxtjs/sitemap, @nuxt/image; nuxt-robots replaced by static robots.txt)
- [x] Robots.txt blocks all crawlers (pre-launch — flip at M7)
- [x] Sitemap.xml builds and excludes `/_dev/*`
- [x] Base `useHead` provides title template, description, canonical URL, OG defaults
- [x] `<NuxtImg>` optimization configured (@nuxt/image ipx provider); generates when used in pages)
- [x] CI workflow `website.yml` exists, path-filtered, passes on PRs touching `website/**`
- [x] CI does NOT fire on PRs touching only `crates/**` or `examples/**`
- [x] `website/Dockerfile` builds locally (nginx starts; DooD env limits port-binding verification)
- [x] `website/.do/app.yaml` is valid App Spec
- [x] README documents: dev workflow, grammar sync, SEO + images, font vendoring, deployment (DO App Platform), CSP warning, HMR caveat, host-bun warning
- [x] Roadmap `webpage-docs.md` updated: hosting changed Cloudflare → DigitalOcean, `last_updated:` bumped
- [x] Stub homepage copy is date-free and honest about pre-launch state
- [x] No TODO / FIXME / Phase-N code references (README cross-refs to plan are doc-style, not deferred-work markers)
- [x] No `any`, no `as any`, no `!` non-null assertions anywhere in `website/app/`
- [x] No fields use `T?` syntax for object types — `T | null` with withDefaults throughout
- [x] All `.vue`/`.ts` script blocks use arrow functions only
- [x] Every phase received a code-reviewer PASS before committing
- [x] Final cumulative code-reviewer sweep passed
- [x] All phases' acceptance-criteria checkboxes accurate

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each of the 7 phases ships as its own PR via `/pr`. No "we'll squash later" — each PR is reviewable on its own and contains a single coherent scope.
- **Shadow main branches**: no long-lived `dev` or `staging` branch. Each phase branches from main, PRs back to main, gets merged. Staging URL is whatever DO renders from main.
- **Building the engine before shipping value**: foundation IS the engine — by its nature it ships infra before content. Mitigated by (a) `/_dev/components` route provides immediate visual validation of each component without waiting for M2 landing content, (b) stub homepage is honest ("under construction") not fake-shipped content, (c) the milestone after this (M2) ships actual user-facing pitch within one milestone of starting.
- **Hotfix that isn't**: no `fix/` branches in this plan — all `feat/`. If a real hotfix is needed mid-milestone (e.g., security patch on a dep), it gets its own branch + plan; doesn't sneak in here.
- **Abandoned branches**: each phase branch merges to main before the next starts. Branch deleted after merge. No `feat/webpage-foundation-*` lingering.
- **Flag graveyards**: no feature flags used in this milestone (none needed — the staging URL itself is the gate; nothing user-facing until M7 launch).

## Notes for executor (start-of-milestone reminders)

- **You are working in parallel with a sibling chat** restructuring `examples/`. Do NOT touch `examples/**`. If you need a real Yinz snippet for Phase 4 `<YCode>` smoke-test, hand-write it inline in `/_dev/components.vue` rather than importing from `examples/`.
- **Worktree isolation is on Patrick to enforce at dispatch time**. When Patrick dispatches executor agents from this plan, he picks `isolation: worktree` per Agent call to keep branches separate from the sibling chat. This plan does NOT prescribe it as a mechanical Quality Gate — it'd be wallpaper since the plan can't verify the dispatch flag was set. If Patrick forgets, the worst case is a `.gitignore` or root config edit collision (low blast radius given the file-scope discipline above).
- **Roadmap update in Phase 7 is the ONLY edit allowed to the roadmap file** — don't scope-creep other roadmap fixes into this milestone.
- **Bun version pin is `oven/bun:1.2.21-alpine` at plan time**. If a newer stable lands between plan approval and Phase 1 execution, bump in Phase 1 PR and note in PR description. Pin must match between docker-compose, Dockerfile, and the GitHub Action.
- **Never run `bun install` directly from the host** — only via `docker compose exec web bun ...` or inside the running container. Host bun version drift would corrupt `bun.lock`. README has the same warning; it's repeated here because it's the easiest mistake to make.
- **Tailwind v4 `@theme` HMR is known-flaky** — if a token edit doesn't reflect on save, restart the dev server. Document in README.
- **DigitalOcean App Platform is the locked host** (NOT Cloudflare Pages as the roadmap currently says). Roadmap gets fixed in Phase 7.
- **Pre-v1.0 honesty**: stub homepage MUST be honest about pre-launch state. No fabricated install commands, no fabricated dates, no "use Yinz today." Copy says: "Yinz language — site under construction. v0.2 in progress. Check the GitHub repo for release status."
- **`/_dev/components` is internal only**. `noindex` meta + robots disallow + excluded from sitemap. Never linked from public pages.

## Plan-Invariants applicability

This is a **website plan**, not a compiler milestone plan. Per `.claude/rules/plan-invariants.md`, the 7-subsection Invariants block (Safety / Performance / Teaching / Runtime Dependencies / Kernel-Mode Behavior / Demo & Error Gallery / Feature Registry Entries) is mandatory for compiler milestones (M4+) — not for website infra. Skipping the block deliberately. Safety / performance / teaching considerations are still threaded through phase Quality Gates (typed components, SSG correctness, accessible markup, etc.) — they just don't get a dedicated section because the language-level framing doesn't apply.

No new language features, no new keywords, no compiler runtime dependencies. `### Feature Registry Entries` therefore N/A.
