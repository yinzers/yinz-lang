<!-- Callout box with info/warn/note variants; tag label optional -->
<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    variant?: 'info' | 'warn' | 'note' | null
    tag?: string | null
  }>(),
  { variant: 'note', tag: null },
)

const resolvedVariant = computed(() => props.variant ?? 'note')

const borderColor: Record<'info' | 'warn' | 'note', string> = {
  info: 'border-river',
  warn: 'border-ember',
  note: 'border-gold',
}
const tagColor: Record<'info' | 'warn' | 'note', string> = {
  info: 'text-river',
  warn: 'text-ember',
  note: 'text-gold',
}
</script>
<template>
  <div :class="['bg-bg-raised border-l-2 rounded-r px-4 py-3', borderColor[resolvedVariant]]">
    <p v-if="props.tag" :class="['font-mono text-[11px] tracking-widest uppercase mb-1', tagColor[resolvedVariant]]">
      {{ props.tag }}
    </p>
    <div class="text-ink-mute text-sm leading-relaxed">
      <slot />
    </div>
  </div>
</template>
