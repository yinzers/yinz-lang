<!-- Status badge; pass any color string to dot, or null for no dot -->
<script setup lang="ts">
const props = withDefaults(
  defineProps<{ dot: string | null }>(),
  { dot: null },
)
</script>
<template>
  <span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-bg-raised border border-line text-ink-mute text-[12px] font-mono">
    <span
      v-if="props.dot !== null"
      class="inline-block w-1.5 h-1.5 rounded-full flex-shrink-0"
      :style="{ background: props.dot || 'var(--color-gold)' }"
    />
    <slot />
  </span>
</template>
