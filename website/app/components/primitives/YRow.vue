<!-- Flex row with configurable gap and alignment -->
<script setup lang="ts">
const props = withDefaults(
  defineProps<{ gap?: string | null; align?: string | null }>(),
  { gap: null, align: null },
)
</script>
<template>
  <div :class="['flex items-center', props.gap ?? 'gap-3', props.align ?? '']">
    <slot />
  </div>
</template>
