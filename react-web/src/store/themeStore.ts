// Ported from src/store/themeStore.js (old Next.js app). Only change: dropped
// the "use client" directive (not needed in Vite) and typed the store.
//
// `theme` is persisted to localStorage key "theme" (matches the old app so a
// user switching between builds keeps their preference). initTheme() is called
// once on mount by <App/> to apply the .dark class.
import { create } from "zustand";
import { persist } from "zustand/middleware";

export type ThemeMode = "light" | "dark" | "system";

const STORAGE_KEY = "theme";
const DEFAULT_THEME: ThemeMode = "system";

interface ThemeState {
  theme: ThemeMode;
  setTheme: (theme: ThemeMode) => void;
  toggleTheme: () => void;
  initTheme: () => void;
}

function effectiveTheme(theme: ThemeMode): "light" | "dark" {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return theme;
}

function applyTheme(theme: ThemeMode) {
  if (typeof window === "undefined") return;
  const root = document.documentElement;
  if (effectiveTheme(theme) === "dark") {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: DEFAULT_THEME,
      setTheme: (theme) => {
        set({ theme });
        applyTheme(theme);
      },
      toggleTheme: () => {
        const next: ThemeMode = effectiveTheme(get().theme) === "dark" ? "light" : "dark";
        set({ theme: next });
        applyTheme(next);
      },
      initTheme: () => applyTheme(get().theme),
    }),
    { name: STORAGE_KEY }
  )
);
