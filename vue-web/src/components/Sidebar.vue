// Sidebar — faithful port of the 9Router sidebar: traffic lights, logo block
// with console-label status + version, nav items with Material Symbols, and
// the System group under a divider.
<script setup lang="ts">
import { onMounted, ref } from "vue";
import Logo from "@/components/Logo.vue";

const props = defineProps<{ onClose?: () => void }>();

const navItems = [
  { href: "/dashboard", label: "Endpoint & Key", icon: "api" },
  { href: "/dashboard/providers", label: "Providers", icon: "dns" },
  { href: "/dashboard/models", label: "Models", icon: "category" },
  { href: "/dashboard/combos", label: "Combos", icon: "layers" },
  { href: "/dashboard/usage", label: "Usage", icon: "bar_chart" },
  { href: "/dashboard/quota", label: "Quota Tracker", icon: "data_usage" },
  { href: "/dashboard/proxy-pools", label: "Proxy Pools", icon: "hub" },
  { href: "/dashboard/console-log", label: "Console Log", icon: "monitoring" },
  { href: "/dashboard/cli-tools", label: "CLI Tools", icon: "terminal" },
];

const systemItems = [
  { href: "/dashboard/profile", label: "Settings", icon: "settings" },
];

const version = ref("");
onMounted(async () => {
  try {
    const res = await fetch("/api/version", { credentials: "include" });
    if (res.ok) {
      const v = (await res.json()) as { version?: string };
      if (v.version) version.value = `v${v.version}`;
    }
  } catch { /* best effort */ }
});

function isActive(href: string, pathname: string) {
  if (href === "/dashboard") return pathname === "/dashboard" || pathname === "/dashboard/";
  return pathname.startsWith(href);
}
</script>

<template>
  <aside class="flex min-h-full w-72 flex-col border-r border-border-subtle bg-sidebar">
    <!-- Traffic lights -->
    <div class="flex items-center gap-2 px-6 pt-5 pb-2">
      <span class="inline-block h-3 w-3 rounded-full" style="background: #FF5F56" />
      <span class="inline-block h-3 w-3 rounded-full" style="background: #FFBD2E" />
      <span class="inline-block h-3 w-3 rounded-full" style="background: #27C93F" />
    </div>

    <!-- Logo -->
    <div class="px-6 py-4 flex flex-col gap-2">
      <router-link to="/dashboard" class="flex items-center gap-3" @click="props.onClose?.()">
        <Logo :size="36" />
        <div class="flex flex-col">
          <h1 class="text-lg font-extrabold tracking-tight text-text-main uppercase" style="margin: 0">ORouter</h1>
          <span class="console-label" style="color: var(--color-success)">Control Plane · Online</span>
          <span v-if="version" style="font-size: 10px; color: var(--color-text-muted)">{{ version }}</span>
        </div>
      </router-link>
    </div>

    <!-- Navigation -->
    <nav class="flex-1 px-4 py-2 space-y-0.5 overflow-y-auto" style="overflow-y: auto">
      <router-link
        v-for="item in navItems"
        :key="item.href"
        :to="item.href"
        class="nav-item"
        :class="{ active: isActive(item.href, $route.path) }"
        @click="props.onClose?.()"
      >
        <span class="material-symbols-outlined" style="font-size: 18px">{{ item.icon }}</span>
        <span style="font-size: 13px; font-weight: 500">{{ item.label }}</span>
      </router-link>

      <div class="pt-3 mt-2 space-y-0.5" style="border-top: 1px solid var(--color-border-subtle)">
        <p class="console-label" style="margin: 0 0 0.5rem; padding: 0 0.75rem">System</p>
        <router-link
          v-for="item in [...systemItems, { href: '/dashboard/token-saver', label: 'Token Saver', icon: 'savings' }, { href: '/dashboard/translator', label: 'Translator', icon: 'translate' }]"
          :key="item.href"
          :to="item.href"
          class="nav-item"
          :class="{ active: isActive(item.href, $route.path) }"
          @click="props.onClose?.()"
        >
          <span class="material-symbols-outlined" style="font-size: 18px">{{ item.icon }}</span>
          <span style="font-size: 13px; font-weight: 500">{{ item.label }}</span>
        </router-link>
      </div>
    </nav>
  </aside>
</template>

<style scoped>
.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.375rem 0.75rem;
  border: 1px solid transparent;
  border-radius: 8px;
  color: var(--color-text-muted);
  text-decoration: none;
  transition: background-color 0.15s, color 0.15s;
}
.nav-item:hover {
  background: color-mix(in srgb, var(--color-surface-2) 70%, transparent);
  color: var(--color-text-main);
}
.nav-item.active {
  background: color-mix(in srgb, var(--color-brand-500) 10%, transparent);
  border-color: color-mix(in srgb, var(--color-brand-500) 20%, transparent);
  color: var(--color-brand-600);
}
.nav-item.active::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0.5rem;
  bottom: 0.5rem;
  width: 2px;
  border-radius: 9999px;
  background: var(--color-brand-500);
}
.nav-item.active .material-symbols-outlined {
  font-variation-settings: "FILL" 1, "wght" 400, "GRAD" 0, "opsz" 24;
}
</style>
