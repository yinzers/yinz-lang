<!-- Max-width wrapper with responsive horizontal padding -->
<script setup lang="ts">
const props = withDefaults(
  defineProps<{ tag?: string | null }>(),
  { tag: null },
)
</script>
<template>
  <component
    :is="props.tag ?? 'div'"
    class="w-full max-w-(--container-max) mx-auto px-[clamp(20px,4vw,56px)] relative z-10"
  >
    <slot />
  </component>
</template>
