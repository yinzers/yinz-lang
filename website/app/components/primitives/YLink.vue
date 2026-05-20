<!-- Text link; defaults to gold with hover to gold-soft -->
<script setup lang="ts">
withDefaults(
  defineProps<{ href: string; external: boolean | null }>(),
  { external: null },
)
</script>
<template>
  <a
    :href="href"
    :target="external ? '_blank' : undefined"
    :rel="external ? 'noopener noreferrer' : undefined"
    class="text-gold hover:text-gold-soft transition-colors"
  >
    <slot />
  </a>
</template>
