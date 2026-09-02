// Header — Node layout: page icon (primary color) + title + description on
// the left, round ghost theme toggle on the right. Titles live HERE, not in
// the page bodies (Node parity).
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { searchQuery } from "@/lib/state";

const route = useRoute();
const dark = ref(document.documentElement.classList.contains("dark"));

function toggleTheme() {
  dark.value = !dark.value;
  document.documentElement.classList.toggle("dark", dark.value);
  try { localStorage.setItem("orouter-theme", dark.value ? "dark" : "light"); } catch { /* ok */ }
}

// Restore saved theme once.
try {
  const saved = localStorage.getItem("orouter-theme");
  if (saved) {
    dark.value = saved === "dark";
    document.documentElement.classList.toggle("dark", dark.value);
  }
} catch { /* ok */ }

const PAGE_INFO: { match: (p: string) => boolean; title: string; description: string; icon: string; searchable?: boolean; searchPlaceholder?: string }[] = [
  { match: (p) => p.includes("/providers"), title: "Providers", description: "Manage your AI provider connections", icon: "dns", searchable: true, searchPlaceholder: "Search providers..." },
  { match: (p) => p.includes("/models"), title: "Models", description: "Every model the router knows about", icon: "category", searchable: true, searchPlaceholder: "Search models..." },
  { match: (p) => p.includes("/combos"), title: "Combos", description: "Model combos with fallback", icon: "layers" },
  { match: (p) => p.includes("/usage"), title: "Usage & Analytics", description: "Monitor your API usage, token consumption, and request logs", icon: "bar_chart" },
  { match: (p) => p.includes("/quota"), title: "Quota Tracker", description: "Live quota per connected account", icon: "data_usage" },
  { match: (p) => p.includes("/proxy-pools"), title: "Proxy Pools", description: "Route provider traffic through proxies", icon: "hub" },
  { match: (p) => p.includes("/console-log"), title: "Console Log", description: "Live server log tail", icon: "monitoring" },
  { match: (p) => p.includes("/token-saver"), title: "Token Saver", description: "Cut prompt tokens in-flight", icon: "savings" },
  { match: (p) => p.includes("/translator"), title: "Translator", description: "Test payloads through the router", icon: "translate" },
  { match: (p) => p.includes("/cli-tools"), title: "CLI Tools", description: "Configure CLI tools", icon: "terminal" },
  { match: (p) => p.includes("/profile"), title: "Settings", description: "Manage your preferences", icon: "settings" },
];

const info = computed(() => {
  const p = route.path;
  return PAGE_INFO.find((x) => x.match(p)) ?? { title: "Endpoint & Key", description: "API endpoint configuration and keys", icon: "api", searchable: false, searchPlaceholder: "" };
});

watch(() => route.path, () => {
  searchQuery.value = "";
});
</script>

<template>
  <header class="sticky top-0 z-30 flex items-center justify-between px-4 lg:px-8 pt-3 pb-2" style="border-bottom: 1px solid var(--color-border-subtle); background: color-mix(in srgb, var(--color-bg) 80%, transparent); backdrop-filter: blur(4px)">
    <div class="flex items-center gap-3">
      <span class="material-symbols-outlined" style="color: var(--color-primary); font-size: 24px">{{ info.icon }}</span>
      <div class="flex flex-col">
        <h1 style="margin: 0; font-size: 1rem; font-weight: 600; letter-spacing: -0.01em" class="lg:text-2xl">{{ info.title }}</h1>
        <p class="hidden lg:block" style="margin: 0; font-family: var(--font-body); font-size: 0.875rem; color: var(--color-text-muted)">{{ info.description }}</p>
      </div>
    </div>

      <div class="flex items-center gap-2">
        <div v-if="info.searchable" style="position: relative; display: flex; align-items: center">
          <span class="material-symbols-outlined" style="font-size: 18px; position: absolute; left: 8px; color: var(--color-text-muted)">search</span>
          <input
            v-model="searchQuery"
            :placeholder="info.searchPlaceholder"
            style="height: 32px; width: 200px; padding: 0 28px 0 32px; font-family: var(--font-body); font-size: 0.85rem; background: color-mix(in srgb, var(--color-surface) 60%, transparent); border: 1px solid var(--nb-border); outline: none; color: var(--color-text-main)"
          />
          <button
            v-if="searchQuery"
            style="position: absolute; right: 4px; border: none; background: transparent; cursor: pointer; color: var(--color-text-muted); display: flex"
            @click="searchQuery = ''"
          >
            <span class="material-symbols-outlined" style="font-size: 16px">close</span>
          </button>
        </div>
        <button
          aria-label="Toggle theme"
          class="theme-toggle"
          @click="toggleTheme"
        >
          <span class="material-symbols-outlined" style="font-size: 22px">{{ dark ? "light_mode" : "dark_mode" }}</span>
        </button>
      </div>
  </header>
</template>

<style scoped>
.theme-toggle {
  display: flex;
  height: 2.5rem;
  width: 2.5rem;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 9999px;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
}
.theme-toggle:hover {
  background: var(--color-surface-2);
  color: var(--color-text-main);
}
</style>
