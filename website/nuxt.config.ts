import tailwindcss from '@tailwindcss/vite'

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-07-15',
  devtools: { enabled: true },

  ssr: true,

  modules: ['@nuxt/fonts'],

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
