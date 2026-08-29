// Usage page: stats overview (totals + by-provider + by-model), per-day bar
// chart, and recent log lines. Backed by GET /api/usage/stats?period=,
// GET /api/usage/chart?period= and GET /api/usage/logs.
import { useEffect, useState, useCallback } from "react";
import { useNotificationStore } from "@/store/notificationStore";

interface BucketStat {
  requests: number;
  promptTokens: number;
  completionTokens: number;
  cachedTokens?: number;
  cost: number;
}
interface UsageStats {
  totalRequests: number;
  totalPromptTokens: number;
  totalCompletionTokens: number;
  totalCachedTokens: number;
  totalCost: number;
  byProvider: Record<string, BucketStat>;
  byModel: Record<string, BucketStat>;
  recentRequests?: { timestamp: string; model: string; provider: string; promptTokens: number; completionTokens: number; status: string }[];
}
interface ChartPoint {
  date: string;
  requests: number;
  promptTokens: number;
  completionTokens: number;
  cost: number;
}

const PERIODS: { value: string; label: string }[] = [
  { value: "today", label: "Today" },
  { value: "24h", label: "24h" },
  { value: "7d", label: "7d" },
  { value: "30d", label: "30d" },
  { value: "60d", label: "60d" },
  { value: "all", label: "All" },
];

function fmt(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1) + "k";
  return String(n);
}

function fmtCost(c: number): string {
  if (c === 0) return "$0";
  if (c < 1) return `$${c.toFixed(4)}`;
  return `$${c.toFixed(2)}`;
}

