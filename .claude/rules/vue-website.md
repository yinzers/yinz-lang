---
paths:
  - 'website/**'
---

> Extends `~/.claude/rules/vue.md` (generic Vue 3 + reusability mandate — auto-loaded). Below: PROJECT-specific deltas only.

# Nuxt 4 + Tailwind v4 + Bun — Website Standards

The Yinz website (`yinzlang.com`) is a Nuxt 4 SSG site under `website/`. Stack locked per `.claude/plans/roadmaps/webpage-docs.md`:

- **Nuxt 4** with the `app/` directory layout (auto-imports for components, composables, utils)
- **Bun** for all package management — pinned in `website/docker-compose.yml`; never run `bun install` from the host
- **Tailwind v4** via `@tailwindcss/vite` plugin (NOT `@nuxtjs/tailwindcss`); design tokens live in `app/assets/css/tailwind.css` as a `@theme` block
- **TypeScript** strict, `<script setup lang="ts">` everywhere
- **SSG only** — `bun run generate` produces static HTML; no runtime server, no API endpoints, no auth, no DB
- **Components prefix `Y*`** (`YButton`, `YNav`, `YCode`) — keeps our primitives visually distinct from Nuxt built-ins (`NuxtLink`, `NuxtImg`)
- **No Pinia, no Reka UI, no Lucide, no Axios, no fetch wrapper** — the MVP site has no cross-page state to manage, no interactive widgets beyond the mobile nav drawer, and no runtime API calls. If a real need emerges, justify it in the plan first.

All site code is dark-only (coal background per `shared.css`); there is no light theme.

---

## File Organization (Nuxt 4 `app/` directory)

```
website/
  app/
    components/
      primitives/    # YButton, YCard, YCode, YNav, etc. — reusable everywhere
      marketing/     # YHero, YPillarGrid, YCompare — landing-page surface (M2+)
      docs/          # YSidebar, YTOC, YBreadcrumb — docs surface (M4+)
      blog/          # YPostCard, YFilterChips — blog surface (M6+)
    composables/     # useScrollLock, useShikiTheme, etc. (auto-imported)
    layouts/         # default.vue + others
    pages/           # / and nested routes — file-based routing
    assets/
      css/           # tailwind.css (@theme block lives here)
      grammars/      # ynz.tmLanguage.json (synced from tooling/vscode-ynz/)
      themes/        # yinz-coal.json (Shiki theme)
    utils/           # pure helpers (no Vue API usage)
    app.vue          # root component
  public/            # static assets served as-is (favicons, robots.txt, fonts)
  modules/           # custom Nuxt modules (e.g., shiki-prerender)
  build/             # build-time scripts (e.g., sync-ynz-grammar.ts)
  nuxt.config.ts
```

Nuxt auto-imports anything under `app/components/`, `app/composables/`, and `app/utils/`. Don't write manual imports for these.

---

## Styling: `<style>` Block Exceptions

**No `<style>` blocks for normal styling — use Tailwind utility classes.** The exceptions:

1. **`<Transition>` animations** (see global vue.md)
2. **Complex CSS Tailwind utilities cannot express** — pseudo-elements (`::before` grain on body), `color-mix()` / `backdrop-filter` cocktails not yet in Tailwind, grid-template subgrid overrides, the prototype's `.code-gutter` grid pattern
3. **Token-driven styles that would create utility-class soup** — e.g., a multi-stop `linear-gradient(var(--gold), var(--gold-deep))` border accent reads cleaner as scoped CSS than as a long arbitrary-value Tailwind class

When you do use `<style scoped>`, reference design tokens via CSS variables (`var(--color-gold)`, `var(--font-display)`) so the `@theme` block stays the single source of truth.

---

## Reactivity: Nuxt-Specific Additions

### SSR-safe state: `useState`

For state that must survive SSR → hydration (e.g., a flag set during server render), use Nuxt's `useState`. It's like `ref` but SSR-safe and globally identified.

```typescript
// ✅ Survives SSR → hydration
const featureFlags = useState<Record<string, boolean>>('feature-flags', () => ({}));
```

For purely client-side state (mobile drawer open/closed, scroll position), regular `ref` is fine inside a composable.

### Avoid global mutable state

The MVP site has no cross-page state. If a need emerges (e.g., persisted theme toggle, search filter state across pages), use `useState` — do NOT install Pinia for a single store.

---

## Watchers: SSR Caveat

`watchEffect` runs during SSR where `document`/`window` don't exist. For page titles + meta tags, ALWAYS use `useHead` / `useSeoMeta` — never poke `document.title` directly.

```typescript
// ❌ Throws on SSR — document not defined
watchEffect(() => {
  document.title = pageTitle.value;
});

// ✅ Use Nuxt's useHead instead
useHead({ title: () => pageTitle.value });
```

