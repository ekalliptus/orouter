// M1 placeholder for the /dashboard (Endpoint & Keys) page. The real page is
// 1310 LOC in the old app; it lands in M3 once the Rust /api/keys + /api/health
// endpoints exist. For now it shows the kid-styled empty state + a live health
// probe so the shell visibly talks to the backend when M2 is done.
import { useEffect, useState } from "react";

export default function EndpointPage() {
  const [health, setHealth] = useState<"loading" | "up" | "down">("loading");

  useEffect(() => {
    let cancelled = false;
    fetch("/health", { credentials: "include" })
      .then((r) => (r.ok ? "up" : "down"))
      .catch(() => "down")
      .then((s) => {
        if (!cancelled) setHealth(s as typeof health);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const status =
    health === "up" ? { emoji: "🟢", text: "Backend is drawing!", bg: "var(--color-success)" } :
    health === "down" ? { emoji: "🔴", text: "Backend is napping (start Rust backend)", bg: "var(--color-danger)" } :
    { emoji: "🟡", text: "Checking…", bg: "var(--color-warning)" };

  return (
    <div className="fade-in">
      <h1 style={{ fontSize: "2rem", marginTop: 0 }}>🔑 Endpoint & Keys</h1>

      <div
        className="kid-card kid-wobble kid-tilt"
        style={{ display: "flex", alignItems: "center", gap: "0.75rem", ["--tilt" as string]: "-0.8deg" }}
      >
        <span style={{ fontSize: "1.6rem" }}>{status.emoji}</span>
        <div>
          <div style={{ fontWeight: 700 }}>{status.text}</div>
          <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)" }}>
            GET /health → <code>{health === "loading" ? "…" : health}</code>
          </div>
        </div>
      </div>

      <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", marginTop: "1.5rem", gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))" }}>
        {[
          { emoji: "📦", title: "Base URL", body: "Your single OpenAI-compatible endpoint" },
          { emoji: "🗝️", title: "API Keys", body: "Generate sk- keys for your tools" },
          { emoji: "🌐", title: "Tunnels", body: "Cloudflare / Tailscale reachability" },
        ].map((c) => (
          <div key={c.title} className="kid-card kid-wobble">
            <div style={{ fontSize: "2rem" }}>{c.emoji}</div>
            <div style={{ fontWeight: 700, marginTop: "0.4rem" }}>{c.title}</div>
            <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)" }}>{c.body}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
