<!-- CSS grid wrapper with configurable columns and gap -->
<script setup lang="ts">
const props = withDefaults(
  defineProps<{ cols?: string | null; gap?: string | null }>(),
  { cols: null, gap: null },
)
</script>
<template>
  <div :class="['grid', props.cols ?? 'grid-cols-1', props.gap ?? 'gap-4']">
    <slot />
  </div>
</template>