---

## Components: Nuxt 4 Specifics

### `Y*` prefix for our primitives

Every component we author is prefixed `Y` to distinguish from Nuxt built-ins (`NuxtLink`, `NuxtImg`, `ClientOnly`, `Suspense`) and any third-party additions:

```vue
<!-- ✅ -->
<YButton variant="primary">Get started</YButton>
<YCode :code="snippet" lang="yinz" filename="example.ynz" />

<!-- ❌ Don't shadow Nuxt names -->
<Link to="/docs">Docs</Link>
```

### PascalCase in templates (required for Nuxt auto-import)

```vue
<!-- ✅ -->
<YPill variant="shipped">v0.1</YPill>

<!-- ❌ Auto-import doesn't match kebab-case -->
<y-pill variant="shipped">v0.1</y-pill>
```

### Internal routing: NuxtLink

```vue
<!-- ✅ Internal — Nuxt handles SPA navigation + prefetching -->
<NuxtLink to="/docs/getting-started">Get started</NuxtLink>

<!-- ✅ External — plain anchor, target=_blank as needed -->
<a href="https://github.com/yinzers/yinz-lang" target="_blank" rel="noopener noreferrer">GitHub</a>

<!-- ❌ Don't use <a href="/docs"> for internal routes — full reload, no prefetching -->
```

### Images: NuxtImg

Use `<NuxtImg>` for every raster image so `@nuxt/image` generates avif/webp variants with lazy-loading and responsive `srcset`.

```vue
<NuxtImg
  src="/og-default.png"
  alt="Yinz language"
  width="1200"
  height="630"
  loading="lazy"
/>
```

Use `<img>` only for inline SVG references where format optimization is moot.

### Prefer auto-import over manual imports

Nuxt auto-imports anything in `app/components/**`. Don't manually import:

```typescript
// ❌ Manual import (redundant in Nuxt 4)
import YButton from '~/components/primitives/YButton.vue';

// ✅ Just use it — Nuxt handles the import
// <YButton>Click</YButton>
```

---

## Composables: SSR-safe by default

Composables run during SSR. Any DOM access must be guarded:

```typescript
export const useViewportSize = () => {
  const width = ref(0);

  onMounted(() => {
    width.value = window.innerWidth;
    window.addEventListener('resize', () => { width.value = window.innerWidth; });
  });

  return { width };
};
```

Never access `window`, `document`, `localStorage`, or `navigator` at the top level of a composable.

---

## Data Fetching (when we need it)

The MVP site is fully static — no runtime fetching. Data lives in markdown files or build-time TOML files, read at build time.

### Use `useAsyncData` with native `$fetch`

```typescript
// Build-time: this runs during nuxi generate
const { data: roadmap } = await useAsyncData('roadmap', () =>
  $fetch<RoadmapEntry[]>('/api/roadmap.json'),
);
```

For reading local files at build time, use a Nuxt module or `nitro:build:before` hook that emits a JSON file under `public/` or generates a virtual `~/data/...` import. Don't ship runtime API calls.

### NO Axios, NO fetch wrapper, NO `API_BASE` constants

The MVP site doesn't hit any APIs at runtime. If a runtime fetch becomes necessary (it shouldn't pre-v1.0), use Nuxt's native `$fetch` — it's a typed wrapper around fetch that integrates with SSR.

---

## Templates: Project-Specific Notes

### `v-html` exception for Shiki

The only place `v-html` appears in our site is `<YCode>` rendering Shiki's pre-highlighted HTML — and Shiki's output is trusted because the `code` prop is hand-authored in `.vue` markup, NOT user input. M9 playground will need to revisit this if user-supplied code lands.

---

## Tailwind v4

### Tokens live in `@theme` — not in components

All colors, fonts, radii, breakpoints, and container widths live in `app/assets/css/tailwind.css` under the `@theme` block. Source of truth is the prototype `shared.css` `:root` vars.

```css
/* app/assets/css/tailwind.css */
@import "tailwindcss";

@theme {
  --color-bg: #14110d;
  --color-gold: #ffd23f;
  --color-ember: #c8442a;
  /* ...full palette per shared.css... */

  --font-display: "Anton", ui-sans-serif, system-ui, sans-serif;
  --font-sans: "Inter", ui-sans-serif, system-ui, sans-serif;
  --font-mono: "JetBrains Mono", ui-monospace, monospace;

  --radius: 10px;
  --container-max: 1240px;
}
```

NEVER hardcode a hex code in a component. NEVER hardcode a font family. NEVER hardcode a radius value. Always reference the theme token via a Tailwind utility (`bg-bg`, `text-gold`, `rounded-lg`, `font-display`) or — for scoped CSS — via `var(--color-gold)`.

### `@apply` threshold

