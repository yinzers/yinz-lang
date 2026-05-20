---
paths:
  - 'website/**'
---

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

## Component Structure

Every `.vue` file follows this order:

```vue
<script setup lang="ts">
  // 1. Imports (rare — Nuxt auto-imports components, composables, refs/computed/etc.)
  // 2. Props & emits
  // 3. Composables & injections
  // 4. Reactive state
  // 5. Computed properties
  // 6. Watchers
  // 7. Functions (arrow only)
  // 8. Lifecycle hooks
</script>

<template>
  <!-- Single root element preferred but not required (Vue 3 supports fragments) -->
</template>
```

**No `<style>` blocks for normal styling — use Tailwind utility classes.** The exceptions:

1. **`<Transition>` animations** (see Transitions section)
2. **Complex CSS Tailwind utilities cannot express** — pseudo-elements (`::before` grain on body), `color-mix()` / `backdrop-filter` cocktails not yet in Tailwind, grid-template subgrid overrides, the prototype's `.code-gutter` grid pattern
3. **Token-driven styles that would create utility-class soup** — e.g., a multi-stop `linear-gradient(var(--gold), var(--gold-deep))` border accent reads cleaner as scoped CSS than as a long arbitrary-value Tailwind class

When you do use `<style scoped>`, reference design tokens via CSS variables (`var(--color-gold)`, `var(--font-display)`) so the `@theme` block stays the single source of truth.

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

## TypeScript: No Escape Hatches

Per `~/.claude/rules/coding-style.md`:

- **NEVER** use `any`. Use `unknown` + type narrowing if the shape is genuinely unknowable.
- **NEVER** use `as any` or `as unknown as T` (unless absolutely last-resort with a comment explaining why).
- **NEVER** use non-null assertion `!`. Use a guard: `if (!ref.value) return; ref.value.foo()`.
- **Use `T | null` for optional object fields, NOT `T?`** — `T?` lets construction sites silently omit the field. `T | null` forces every construction site to declare absence. Round-trips through JSON.
- **`satisfies` over `as`** when validating a literal matches a type without widening.
- **Arrow functions only** in script blocks (`const foo = (): void => { ... }`). No `function` keyword in non-class code.

```typescript
// ✅ Good
interface Post {
  title: string;
  publishedAt: string | null;     // null, not omitted
  tags: Array<string> | null;
}

const props = defineProps<{
  post: Post;
  showExcerpt: boolean;
}>();

const wordCount = computed((): number => props.post.title.split(/\s+/).length);

// ❌ Bad
interface Post {
  title: string;
  publishedAt?: string;            // ❌ T? silently omittable
  tags?: Array<string>;            // ❌ same
}

const props = defineProps<{
  post: any;                       // ❌ any
}>();

function getWordCount() {           // ❌ function keyword
  return (props.post as Post).title!.split(/\s+/).length;  // ❌ as, !
}
```

### Type-based defineProps + withDefaults

```typescript
const props = defineProps<{
  variant: 'primary' | 'ghost';
  href: string | null;             // null, not omitted
  disabled: boolean;
}>();

// With defaults — use null in the type, default value in withDefaults
const props = withDefaults(
  defineProps<{
    variant: 'primary' | 'ghost';
    disabled: boolean;
  }>(),
  { variant: 'primary', disabled: false },
);
```

### Type-based defineEmits

```typescript
const emit = defineEmits<{
  select: [tag: string];
  close: [];
}>();
```

### Template refs

```typescript
const navEl = ref<HTMLElement | null>(null);

onMounted(() => {
  if (!navEl.value) return;
  navEl.value.focus();
});
```

### defineModel for v-model

```typescript
const query = defineModel<string>({ required: true });
```

---

## Reactivity

### Use ref() over reactive()

`ref()` is the default. `reactive()` loses reactivity when destructured, can't be reassigned, and has proxy identity issues with `===`.

```typescript
// ✅ Good
const isOpen = ref(false);
const activeTag = ref<string | null>(null);

// ❌ Avoid
const state = reactive({ isOpen: false, activeTag: null as string | null });
```

### Never destructure reactive() objects

If you must use `reactive`, access properties directly (`state.foo`) — never destructure.

### Use shallowRef() for large objects and non-reactive library instances

```typescript
// ✅ Shiki highlighter instance — never deep-reactive
const highlighter = shallowRef<Highlighter | null>(null);
```

Also use `markRaw()` for objects that must never be reactive (third-party class instances, build-time data dumps).

