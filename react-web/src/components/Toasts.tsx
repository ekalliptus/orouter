// Global toast container. Reads the notification store (port of the old app's
// toast system) and renders each as a kid-styled sticky note. Mounted once in
// DashboardLayout.
import { useNotificationStore } from "@/store/notificationStore";

const TYPE_ACCENT: Record<string, { bg: string; label: string }> = {
  success: { bg: "var(--color-success)", label: "Yay!" },
  error: { bg: "var(--color-danger)", label: "Oops!" },
  warning: { bg: "var(--color-warning)", label: "Heads up" },
  info: { bg: "var(--color-info)", label: "Psst" },
};

export default function Toasts() {
  const notifications = useNotificationStore((s) => s.notifications);
  const remove = useNotificationStore((s) => s.removeNotification);

  return (
    <div className="fixed bottom-6 right-6 z-50 flex w-80 flex-col gap-3">
      {notifications.map((n) => {
        const accent = TYPE_ACCENT[n.type] ?? TYPE_ACCENT.info;
        return (
          <div
            key={n.id}
            className="kid-card kid-wobble kid-tilt slide-in-right"
            style={{ ["--tilt" as string]: `${(n.id % 3) - 1}deg`, padding: 0 }}
            role="status"
          >
            <div style={{ background: accent.bg, color: "#1a1410", padding: "0.35rem 0.9rem", fontWeight: 700 }}>
              {accent.label}
            </div>
            <div style={{ padding: "0.7rem 0.9rem" }}>
              {n.title && <div style={{ fontWeight: 700, marginBottom: 2 }}>{n.title}</div>}
              <div style={{ fontFamily: "var(--font-body)" }}>{n.message}</div>
            </div>
            {n.dismissible && (
              <button
                onClick={() => remove(n.id)}
                aria-label="Dismiss"
                className="kid-btn"
                style={{ position: "absolute", top: 6, right: 6, padding: "0.15rem 0.5rem", fontSize: "0.9rem" }}
              >
                ✕
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
