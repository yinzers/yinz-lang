<!-- Display heading using Anton font; level 1-4 maps to h1-h4 -->
<script setup lang="ts">
const props = defineProps<{ level: 1 | 2 | 3 | 4 }>()
const tag = `h${props.level}` as const
const sizeClass: Record<1 | 2 | 3 | 4, string> = {
  1: 'font-display text-[clamp(48px,7vw,96px)] leading-[0.98] tracking-[-0.035em]',
  2: 'font-display text-[clamp(28px,3.4vw,44px)] leading-[1.05] tracking-[-0.025em]',
  3: 'font-display text-[clamp(20px,1.8vw,24px)] leading-[1.2] tracking-[-0.02em]',
  4: 'font-display text-[16px] leading-[1.3] tracking-[-0.02em]',
}
</script>
<template>
  <component :is="tag" :class="['text-ink font-[400]', sizeClass[props.level]]">
    <slot />
  </component>
</template>
