// Dashboard shell: 288px sidebar + header + main with the Node blueprint
// grid overlay behind the content.
<script setup lang="ts">
import Sidebar from "./Sidebar.vue";
import AppHeader from "./AppHeader.vue";
</script>

<template>
  <div class="flex h-screen w-full overflow-hidden" style="background: var(--color-bg); color: var(--color-text-main)">
    <Sidebar />
    <div class="flex flex-1 flex-col min-w-0" style="position: relative; isolation: isolate">
      <AppHeader />
      <main class="flex-1 overflow-y-auto p-6" style="padding: 1.5rem">
        <div class="landing-grid" aria-hidden="true"></div>
        <div style="max-width: 80rem; margin: 0 auto">
          <router-view />
        </div>
      </main>
    </div>
  </div>
</template>
