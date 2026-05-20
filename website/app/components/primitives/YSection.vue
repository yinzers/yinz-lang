<!-- Vertical padding section; optional top border via border prop -->
<script setup lang="ts">
const props = withDefaults(
  defineProps<{ border: boolean | null; tag: string | null }>(),
  { border: null, tag: null },
)
</script>
<template>
  <component
    :is="props.tag ?? 'section'"
    :class="['py-16 md:py-24', props.border && 'border-t border-line']"
  >
    <slot />
  </component>
</template>
