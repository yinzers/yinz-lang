import tailwindcss from '@tailwindcss/vite'

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-07-15',
  devtools: { enabled: true },

  ssr: true,

  // Inline SSR content into HTML so code highlighting works without JS
  experimental: {
    // 'client' = full-static: SSR HTML in initial response + payload in separate files
    payloadExtraction: 'client',
  },

  modules: [
    '@nuxt/fonts',
    'nuxt-schema-org',
    'nuxt-og-image',
    '@nuxtjs/sitemap',
    '@nuxt/image',
  ],

  css: ['~/assets/css/tailwind.css'],

  fonts: {
    families: [
      // Anton only ships at weight 400 — no additional weights available from Google Fonts
      { name: 'Anton', weights: [400], display: 'swap' },
      { name: 'Inter', weights: [400, 500, 600, 700], display: 'swap' },
      { name: 'JetBrains Mono', weights: [400, 500, 600], display: 'swap' },
    ],
    defaults: {
      weights: [400],
      styles: ['normal'],
      subsets: ['latin'],
    },
  },

  // SEO — site identity (used by sitemap, schema-org, og-image)
  site: {
    url: 'https://yinzlang.com',
    name: 'Yinz',
    description: 'Yinz — Rust-level performance, TypeScript-level readability. v0.2 in progress.',
  },

  // Sitemap: exclude internal gallery routes
  sitemap: {
    exclude: ['/_dev/**'],
  },

  // @nuxt/image: ipx provider for build-time optimization
  image: {
    provider: 'ipx',
    formats: ['avif', 'webp'],
  },

    components: [
    // pathPrefix: false so components/primitives/YFoo.vue registers as <YFoo />
    // (not <PrimitivesYFoo />)
    { path: '~/components', pathPrefix: false },
  ],

  vite: {
    plugins: [tailwindcss()],
    server: {
      watch: {
        usePolling: true,
        interval: 300,
      },
    },
  },
})
