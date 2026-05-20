// Body-scroll lock for mobile drawer; SSR-safe
export const useScrollLock = () => {
  const isLocked = ref(false)

  const lock = () => {
    if (import.meta.client) {
      document.body.style.overflow = 'hidden'
      isLocked.value = true
    }
  }

  const unlock = () => {
    if (import.meta.client) {
      document.body.style.overflow = ''
      isLocked.value = false
    }
  }

  onUnmounted(unlock)

  return { isLocked, lock, unlock }
}
