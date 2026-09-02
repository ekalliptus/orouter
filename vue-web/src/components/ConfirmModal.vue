// Node ConfirmModal parity: title, message, cancel + colored confirm.
<script setup lang="ts">
import Modal from "./Modal.vue";

defineProps<{
  title: string;
  message?: string;
  confirmLabel?: string;
  danger?: boolean;
}>();
const emit = defineEmits<{ (e: "close"): void; (e: "confirm"): void }>();
</script>

<template>
  <Modal :width="'380px'" @close="emit('close')">
    <h3 style="font-size: 1.2rem; margin: 0 0 0.5rem">{{ title }}</h3>
    <p v-if="message" style="font-family: var(--font-body); color: var(--color-text-muted); margin: 0 0 0.5rem">{{ message }}</p>
    <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1rem">
      <button class="kid-btn" @click="emit('close')">Cancel</button>
      <button
        class="kid-btn"
        :style="danger ? { background: 'var(--color-danger)', color: '#fff' } : { background: 'var(--color-primary)', color: '#fff' }"
        @click="emit('confirm')"
      >
        {{ confirmLabel ?? "Confirm" }}
      </button>
    </div>
  </Modal>
</template>
