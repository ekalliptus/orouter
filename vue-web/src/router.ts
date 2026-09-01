// Router + auth guard. Unauthenticated users land on /login; the guard
// re-checks the session when the module cache is cold (page load).
import { createRouter, createWebHistory } from "vue-router";
import { fetchAuthStatus, useAuthed } from "@/lib/state";

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", redirect: "/dashboard" },
    { path: "/login", component: () => import("@/pages/LoginPage.vue") },
    {
      path: "/dashboard",
      component: () => import("@/components/DashboardLayout.vue"),
      children: [
        { path: "", component: () => import("@/pages/EndpointPage.vue") },
        { path: "providers", component: () => import("@/pages/ProvidersPage.vue") },
        { path: "models", component: () => import("@/pages/ModelsPage.vue") },
        { path: "combos", component: () => import("@/pages/CombosPage.vue") },
        { path: "usage", component: () => import("@/pages/UsagePage.vue") },
        { path: "quota", component: () => import("@/pages/QuotaPage.vue") },
        { path: "proxy-pools", component: () => import("@/pages/ProxyPoolsPage.vue") },
        { path: "console-log", component: () => import("@/pages/ConsoleLogPage.vue") },
        { path: "cli-tools", component: () => import("@/pages/CliToolsPage.vue") },
        { path: "profile", component: () => import("@/pages/SettingsPage.vue") },
      ],
    },
    { path: "/:pathMatch(.*)*", redirect: "/dashboard" },
  ],
});

router.beforeEach(async (to) => {
  if (to.path === "/login") return true;
  const authed = useAuthed();
  if (authed.value === null) await fetchAuthStatus();
  return authed.value ? true : { path: "/login" };
});
