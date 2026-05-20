<!-- 4-column grid footer with copyright row -->
<script setup lang="ts">
interface FooterLink {
  label: string
  href: string
}

interface FooterColumn {
  heading: string
  links: FooterLink[]
}

withDefaults(
  defineProps<{ columns: FooterColumn[] | null; copyright: string | null }>(),
  { columns: null, copyright: null },
)
</script>
<template>
  <footer class="border-t border-line mt-24">
    <div class="w-full max-w-(--container-max) mx-auto px-[clamp(20px,4vw,56px)] py-16">
      <!-- 4-column grid, collapses at smaller viewports -->
      <div v-if="columns" class="grid grid-cols-1 gap-10 sm:grid-cols-2 md:grid-cols-[1.4fr_1fr_1fr_1fr] mb-12">
        <div v-for="col in columns" :key="col.heading">
          <p class="font-mono text-[11px] tracking-[0.18em] uppercase text-gold mb-4">{{ col.heading }}</p>
          <ul class="space-y-2">
            <li v-for="link in col.links" :key="link.href">
              <a :href="link.href" class="text-ink-mute hover:text-ink text-sm transition-colors no-underline">
                {{ link.label }}
              </a>
            </li>
          </ul>
        </div>
      </div>

      <!-- Bottom row -->
      <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 border-t border-line pt-6">
        <YLogo size="sm" />
        <p v-if="copyright" class="text-ink-dim text-sm">{{ copyright }}</p>
      </div>
    </div>
  </footer>
</template>
