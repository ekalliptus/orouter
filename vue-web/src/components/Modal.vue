// Generic modal — Node Modal parity: dark overlay + kid-card panel.
<script setup lang="ts">
defineProps<{ width?: string }>();
const emit = defineEmits<{ (e: "close"): void }>();
</script>

<template>
  <div
    style="position: fixed; inset: 0; background: rgba(0,0,0,0.5); backdrop-filter: blur(2px); z-index: 100; display: flex; align-items: center; justify-content: center; padding: 1rem"
    @click.self="emit('close')"
  >
    <div class="kid-card kid-wobble fade-in" :style="{ width: `min(${width ?? '440px'}, 100%)`, background: 'var(--color-surface)' }">
      <slot />
    </div>
  </div>
</template>
