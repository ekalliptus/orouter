// Tinted pill badge — Node parity (bg color/12 + colored text + dot).
<script setup lang="ts">
const props = defineProps<{
  variant?: "success" | "danger" | "warning" | "info" | "neutral";
  size?: "sm" | "md";
  dot?: boolean;
}>();

const colorMap: Record<string, string> = {
  success: "var(--color-success)",
  danger: "var(--color-danger)",
  warning: "var(--color-warning)",
  info: "var(--color-info)",
  neutral: "var(--color-surface-3)",
};
const c = colorMap[props.variant ?? "neutral"];
const textColor =
  props.variant === "neutral" || props.variant === "warning" || !props.variant
    ? "var(--color-text-main)"
    : c;
</script>

<template>
  <span
    :style="{
      display: 'inline-flex',
      alignItems: 'center',
      gap: '0.3rem',
      padding: props.size === 'sm' ? '0.12rem 0.5rem' : '0.22rem 0.62rem',
      fontSize: props.size === 'sm' ? '0.68rem' : '0.75rem',
      fontWeight: 600,
      fontFamily: 'var(--font-body)',
      borderRadius: '9999px',
      backgroundColor: props.variant === 'neutral' || !props.variant
        ? 'var(--color-surface-2)'
        : `color-mix(in srgb, ${c} 12%, transparent)`,
      color: textColor,
    }"
  >
    <span
      v-if="props.dot"
      :style="{ width: '6px', height: '6px', borderRadius: '50%', backgroundColor: c, display: 'inline-block' }"
    />
    <slot />
  </span>
</template>