Don't `@apply` unless a combination repeats **3+ times** (stricter than the global 2+ rule for this project).

### Dark mode: N/A

Site is dark-only. No `dark:` variants. No theme toggle. The coal background is the brand.

### Responsive breakpoints

Breakpoints from the prototype: `<600px` (`sm`), `<900px` (`md`), default `lg`. Tailwind v4 ships these out of the box.

### `@theme` HMR caveat

Edits to `@theme` block sometimes don't HMR cleanly. If a token edit doesn't reflect on save, restart the dev server (`docker compose restart web`).

---

## SEO (Nuxt SEO suite)

Use Nuxt's SEO primitives — never poke `document.head` or `<meta>` manually.

### `useHead` for per-page title + meta

```typescript
useHead({
  title: 'Getting Started',
  meta: [
    { name: 'description', content: 'Install Yinz and write your first program in 5 minutes.' },
  ],
});
```

The base title template lives in `app/app.vue` (e.g. `'%s · Yinz'`) — per-page only sets the `%s` portion.

### `useSeoMeta` for OG + Twitter cards

```typescript
useSeoMeta({
  ogTitle: 'Getting Started',
  ogDescription: 'Install Yinz and write your first program in 5 minutes.',
  ogImage: '/og/docs-getting-started.png',
  twitterCard: 'summary_large_image',
});
```

For per-page generated OG images, `nuxt-og-image` renders them at build time.

### Structured data via `nuxt-schema-org`

Schema.org JSON-LD via `useSchemaOrg` — only on pages where meaningful (homepage Organization, docs Article, blog BlogPosting).

---

## Yinz Code Snippets (Anti-Drift Rule)

Every Yinz snippet rendered on the site MUST be anchored to a real `.ynz` file in `examples/website/` (or equivalent — exact path locked by M5). CI builds + runs that file; output is captured via `insta` snapshots. The snippet on the site comes from the file, not from copy-pasted source in a `.vue` markup.

```vue
<!-- ❌ Banned in production pages — drifts from compiler -->
<YCode lang="yinz" :code="`function hello() { ... }`" />

<!-- ✅ Sourced from a real .ynz file at build time -->
<YCode lang="yinz" :code="snippets.helloWorld" filename="hello.ynz" />
```

---

## Performance: SSG-Specific Notes

### SSG-first — no runtime JS for static content

The whole site renders to static HTML at build time. Components used only for static content emit zero runtime JS beyond Vue's hydration.

Use `<ClientOnly>` to wrap genuinely client-side-only widgets (a future search input, a playground editor) so they don't run during SSR.

```vue
<ClientOnly>
  <YSearchBox />
</ClientOnly>
```

### Don't ship Pagefind index on the home page

Pagefind (search, M4) ships its index as separate `.pf_*` files lazy-loaded only when the search input is focused. Foundation phases don't touch this.

---

## What's NOT in this Stack (and Why)

| Thing | Why we don't use it |
|---|---|
| **Pinia** | No cross-page state in MVP. Use Nuxt `useState` for the one-off case. Revisit only if multiple unrelated components need shared state. |
| **Reka UI** | No interactive widgets in MVP beyond `<YNav>` mobile drawer. Build primitives ourselves to match the locked aesthetic. Revisit if we add Dialog/Combobox/Popover. |
| **Lucide** (or any icon library) | MVP needs zero icons. The block-Y logo mark + small SVG chevrons in `.prose` lists are inline SVG. Revisit only when a real icon need surfaces. |
| **Axios / fetch wrapper / `api.*` helper** | Site is fully static SSG. No runtime API calls. Build-time data via `useAsyncData` + local files. |
| **`@nuxtjs/tailwindcss` module** | Doesn't support Tailwind v4 yet (nuxt-modules/tailwindcss#820). We use `@tailwindcss/vite` directly. |
| **Light theme / theme toggle** | Site is dark-only. Coal background is the brand. |
| **Cookie banners / analytics consent UI** | Selected analytics tools (GoatCounter / Umami / Plausible — final choice TBD at M7) all run consent-free under GDPR. |
| **i18n** | English only at MVP. |
| **CMS, auth, user accounts** | All content is MD-in-repo. No backend. |

If a future milestone genuinely needs one of these, justify it in the plan first — don't sneak it in.

---

## Cross-References

- `~/.claude/rules/coding-style.md` — `T | null` over `T?`, no `any`, arrow functions, satisfies/as guidance
- `.claude/plans/roadmaps/webpage-docs.md` — locked stack decisions (Nuxt 4, Tailwind v4, Bun, hosting, fonts, SSG)
- `.claude/plans/active/webpage-foundation.md` — current milestone (component inventory, phase breakdown)
- `/tmp/yinz-design/yinz/project/shared.css` — design token source-of-truth (extracted into `@theme` block in foundation Phase 2)
