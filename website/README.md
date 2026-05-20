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

## Deployment (DigitalOcean App Platform)

See Phase 7 of `.claude/plans/active/webpage-foundation.md` for the full deployment spec. Summary:

- Production target: **DigitalOcean App Platform** (static site tier)
- Build command: `bun run generate`
- Output directory: `.output/public/`
- Config: `website/.do/app.yaml` (ships in Phase 7)
- Dockerfile: `website/Dockerfile` (ships in Phase 7)

Patrick handles DNS + App Platform wiring. See the plan for step-by-step instructions.
