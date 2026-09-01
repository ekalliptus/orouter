// App root: route outlet + toast layer. The login page stands alone; every
// other route renders inside DashboardLayout (guard enforced in router.ts).
<script setup lang="ts">
import Toasts from "@/components/Toasts.vue";
</script>

<template>
  <router-view />
  <Toasts />
</template>
