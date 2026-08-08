// Usage page: stats overview (totals + by-provider) + recent log lines.
// Backed by GET /api/usage/stats?period= and GET /api/usage/logs.
// The Rust proxy writes a usageHistory row after each completed chat request.
import { useEffect, useState, useCallback } from "react";
import { useNotificationStore } from "@/store/notificationStore";

interface UsageStats {
  totalRequests: number;
  totalPromptTokens: number;
  totalCompletionTokens: number;
  totalCachedTokens: number;
  totalCost: number;
  byProvider: Record<string, { requests: number; promptTokens: number; completionTokens: number; cachedTokens?: number; cost: number }>;
  recentRequests?: { timestamp: string; model: string; provider: string; promptTokens: number; completionTokens: number; status: string }[];
}

const PERIODS: { value: string; label: string }[] = [
  { value: "24h", label: "24h" },
  { value: "7d", label: "7d" },
  { value: "30d", label: "30d" },
  { value: "all", label: "All" },
];

function fmt(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1) + "k";
  return String(n);
}

export default function UsagePage() {
  const [period, setPeriod] = useState("7d");
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const notify = useNotificationStore();

  const load = useCallback(async (p: string) => {
    setLoading(true);
    try {
      const [statsRes, logsRes] = await Promise.all([
        fetch(`/api/usage/stats?period=${p}`, { credentials: "include" }),
        fetch("/api/usage/logs", { credentials: "include" }),
      ]);
      if (statsRes.ok) setStats(await statsRes.json());
      if (logsRes.ok) setLogs((await logsRes.json()) as string[]);
    } catch {
      notify.error("Failed to load usage data");
    } finally {
      setLoading(false);
    }
  }, [notify]);

  useEffect(() => { load(period); }, [period, load]);

  const providers = stats ? Object.entries(stats.byProvider).sort((a, b) => b[1].requests - a[1].requests) : [];

  return (
    <div className="fade-in">
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: "0.5rem" }}>
        <h1 style={{ fontSize: "2rem", margin: 0 }}>📊 Usage</h1>
        <div style={{ display: "flex", gap: "0.4rem" }}>
          {PERIODS.map((p) => (
            <button
              key={p.value}
              className="kid-btn"
              style={{
                padding: "0.4rem 0.8rem", fontSize: "0.95rem",
                background: period === p.value ? "var(--color-accent)" : "var(--color-surface)",
              }}
              onClick={() => setPeriod(p.value)}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading usage…</p>}

      {/* Totals */}
      {stats && !loading && (
        <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", marginTop: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))" }}>
          {[
            { emoji: "📨", label: "Requests", value: fmt(stats.totalRequests) },
            { emoji: "📥", label: "Prompt tokens", value: fmt(stats.totalPromptTokens) },
            { emoji: "📤", label: "Completion tokens", value: fmt(stats.totalCompletionTokens) },
            { emoji: "🗃️", label: "Cached tokens", value: fmt(stats.totalCachedTokens) },
          ].map((c) => (
            <div key={c.label} className="kid-card kid-wobble">
              <div style={{ fontSize: "1.6rem" }}>{c.emoji}</div>
              <div style={{ fontSize: "1.8rem", fontWeight: 700, lineHeight: 1.1 }}>{c.value}</div>
              <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)" }}>{c.label}</div>
            </div>
          ))}
        </div>
      )}

      {/* By provider */}
      {providers.length > 0 && (
        <div style={{ marginTop: "1.5rem" }}>
          <h2 style={{ fontSize: "1.3rem" }}>By provider</h2>
          <div style={{ display: "grid", gap: "0.6rem", marginTop: "0.5rem" }}>
            {providers.map(([name, s]) => (
              <div key={name} className="kid-card kid-wobble" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "0.7rem 1rem" }}>
                <strong>{name}</strong>
                <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", display: "flex", gap: "1.2rem" }}>
                  <span>📨 {fmt(s.requests)}</span>
                  <span>🪙 {fmt(s.promptTokens + s.completionTokens)}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Recent logs */}
      <div style={{ marginTop: "1.5rem" }}>
        <h2 style={{ fontSize: "1.3rem" }}>Recent requests</h2>
        {logs.length === 0 ? (
          <div className="kid-card kid-wobble kid-tilt" style={{ textAlign: "center", ["--tilt" as string]: "0.6deg" }}>
            <div style={{ fontSize: "2rem" }}>🗒️</div>
            <p style={{ fontFamily: "var(--font-body)" }}>No requests logged yet. Send a chat to see usage here!</p>
          </div>
        ) : (
          <div className="kid-card kid-wobble" style={{ padding: 0, overflow: "hidden" }}>
            {logs.slice(0, 50).map((line, i) => (
              <div
                key={i}
                style={{
                  fontFamily: "var(--font-body)", fontSize: "0.85rem", padding: "0.45rem 0.8rem",
                  borderBottom: i < Math.min(logs.length, 50) - 1 ? "1px solid var(--color-bg-alt)" : "none",
                  whiteSpace: "pre-wrap", wordBreak: "break-word",
                }}
              >
                {line}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
