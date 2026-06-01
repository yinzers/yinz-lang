<!-- Sticky top nav with backdrop blur, links, CTA slot, and mobile drawer -->
<script setup lang="ts">
interface NavLink {
  label: string
  href: string
  active: boolean | null
}

const props = withDefaults(
  defineProps<{ links?: NavLink[] | null }>(),
  { links: null },
)

const { isLocked, lock, unlock } = useScrollLock()
const drawerOpen = ref(false)

const openDrawer = () => {
  drawerOpen.value = true
  lock()
}

const closeDrawer = () => {
  drawerOpen.value = false
  unlock()
}
</script>

<template>
  <header class="sticky top-0 z-50 border-b border-line bg-bg/80 backdrop-blur-[14px] backdrop-saturate-[140%]">
    <div class="w-full max-w-(--container-max) mx-auto px-[clamp(20px,4vw,56px)]">
      <div class="flex items-center justify-between h-16">
        <!-- Logo -->
        <NuxtLink href="/" class="no-underline">
          <YLogo />
        </NuxtLink>

        <!-- Desktop links -->
        <nav v-if="props.links" class="hidden sm:flex gap-7 text-sm">
          <NuxtLink
            v-for="link in props.links"
            :key="link.href"
            :href="link.href"
            :class="['text-ink-mute hover:text-ink font-medium transition-colors no-underline', link.active && 'text-ink']"
          >
            {{ link.label }}
          </NuxtLink>
        </nav>

        <!-- CTA slot + hamburger -->
        <div class="flex items-center gap-3">
          <slot />
          <!-- Mobile hamburger -->
          <button
            class="sm:hidden flex flex-col justify-center gap-[5px] w-8 h-8 bg-transparent border-0 cursor-pointer p-1"
            :aria-label="drawerOpen ? 'Close menu' : 'Open menu'"
            :aria-expanded="drawerOpen"
            @click="drawerOpen ? closeDrawer() : openDrawer()"
          >
            <span class="block w-full h-[1.5px] bg-ink transition-transform" :class="drawerOpen && 'translate-y-[6.5px] rotate-45'" />
            <span class="block w-full h-[1.5px] bg-ink transition-opacity" :class="drawerOpen && 'opacity-0'" />
            <span class="block w-full h-[1.5px] bg-ink transition-transform" :class="drawerOpen && '-translate-y-[6.5px] -rotate-45'" />
          </button>
        </div>
      </div>
    </div>

    <!-- Mobile drawer -->
    <div
      v-if="drawerOpen"
      class="sm:hidden border-t border-line bg-bg-raised px-[clamp(20px,4vw,56px)] py-6 flex flex-col gap-4"
    >
      <NuxtLink
        v-for="link in props.links ?? []"
        :key="link.href"
        :href="link.href"
        :class="['text-ink-mute hover:text-ink text-lg font-medium no-underline transition-colors', link.active && 'text-ink']"
        @click="closeDrawer"
      >
        {{ link.label }}
      </NuxtLink>
      <!-- CTA in drawer -->
      <div class="pt-2">
        <slot />
      </div>
    </div>
  </header>
</template>