### SSR-safe state: useState

For state that must survive SSR → hydration (e.g., a flag set during server render), use Nuxt's `useState`. It's like `ref` but SSR-safe and globally identified.

```typescript
// ✅ Survives SSR → hydration
const featureFlags = useState<Record<string, boolean>>('feature-flags', () => ({}));
```

For purely client-side state (mobile drawer open/closed, scroll position), regular `ref` is fine inside a composable.

### Avoid global mutable state

The MVP site has no cross-page state. If a need emerges (e.g., persisted theme toggle, search filter state across pages), use `useState` — do NOT install Pinia for a single store.

---

## Computed

### No side effects in computed getters

```typescript
// ❌ Side effect in computed
const filteredPosts = computed(() => {
  analytics.track('filter');
  return posts.value.filter((p) => p.tag === activeTag.value);
});

// ✅ Pure
const filteredPosts = computed(() => posts.value.filter((p) => p.tag === activeTag.value));
```

### Computed over methods for derived state

Computed caches until dependencies change. Methods recalculate every render.

### Never mutate source data in computed — return new arrays/objects

```typescript
// ❌ Mutates
const sortedPosts = computed(() => posts.value.sort((a, b) => a.publishedAt.localeCompare(b.publishedAt)));

// ✅ Returns new array
const sortedPosts = computed(() => [...posts.value].sort((a, b) => a.publishedAt.localeCompare(b.publishedAt)));
```

### Use computed for conditional class logic

```typescript
const pillClass = computed((): string => {
  const map: Record<string, string> = {
    shipped: 'bg-gold/15 text-gold-soft border-gold/30',
    inProgress: 'bg-river/15 text-river border-river/30',
    planned: 'bg-line-strong/30 text-ink-mute border-line-strong',
  };
  return map[props.status] ?? map.planned;
});
```

---

## Watchers

### Use getter function to watch specific properties

```typescript
// ❌ Watches entire object
watch(state, () => { /* fires on ANY property change */ });

// ✅ Watches specific property
watch(() => state.count, (newVal) => { /* fires only when count changes */ });
```

### watch vs watchEffect

- `watch`: explicit sources, old + new values, lazy by default
- `watchEffect`: auto-tracks dependencies, runs immediately, no old value

Use `watch` for old/new comparison. Use `watchEffect` for "run whenever any dependency changes."

### SSR caveat: don't access DOM in eager watchers

`watchEffect` runs immediately, which on SSR means it runs server-side where `document`/`window` don't exist. For DOM-touching watchers, either:

- Wrap the side effect in `if (import.meta.client) { ... }`
- Or move the logic into `onMounted` if it should only run once

```typescript
// ❌ Throws on SSR — document not defined
watchEffect(() => {
  document.title = pageTitle.value;
});

// ✅ Use Nuxt's useHead instead
useHead({ title: () => pageTitle.value });
```

For page titles + meta tags, ALWAYS use `useHead` / `useSeoMeta` (see SEO section) — never poke `document.title` directly.

---

## Components

### Y* prefix for our primitives

Every component we author is prefixed `Y` to distinguish from Nuxt built-ins (`NuxtLink`, `NuxtImg`, `ClientOnly`, `Suspense`) and any third-party additions:

```vue
<!-- ✅ -->
<YButton variant="primary">Get started</YButton>
<YCode :code="snippet" lang="yinz" filename="example.ynz" />

<!-- ❌ Don't shadow Nuxt names -->
<Link to="/docs">Docs</Link>
```

### PascalCase in templates (Nuxt requires it for auto-import)

```vue
<!-- ✅ -->
<YPill variant="shipped">v0.1</YPill>

<!-- ❌ Auto-import doesn't match kebab-case -->
<y-pill variant="shipped">v0.1</y-pill>
```

### Props down, events up

Parents pass data via props. Children communicate via `defineEmits()`. No `ref()` access to child internals for data flow.

### Use provide/inject for deep component trees (avoid prop drilling)

```typescript
import type { InjectionKey } from 'vue';
const NAV_KEY: InjectionKey<Ref<boolean>> = Symbol('nav-open');
provide(NAV_KEY, isOpen);

// Deep child
const isOpen = inject(NAV_KEY);
```

Use Symbol keys to avoid collisions. Mutations happen in the provider, not consumers.

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

## Composables

### Naming: use{Feature}, return object with named refs

