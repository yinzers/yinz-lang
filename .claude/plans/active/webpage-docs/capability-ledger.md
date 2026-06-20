# Capability Ledger: Webpage + Docs

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
