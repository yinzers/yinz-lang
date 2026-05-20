# Yinz Language Website

Static site for [yinzlang.com](https://yinzlang.com). Built with [Nuxt 4](https://nuxt.com/) and [Bun](https://bun.sh/).

---

## Dev workflow

**Prerequisite**: Docker installed and running.

```bash
# Start the dev server on http://localhost:6002
docker compose -f website/docker-compose.yml up
```

First run downloads the pinned bun image and installs dependencies — subsequent runs start in seconds.

**Port 6002** is deliberate: it avoids collisions with other dev services in this repo.

**Hot reload** works with file edits. If a `@theme` block edit in `tailwind.css` doesn't reflect on save, restart the dev server — this is a known Tailwind v4 HMR quirk.

**WSL2 users**: polling watcher is enabled in `nuxt.config.ts` (`vite.server.watch.usePolling = true`). If you see hot-reload delays, this is normal for WSL2 bind mounts.

> **Warning — always install via docker compose, never bare `bun install` from the host.**
> Host bun version drift would corrupt `bun.lock` if your host bun differs from the pinned image version (`1.2.21`).

---

## Production build

```bash
docker compose -f website/docker-compose.yml exec web bun run generate
```

Output lands in `website/.output/public/` — this is the static site artifact.



---

## Design tokens

All color, font, radius, and spacing values come from `website/app/assets/css/tailwind.css` — a `@theme` block that maps the Yinz palette to Tailwind v4 utilities.

Historical reference: `/tmp/yinz-design/yinz/project/shared.css` (the design prototype CSS). **Do not import it** — the tokens were extracted from it; it is not a build dependency.

**Using tokens in components:**

```html
<!-- color -->
<div class="bg-bg text-ink border border-line">...</div>
<span class="text-gold">Pittsburgh gold</span>

<!-- typography -->
<h1 class="font-display">Anton headline</h1>
<p class="font-sans">Inter body</p>
<code class="font-mono">JetBrains Mono</code>

<!-- radii -->
<div class="rounded rounded-lg">...</div>
```

**Font delivery**: `@nuxt/fonts` downloads Anton, Inter, and JetBrains Mono from Google Fonts at build time and serves them from `/_fonts/` in the generated output. Production builds require internet access to Google Fonts at build time; the browser never hits Google CDN at runtime. See the Phase 5 vendoring deferral in `.claude/plans/active/webpage-foundation.md` for the follow-up tracking committing vendored font files for hermetic builds.

---

## Yinz grammar sync

`<YCode>` uses `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` as the source of truth for Yinz syntax highlighting. The `build/sync-ynz-grammar.ts` script copies it to `website/app/assets/grammars/ynz.tmLanguage.json` and verifies the sha256 matches.

The `pregenerate` and `prebuild` scripts run sync automatically before any build. To run manually:

```bash
docker compose exec web bun run sync-grammar
```

The vendored copy in `app/assets/grammars/` is checked into git so developers can browse the site without running the sync first. If the VSCode extension grammar changes, pull main and run sync-grammar — the prebuild hook handles it automatically otherwise.

**Grammar coverage as of Phase 4**: keywords, comments, numbers, booleans/null, string template literals (backtick), banned/deferred keyword highlighting. Function names and type names are not yet in the grammar — these appear in plain foreground color. See PR description for coverage gaps.

---

## Deployment (DigitalOcean App Platform)

**Build context**: Dockerfile uses the **repo root** as context (not `website/`) because it needs access to `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` for the grammar sync step.

### Option A — App Platform dashboard (recommended)

1. Push the repo to GitHub (`yinzers/yinz-lang`)
2. In DO App Platform: New App → GitHub → select `yinzers/yinz-lang`, branch `main`
3. App Spec: paste contents of `website/.do/app.yaml`
4. Deploy
5. Wire custom domain → `yinzlang.com` CNAME in DNS

### Option B — CLI

```bash
doctl apps create --spec website/.do/app.yaml
```

### Local production build (verify before deploying)

```bash
# Build from repo root (required — Dockerfile COPY needs tooling/ directory)
docker build -f website/Dockerfile -t yinzlang-prod .
docker run -p 8080:80 yinzlang-prod
# → http://localhost:8080
```

### CSP forward-warning

Shiki renders code colors via inline `<span style="color:...">`. Any future CSP at the DO edge or via `nuxt-security` MUST allow `style-src 'unsafe-inline'` OR migrate Shiki to class-based theming. See Phase 5 deferral in `.claude/plans/active/webpage-foundation.md` for the vendoring follow-up.

**Tailwind v4 `@theme` HMR caveat**: edits to the `@theme` block in `app/assets/css/tailwind.css` sometimes require a full dev-server restart to reflect. Known v4 quirk.

**Always install via docker compose in dev** — never `bun install` from the host; host bun version drift would corrupt `bun.lock`.
