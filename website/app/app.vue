<script setup lang="ts">
const route = useRoute()
const siteUrl = 'https://yinzlang.com'

// Global head defaults — individual pages override title/description as needed
useHead({
  titleTemplate: (title) => title ? `${title} · Yinz` : 'Yinz',
  meta: [
    { name: 'description', content: 'Yinz — Rust-level performance, TypeScript-level readability. v0.2 in progress.' },
    { property: 'og:type', content: 'website' },
    { name: 'twitter:card', content: 'summary_large_image' },
  ],
  link: computed(() => [
    { rel: 'canonical', href: `${siteUrl}${route.path}` },
  ]),
})

// Schema.org: site identity injected via useHead for reliable SSG
useHead({
  script: [
    {
      type: 'application/ld+json',
      innerHTML: JSON.stringify({
        '@context': 'https://schema.org',
        '@type': 'WebSite',
        name: 'Yinz',
        url: siteUrl,
        description: 'Yinz — Rust-level performance, TypeScript-level readability.',
        inLanguage: 'en',
      }),
    },
    {
      type: 'application/ld+json',
      innerHTML: JSON.stringify({
        '@context': 'https://schema.org',
        '@type': 'Organization',
        name: 'Yinz Contributors',
        url: siteUrl,
        logo: `${siteUrl}/yinz.svg`,
      }),
    },
  ],
})
</script>

<template>
  <div class="relative min-h-screen">
    <NuxtRouteAnnouncer />
    <YNav
      :links="[
        { label: 'Docs', href: '#', active: null },
        { label: 'Examples', href: '#', active: null },
        { label: 'Blog', href: '#', active: null },
      ]"
    >
      <YButton variant="ghost" class="text-sm">GitHub</YButton>
    </YNav>
    <NuxtPage />
  </div>
</template>
