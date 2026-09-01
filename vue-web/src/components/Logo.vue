// ORouter logo mark — orange square + yellow "O" ring (Node parity).
<script setup lang="ts">
defineProps<{ size?: number }>();
</script>

<template>
  <span
    class="inline-flex shrink-0 items-center justify-center"
    :style="{ width: (size ?? 36) + 'px', height: (size ?? 36) + 'px' }"
    role="img"
    aria-label="ORouter"
  >
    <svg
      :width="size ?? 36"
      :height="size ?? 36"
      viewBox="0 0 32 32"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <rect x="4" y="4" width="25" height="25" fill="var(--nb-border, #000)" />
      <rect x="1" y="1" width="25" height="25" fill="#FF6B35" stroke="#000000" stroke-width="2.5" />
      <circle cx="13.5" cy="13.5" r="8" fill="#FFD23F" stroke="#000000" stroke-width="2.5" />
      <circle cx="13.5" cy="13.5" r="3.5" fill="#FF6B35" stroke="#000000" stroke-width="2" />
    </svg>
  </span>
</template>