```typescript
// ✅ app/composables/useScrollLock.ts
export const useScrollLock = () => {
  const isLocked = ref(false);

  const lock = (): void => {
    if (!import.meta.client) return;
    document.body.style.overflow = 'hidden';
    isLocked.value = true;
  };

  const unlock = (): void => {
    if (!import.meta.client) return;
    document.body.style.overflow = '';
    isLocked.value = false;
  };

  onUnmounted(unlock);

  return { isLocked, lock, unlock } as const;
};
```

### Composable vs utility function

- **Composable**: uses Vue reactivity (`ref`, `computed`, `watch`, lifecycle hooks). Lives in `app/composables/`. Auto-imported.
- **Utility**: pure function, no Vue API. Lives in `app/utils/`. Also auto-imported.

If it doesn't touch Vue, it's a utility — not a composable.

### Expose readonly state, keep mutations internal

```typescript
export const useMobileNav = () => {
  const _isOpen = ref(false);
  const isOpen = readonly(_isOpen);

  const toggle = (): void => {
    _isOpen.value = !_isOpen.value;
  };

  return { isOpen, toggle } as const;
};
```

### SSR-safe by default

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

When a future milestone needs build-time data (e.g., M3 roadmap registry reads `registry/roadmap.toml`, M4 docs reads `spec/*.md`):

### Use useAsyncData with native $fetch

```typescript
// Build-time: this runs during nuxi generate
const { data: roadmap } = await useAsyncData('roadmap', () =>
  $fetch<RoadmapEntry[]>('/api/roadmap.json'), // generated at build time by a Nitro route
);
```

For reading local files at build time, use a Nuxt module or `nitro:build:before` hook that emits a JSON file under `public/` or generates a virtual `~/data/...` import. Don't ship runtime API calls.

### NO Axios, NO fetch wrapper, NO API_BASE constants

The MVP site doesn't hit any APIs at runtime. If a runtime fetch becomes necessary (it shouldn't pre-v1.0), use Nuxt's native `$fetch` — it's a typed wrapper around fetch that integrates with SSR.

---

## Templates

### Never use v-html with untrusted content (XSS)

The only place `v-html` appears in our site is `<YCode>` rendering Shiki's pre-highlighted HTML — and Shiki's output is trusted because the `code` prop is hand-authored in `.vue` markup, NOT user input. M9 playground will need to revisit this if user-supplied code lands.

### Never combine v-if and v-for on the same element

```vue
<!-- ❌ v-if evaluated for every item -->
<li v-for="post in posts" v-if="post.published">

<!-- ✅ Filter first with computed -->
<li v-for="post in publishedPosts">
```

### Use v-show for frequent toggles, v-if for rare ones

`v-show` keeps the element in DOM. `v-if` destroys/recreates. Mobile drawer = `v-show`. Conditional sections that depend on a build flag = `v-if`.

### Always use :key with v-for

```vue
<YPostCard v-for="post in posts" :key="post.slug" :post="post" />
```

---

## Tailwind v4

### Tokens live in @theme — not in components

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

### No utility extraction for one-off styles

Use Tailwind classes directly. Don't `@apply` unless a combination repeats 3+ times.

### Dynamic classes — object syntax or computed

```vue
<!-- ✅ Object syntax -->
<div :class="{ 'text-ember': hasError, 'text-gold': !hasError }">

<!-- ✅ Computed for complex logic (see Computed section) -->
<div :class="statusClass">
```

### Never concatenate Tailwind class names dynamically

```typescript
// ❌ Tailwind can't detect these at build time — purged from CSS
const color = `text-${status}`;

// ✅ Complete class names in a lookup
const colorMap: Record<string, string> = {
  shipped: 'text-gold',
  inProgress: 'text-river',
  planned: 'text-ink-mute',
};
const color = colorMap[status];
```

### Responsive design: mobile-first

```vue
<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3">
```

Breakpoints from the prototype: `<600px` (`sm`), `<900px` (`md`), default `lg`. Tailwind v4 ships these out of the box.

### Dark mode: N/A

Site is dark-only. No `dark:` variants. No theme toggle. The coal background is the brand.

### Tailwind v4 `@theme` HMR caveat

Edits to `@theme` block sometimes don't HMR cleanly. If a token edit doesn't reflect on save, restart the dev server (`docker compose restart web`).

---

## SEO (Nuxt SEO suite)

Use Nuxt's SEO primitives — never poke `document.head` or `<meta>` manually.

### useHead for per-page title + meta

