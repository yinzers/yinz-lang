<!-- Button with primary (gold) and ghost (outlined) variants -->
<script setup lang="ts">
withDefaults(
  defineProps<{ variant: 'primary' | 'ghost' | null }>(),
  { variant: 'primary' },
)
defineEmits<{ click: [] }>()
</script>
<template>
  <button
    :class="[
      'inline-flex items-center gap-2 h-10 px-[18px] rounded-full font-sans text-sm font-semibold border cursor-pointer',
      'transition-transform duration-[60ms] hover:-translate-y-px',
      variant === 'ghost'
        ? 'bg-transparent text-ink border-line-strong hover:bg-bg-raised hover:border-[#4a4034]'
        : 'bg-gold text-[#1a1208] border-gold hover:bg-gold-soft hover:border-gold-soft',
    ]"
    @click="$emit('click')"
  >
    <slot />
  </button>
</template>
