<!-- Syntax-highlighted code block with filename header and optional line gutter -->
<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    code: string
    lang: string | null   // future: when server plugin supports multiple langs; M1 = yinz only
    filename: string | null
    showLineNumbers: boolean | null
  }>(),
  { lang: 'yinz', filename: null, showLineNumbers: true },
)

// Server plugin provides $shikiHighlight; null on client (colors come from SSR HTML)
const { $shikiHighlight } = useNuxtApp()
let highlightedHtml: string | null = null
try {
  // $shikiHighlight currently only highlights yinz; lang prop is reserved for future multi-lang support
  highlightedHtml = $shikiHighlight ? $shikiHighlight(props.code) : null
} catch (err) {
  console.error('[YCode] Shiki highlight failed:', err)
}

const lines = computed(() => props.code.split('\n'))
const ext = computed(() => {
  if (!props.filename) return null
  const dot = props.filename.lastIndexOf('.')
  return dot > 0 ? props.filename.slice(dot) : null
})
const basename = computed(() => {
  if (!props.filename) return null
  const dot = props.filename.lastIndexOf('.')
  return dot > 0 ? props.filename.slice(0, dot) : props.filename
})
</script>

<template>
  <div class="bg-bg-card border border-line rounded overflow-hidden font-mono text-[13.5px] leading-[1.65]">
    <!-- Filename header -->
    <div v-if="filename" class="flex items-center px-[14px] py-2 border-b border-line bg-bg-raised text-[12px] text-ink-mute">
      <span class="font-mono text-ink">
        {{ basename }}<span v-if="ext" class="text-gold">{{ ext }}</span>
      </span>
    </div>

    <!-- Code body -->
    <div class="overflow-x-auto px-5 py-[18px]">
      <!-- With line numbers: gutter always shown, content column renders Shiki or plain -->
      <div v-if="showLineNumbers" class="grid grid-cols-[auto_1fr]">
        <div class="text-ink-dim text-[12.5px] text-right pr-4 border-r border-line mr-4 select-none">
          <div v-for="(_, i) in lines" :key="i">{{ i + 1 }}</div>
        </div>
        <div v-if="highlightedHtml" class="shiki-output min-w-0" v-html="highlightedHtml" />
        <pre v-else class="m-0 whitespace-pre text-ink">{{ code }}</pre>
      </div>

      <!-- Without line numbers -->
      <template v-else>
        <div v-if="highlightedHtml" class="shiki-output" v-html="highlightedHtml" />
        <pre v-else class="m-0 whitespace-pre text-ink">{{ code }}</pre>
      </template>
    </div>
  </div>
</template>

<style scoped>
/* Shiki wraps output in <pre>; reset it to inherit container styles */
.shiki-output :deep(pre) {
  margin: 0;
  background: transparent !important;
  padding: 0;
  font-family: inherit;
  font-size: inherit;
  line-height: inherit;
}
</style>