```typescript
// app/pages/docs/getting-started.vue
useHead({
  title: 'Getting Started',
  meta: [
    { name: 'description', content: 'Install Yinz and write your first program in 5 minutes.' },
  ],
});
```

The base title template lives in `app/app.vue` (set once, e.g. `'%s · Yinz'`) — per-page only sets the `%s` portion.

### useSeoMeta for OG + Twitter cards

```typescript
useSeoMeta({
  ogTitle: 'Getting Started',
  ogDescription: 'Install Yinz and write your first program in 5 minutes.',
  ogImage: '/og/docs-getting-started.png',
  twitterCard: 'summary_large_image',
});
```

For per-page generated OG images, the `nuxt-og-image` module renders them at build time — define the template per page, the module produces the PNG.

### Structured data via nuxt-schema-org

Schema.org JSON-LD via `useSchemaOrg` — only on pages where it's meaningful (homepage Organization, docs Article, blog BlogPosting).

---

## Yinz code snippets

Every Yinz snippet rendered on the site MUST be anchored to a real `.ynz` file in `examples/website/` (or equivalent — exact path locked by M5). CI builds + runs that file; output is captured via `insta` snapshots. The snippet on the site comes from the file, not from copy-pasted source in a `.vue` markup.

This is the **anti-drift commitment** from the roadmap. Hand-pasted Yinz snippets in `.vue` markup are banned outside the foundation milestone's `/_dev/components` gallery (which is dev-only and noindex'd).

```vue
<!-- ❌ Banned in production pages — drifts from compiler -->
<YCode lang="yinz" :code="`function hello() { ... }`" />

<!-- ✅ Sourced from a real .ynz file at build time -->
<YCode lang="yinz" :code="snippets.helloWorld" filename="hello.ynz" />
```

The `<YCode>` component itself is built in foundation milestone Phase 4 — it consumes pre-highlighted HTML from a build-time Nuxt module that runs Shiki against `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` (synced via `bun run sync-grammar`).

---

## Performance

### SSG-first — no runtime JS for static content

The whole site renders to static HTML at build time. Components used only for static content (most of the site) emit zero runtime JS beyond Vue's hydration.

Use `<ClientOnly>` to wrap genuinely client-side-only widgets (a future search input, a playground editor) so they don't run during SSR.

```vue
<ClientOnly>
  <YSearchBox />
</ClientOnly>
```

### Lazy-load heavy components with defineAsyncComponent

For widgets that aren't above-the-fold (search panel, video embeds, future playground):

```typescript
import { defineAsyncComponent } from 'vue';

const YPlayground = defineAsyncComponent({
  loader: () => import('~/components/playground/YPlayground.vue'),
  loadingComponent: YPlaygroundSkeleton,
  delay: 200,
});
```

Pair with `<Suspense>` for fallback UI.

### Use v-once for static content that never re-renders

```vue
<footer v-once>
  <p>Forged in Pittsburgh · Apache 2.0</p>
</footer>
```

### Use shallowRef for large datasets that replace entirely (not mutate)

```typescript
const docTree = shallowRef<DocNode[]>([]);
// Replace whole tree on update — no deep reactivity tax
docTree.value = await loadDocTree();
```

### Prefer computed over watchers for derived state

If a watcher updates a ref based on other refs, it should probably be a computed.

### Don't ship Pagefind index on the home page

Pagefind (search, M4) ships its index as separate `.pf_*` files lazy-loaded only when the search input is focused. Foundation phases don't touch this.

---

## Transitions

The other exception to "no `<style>` blocks" — Vue `<Transition>` components need CSS classes. Keep them tight:

```vue
<Transition name="fade">
  <div v-if="visible">Content</div>
</Transition>

<style scoped>
  .fade-enter-active,
  .fade-leave-active {
    transition: opacity 0.2s ease;
  }
  .fade-enter-from,
  .fade-leave-to {
    opacity: 0;
  }
</style>
```

---

## What's NOT in this stack (and why)

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

- `~/.claude/rules/coding-style.md` — `T \| null` over `T?`, no `any`, arrow functions, satisfies/as guidance
- `.claude/plans/roadmaps/webpage-docs.md` — locked stack decisions (Nuxt 4, Tailwind v4, Bun, hosting, fonts, SSG)
- `.claude/plans/active/webpage-foundation.md` — current milestone (component inventory, phase breakdown)
- `/tmp/yinz-design/yinz/project/shared.css` — design token source-of-truth (extracted into `@theme` block in foundation Phase 2)