export default function UsagePage() {
  const [period, setPeriod] = useState("7d");
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [chart, setChart] = useState<ChartPoint[]>([]);
  const [logs, setLogs] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const notify = useNotificationStore();

  const load = useCallback(async (p: string) => {
    setLoading(true);
    try {
      const [statsRes, chartRes, logsRes] = await Promise.all([
        fetch(`/api/usage/stats?period=${p}`, { credentials: "include" }),
        fetch(`/api/usage/chart?period=${p}`, { credentials: "include" }),
        fetch("/api/usage/logs", { credentials: "include" }),
      ]);
      if (statsRes.ok) setStats(await statsRes.json());
      if (chartRes.ok) setChart(((await chartRes.json()) as { series?: ChartPoint[] }).series ?? []);
      if (logsRes.ok) setLogs((await logsRes.json()) as string[]);
    } catch {
      notify.error("Failed to load usage data");
    } finally {
      setLoading(false);
    }
  }, [notify]);

  useEffect(() => { load(period); }, [period, load]);

  const providers = stats ? Object.entries(stats.byProvider).sort((a, b) => b[1].requests - a[1].requests) : [];
  const models = stats ? Object.entries(stats.byModel).sort((a, b) => b[1].requests - a[1].requests).slice(0, 12) : [];
  const maxRequests = chart.reduce((mx, p) => Math.max(mx, p.requests), 0);

  return (
    <div className="fade-in">
      {/* Period tabs — page title lives in the Header (Node parity) */}
      <div style={{ display: "flex", justifyContent: "flex-end", flexWrap: "wrap", gap: "0.4rem" }}>
        {PERIODS.map((p) => (
          <button
            key={p.value}
            className="kid-btn"
            style={{
              padding: "0.3rem 0.7rem", fontSize: "0.8rem",
              background: period === p.value ? "var(--color-accent)" : "var(--color-surface)",
            }}
            onClick={() => setPeriod(p.value)}
          >
            {p.label}
          </button>
        ))}
      </div>

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading usage…</p>}

      {/* Totals — Node OverviewCards style: uppercase muted label, bold value */}
      {stats && !loading && (
        <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", marginTop: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))" }}>
          {[
            { label: "Requests", value: fmt(stats.totalRequests) },
            { label: "Prompt tokens", value: fmt(stats.totalPromptTokens) },
            { label: "Completion tokens", value: fmt(stats.totalCompletionTokens) },
            { label: "Cached tokens", value: fmt(stats.totalCachedTokens) },
            { label: "Cost", value: fmtCost(stats.totalCost) },
          ].map((c) => (
            <div key={c.label} className="kid-card kid-wobble" style={{ padding: "0.75rem 1rem", display: "flex", flexDirection: "column", gap: "0.25rem" }}>
              <div style={{ fontFamily: "var(--font-body)", fontSize: "0.8rem", textTransform: "uppercase", fontWeight: 600, letterSpacing: "0.05em", color: "var(--color-text-muted)" }}>{c.label}</div>
              <div style={{ fontSize: "1.6rem", fontWeight: 700, lineHeight: 1.15 }}>{c.value}</div>
            </div>
          ))}
        </div>
      )}

      {/* Per-day chart */}
      {chart.length > 0 && !loading && (
        <div style={{ marginTop: "1.5rem" }}>
          <h2 style={{ fontSize: "1.3rem" }}>Daily requests</h2>
          <div className="kid-card kid-wobble" style={{ display: "flex", alignItems: "flex-end", gap: "0.45rem", height: 160, padding: "1rem", overflowX: "auto" }}>
            {chart.map((p) => (
              <div key={p.date} style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: "0.3rem", minWidth: 34 }} title={`${p.date}: ${p.requests} req, ${fmt(p.promptTokens + p.completionTokens)} tok, ${fmtCost(p.cost)}`}>
                <span style={{ fontFamily: "var(--font-body)", fontSize: "0.75rem", color: "var(--color-text-muted)" }}>{p.requests}</span>
                <div
                  style={{
                    width: 26,
                    height: Math.max(4, Math.round((p.requests / Math.max(maxRequests, 1)) * 100)),
                    background: "var(--color-brand-500)",
                    borderRadius: "6px 6px 2px 2px",
                  }}
                />
                <span style={{ fontFamily: "var(--font-body)", fontSize: "0.7rem", color: "var(--color-text-muted)", whiteSpace: "nowrap" }}>{p.date.slice(5)}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* By provider */}
      {providers.length > 0 && (
        <div style={{ marginTop: "1.5rem" }}>
          <h2 style={{ fontSize: "1.3rem" }}>By provider</h2>
          <div style={{ display: "grid", gap: "0.6rem", marginTop: "0.5rem" }}>
            {providers.map(([name, s]) => (
              <div key={name} className="kid-card kid-wobble" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "0.7rem 1rem", flexWrap: "wrap", gap: "0.5rem" }}>
                <strong>{name}</strong>
                <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", display: "flex", gap: "1.2rem", flexWrap: "wrap" }}>
                  <span>📨 {fmt(s.requests)}</span>
                  <span>🪙 {fmt(s.promptTokens + s.completionTokens)}</span>
                  <span>💰 {fmtCost(s.cost)}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Top models */}
      {models.length > 0 && (
        <div style={{ marginTop: "1.5rem" }}>
          <h2 style={{ fontSize: "1.3rem" }}>Top models</h2>
          <div className="kid-card kid-wobble" style={{ padding: 0, overflowX: "auto", marginTop: "0.5rem" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", textAlign: "left", fontFamily: "var(--font-body)" }}>
              <thead>
                <tr style={{ borderBottom: "3px solid var(--nb-border)", fontSize: "0.7rem", textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--color-text-muted)" }}>
                  <th style={{ padding: "0.5rem 0.8rem" }}>Model</th>
                  <th style={{ padding: "0.5rem 0.8rem", textAlign: "right" }}>Req</th>
                  <th style={{ padding: "0.5rem 0.8rem", textAlign: "right" }}>Tokens</th>
                  <th style={{ padding: "0.5rem 0.8rem", textAlign: "right" }}>Cost</th>
                </tr>
              </thead>
              <tbody>
                {models.map(([name, s]) => (
                  <tr key={name} style={{ borderBottom: "2px solid var(--color-surface-3)" }}>
                    <td style={{ padding: "0.5rem 0.8rem" }}><code style={{ fontSize: "0.85rem" }}>{name}</code></td>
                    <td style={{ padding: "0.5rem 0.8rem", textAlign: "right" }}>{fmt(s.requests)}</td>
                    <td style={{ padding: "0.5rem 0.8rem", textAlign: "right" }}>{fmt(s.promptTokens + s.completionTokens)}</td>
                    <td style={{ padding: "0.5rem 0.8rem", textAlign: "right" }}>{fmtCost(s.cost)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
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
