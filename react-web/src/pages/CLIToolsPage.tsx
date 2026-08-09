// CLI Tools page: list available CLI integrations (Claude Code, Cursor, Kiro, Antigravity, Cowork).
// Backed by GET /api/cli-tools.
import { useEffect, useState } from "react";

interface CLITool {
  id: string;
  name: string;
  description: string;
}

export default function CLIToolsPage() {
  const [tools, setTools] = useState<CLITool[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch("/api/cli-tools", { credentials: "include" })
      .then((r) => r.json())
      .then((d) => setTools(d.tools ?? []))
      .catch(() => {})
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="fade-in">
      <h1 style={{ fontSize: "2rem", marginTop: 0 }}>🛠️ CLI Tools & Integrations</h1>
      <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", marginTop: "-0.5rem" }}>
        Connect your local developer tools to ORouter.
      </p>

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading tools…</p>}

      <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))" }}>
        {tools.map((t) => (
          <div key={t.id} className="kid-card kid-wobble">
            <div style={{ fontSize: "2rem", marginBottom: "0.25rem" }}>🛠️</div>
            <strong style={{ fontSize: "1.2rem" }}>{t.name}</strong>
            <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", marginTop: "0.25rem" }}>
              {t.description}
            </p>
            <div style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem", color: "var(--color-success)", fontWeight: 700 }}>
              ✓ Ready for configuration
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
