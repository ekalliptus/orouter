// Ported from src/store/notificationStore.js. Global toast system. Auto-dismiss
// timers live here; durations default 5s, errors 8s.
import { create } from "zustand";

export type NotificationType = "success" | "error" | "warning" | "info";

export interface Notification {
  id: number;
  type: NotificationType;
  message: string;
  title: string | null;
  duration: number;
  dismissible: boolean;
  createdAt: number;
}

interface NotificationState {
  notifications: Notification[];
  addNotification: (n: Partial<Notification> & { message: string }) => number;
  removeNotification: (id: number) => void;
  clearAll: () => void;
  success: (message: string, title?: string) => number;
  error: (message: string, title?: string) => number;
  warning: (message: string, title?: string) => number;
  info: (message: string, title?: string) => number;
}

let idCounter = 0;

export const useNotificationStore = create<NotificationState>((set, get) => ({
  notifications: [],

  addNotification: (notification) => {
    const id = ++idCounter;
    const entry: Notification = {
      id,
      type: (notification.type as NotificationType) ?? "info",
      message: notification.message,
      title: notification.title ?? null,
      duration: notification.duration ?? 5000,
      dismissible: notification.dismissible ?? true,
      createdAt: Date.now(),
    };
    set((s) => ({ notifications: [...s.notifications, entry] }));
    if (entry.duration > 0) {
      setTimeout(() => get().removeNotification(id), entry.duration);
    }
    return id;
  },

  removeNotification: (id) =>
    set((s) => ({ notifications: s.notifications.filter((n) => n.id !== id) })),

  clearAll: () => set({ notifications: [] }),

  success: (message, title) => get().addNotification({ type: "success", message, title }),
  error: (message, title) => get().addNotification({ type: "error", message, title, duration: 8000 }),
  warning: (message, title) => get().addNotification({ type: "warning", message, title }),
  info: (message, title) => get().addNotification({ type: "info", message, title }),
}));
