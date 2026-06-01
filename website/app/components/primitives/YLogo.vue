<!-- Block-Y logo: SVG mark + Anton wordmark "yinz" + optional mono extension -->
<script setup lang="ts">
withDefaults(
  defineProps<{ ext?: string | null; size?: 'sm' | 'md' | 'lg' | null }>(),
  { ext: null, size: 'md' },
)
const sizeMap: Record<'sm' | 'md' | 'lg', { mark: string; text: string }> = {
  sm: { mark: 'w-5 h-5',  text: 'text-base'  },
  md: { mark: 'w-7 h-7',  text: 'text-[22px]' },
  lg: { mark: 'w-10 h-10', text: 'text-[28px]' },
}
</script>
<template>
  <span class="inline-flex items-baseline gap-2 font-display font-bold tracking-[-0.04em] text-ink">
    <img
      src="/yinz.svg"
      alt=""
      aria-hidden="true"
      :class="sizeMap[size ?? 'md'].mark"
      class="translate-y-0.5 flex-shrink-0"
    />
    <span :class="sizeMap[size ?? 'md'].text">yinz</span>
    <span v-if="ext" class="font-mono text-[12px] text-ink-dim font-normal tracking-normal">
      {{ ext }}
    </span>
  </span>
</template>
