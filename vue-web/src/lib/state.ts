// Auth state + toast store — tiny reactive singletons (Vue equivalents of the
// react-web module stores).
import { ref } from "vue";

const cached = ref<boolean | null>(null);

// Header contextual search (Node parity): pages like Providers/Models read
// this instead of a page-local input.
export const searchQuery = ref("");

export function useAuthed() {
  return cached;
}

export async function fetchAuthStatus(force = false): Promise<boolean> {
  if (cached.value !== null && !force) return cached.value;
  try {
    const res = await fetch("/api/auth/status", { credentials: "include" });
    const data = (await res.json()) as { authenticated?: boolean };
    cached.value = !!data.authenticated;
  } catch {
    cached.value = false;
  }
  return cached.value;
}

export interface Toast {
  id: number;
  kind: "success" | "error" | "info";
  title: string;
  message?: string;
}

let nextId = 1;
export const toasts = ref<Toast[]>([]);

export function notify(kind: Toast["kind"], title: string, message?: string) {
  const t = { id: nextId++, kind, title, message };
  toasts.value.push(t);
  setTimeout(() => {
    toasts.value = toasts.value.filter((x) => x.id !== t.id);
  }, 4200);
}

export const toast = {
  success: (title: string, message?: string) => notify("success", title, message),
  error: (title: string, message?: string) => notify("error", title, message),
  info: (title: string, message?: string) => notify("info", title, message),
};
