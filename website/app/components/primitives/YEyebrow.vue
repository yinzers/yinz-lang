<!-- Small all-caps monospace label, gold by default -->
<script setup lang="ts">
const props = withDefaults(
  defineProps<{ mute?: boolean | null }>(),
  { mute: null },
)
</script>
<template>
  <p :class="['font-mono text-[11px] tracking-[0.18em] uppercase', props.mute ? 'text-ink-mute' : 'text-gold']">
    <slot />
  </p>
</template>
