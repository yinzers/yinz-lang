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
