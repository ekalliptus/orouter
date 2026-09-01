// Toast layer — Node dashboard style: fixed top-right, tinted border/bg.
<script setup lang="ts">
import { toasts } from "@/lib/state";

const kindStyle: Record<string, string> = {
  success: "#2A9D8F",
  error: "#E63946",
  info: "#4CC9F0",
};
</script>

<template>
  <div style="position: fixed; top: 1rem; right: 1rem; z-index: 80; width: min(92vw, 380px); display: flex; flex-direction: column; gap: 0.5rem">
    <div
      v-for="t in toasts"
      :key="t.id"
      class="kid-card"
      :style="{
        padding: '0.6rem 0.9rem',
        borderColor: kindStyle[t.kind],
        backgroundColor: `color-mix(in srgb, ${kindStyle[t.kind]} 10%, var(--color-surface))`,
        boxShadow: 'var(--nb-shadow-sm)',
      }"
    >
      <strong style="display: block; font-size: 0.95rem">{{ t.title }}</strong>
      <span v-if="t.message" style="font-family: var(--font-body); font-size: 0.9rem; color: var(--color-text-muted)">{{ t.message }}</span>
    </div>
  </div>
</template>
