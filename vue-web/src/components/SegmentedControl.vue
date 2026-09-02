// Node SegmentedControl parity: pill container with sliding active segment.
<script setup lang="ts">
defineProps<{ options: { value: string; label: string }[]; modelValue: string; small?: boolean }>();
const emit = defineEmits<{ (e: "update:modelValue", v: string): void }>();
</script>

<template>
  <div
    style="display: inline-flex; background: var(--color-surface-2); border: 1px solid var(--nb-border); padding: 2px"
  >
    <button
      v-for="o in options"
      :key="o.value"
      :style="{
        padding: small ? '0.2rem 0.7rem' : '0.35rem 1rem',
        fontSize: small ? '0.78rem' : '0.9rem',
        fontWeight: 600,
        fontFamily: 'var(--font-body)',
        border: 'none',
        cursor: 'pointer',
        background: modelValue === o.value ? 'var(--color-surface)' : 'transparent',
        color: modelValue === o.value ? 'var(--color-text-main)' : 'var(--color-text-muted)',
        boxShadow: modelValue === o.value ? 'var(--nb-shadow-sm)' : 'none',
      }"
      @click="emit('update:modelValue', o.value)"
    >
      {{ o.label }}
    </button>
  </div>
</template>
