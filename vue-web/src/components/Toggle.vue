// Pill toggle — Node parity: on = brand orange, off = surface-3, clean thumb.
<script setup lang="ts">
defineProps<{ checked: boolean; disabled?: boolean }>();
const emit = defineEmits<{ (e: "change", value: boolean): void }>();
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="checked"
    :disabled="disabled"
    :style="{
      width: '44px',
      height: '24px',
      borderRadius: '9999px',
      backgroundColor: checked ? 'var(--color-primary)' : 'var(--color-surface-3)',
      border: 'none',
      position: 'relative',
      cursor: disabled ? 'not-allowed' : 'pointer',
      padding: 0,
      transition: 'background-color 0.2s',
    }"
    @click="!disabled && emit('change', !checked)"
  >
    <span
      :style="{
        width: '20px',
        height: '20px',
        borderRadius: '9999px',
        backgroundColor: '#ffffff',
        position: 'absolute',
        top: '2px',
        left: checked ? '22px' : '2px',
        transition: 'left 0.2s ease',
      }"
    />
  </button>
</template>
