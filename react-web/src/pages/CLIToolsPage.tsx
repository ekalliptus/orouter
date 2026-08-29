// CLI Tools page — functional parity-lite with 9Router's CLI tools: pick an
// API key, then copy ready-to-paste env/config snippets that point each
// developer tool at this ORouter instance.
import { useEffect, useMemo, useState } from "react";
import { useKeysStore } from "@/store/keysStore";
import { useNotificationStore } from "@/store/notificationStore";
import { Badge } from "@/components/ui";

interface ToolDef {
  id: string;
  name: string;
  description: string;
  emoji: string;
  snippet: (baseUrl: string, key: string, model: string) => string;
}

export default function CLIToolsPage() {
  const keys = useKeysStore((s) => s.keys);
  const fetchKeys = useKeysStore((s) => s.fetchKeys);
  const notify = useNotificationStore();

  const [keyId, setKeyId] = useState("");
  const [model, setModel] = useState("openrouter/openai/gpt-4o-mini");

  useEffect(() => {
    fetchKeys().then(() => {
      const ks = useKeysStore.getState().keys;
      const active = ks.find((k) => k.isActive !== false);
      if (active) setKeyId(active.id);
    });
  }, [fetchKeys]);

  const selectedKey = keys.find((k) => k.id === keyId)?.key ?? "<your-sk-key>";
  const baseUrl = typeof window !== "undefined" ? `${window.location.protocol}//${window.location.host}` : "http://127.0.0.1:20130";

  const TOOLS: ToolDef[] = useMemo(() => [
    {
      id: "claude",
      name: "Claude Code",
      description: "Anthropic CLI agent — point it at ORouter with env vars.",
      emoji: "🧑‍💻",
      snippet: (b, k, m) => [
        `# Claude Code → ORouter`,
        `export ANTHROPIC_BASE_URL="${b}"`,
        `export ANTHROPIC_AUTH_TOKEN="${k}"`,
        `export ANTHROPIC_MODEL="${m}"`,
        `claude`,
      ].join("\n"),
    },
    {
      id: "codex",
      name: "Codex CLI",
      description: "OpenAI Codex CLI via OpenAI-compatible endpoint.",
      emoji: "🧠",
      snippet: (b, k, m) => [
        `# ~/.codex/config.toml`,
        `model = "${m}"`,
        `model_provider = "orouter"`,
        ``,
        `[model_providers.orouter]`,
        `name = "ORouter"`,
        `base_url = "${b}/v1"`,
        `env_key = "OROUTER_API_KEY"`,
        ``,
        `# then: export OROUTER_API_KEY="${k}" && codex`,
      ].join("\n"),
    },
    {
      id: "cursor",
      name: "Cursor / Windsurf",
      description: "Override OpenAI Base URL in Model settings.",
      emoji: "🎨",
      snippet: (b, k) => [
        `Cursor → Settings → Models → OpenAI API Key`,
        `Base URL: ${b}/v1`,
        `API Key:  ${k}`,
        `(enable the models you want, then verify with a small chat)`,
      ].join("\n"),
    },
    {
      id: "generic-openai",
      name: "Generic OpenAI SDK",
      description: "Any OpenAI-compatible client (curl, SDK, LangChain…).",
      emoji: "🔌",
      snippet: (b, k, m) => [
        `curl ${b}/v1/chat/completions \\`,
        `  -H "Authorization: Bearer ${k}" \\`,
        `  -H "Content-Type: application/json" \\`,
        `  -d '{"model":"${m}","messages":[{"role":"user","content":"hi"}]}'`,
      ].join("\n"),
    },
  ], []);

  function copyText(text: string, label: string) {
    navigator.clipboard?.writeText(text).then(
      () => notify.success(`Copied ${label}`),
      () => notify.error("Failed to copy")
    );
  }

  return (
    <div className="fade-in flex flex-col gap-4" style={{ maxWidth: 1000 }}>
      {/* Key + model pickers */}
      <div className="kid-card kid-wobble" style={{ display: "flex", flexWrap: "wrap", gap: "0.6rem", alignItems: "center", padding: "0.8rem 1rem" }}>
        <label style={{ fontFamily: "var(--font-body)", fontWeight: 700 }}>API Key:</label>
        <select className="kid-input" style={{ width: "auto", minWidth: 220 }} value={keyId} onChange={(e) => setKeyId(e.target.value)}>
          {keys.length === 0 && <option value="">(no keys — create one in Endpoint page)</option>}
          {keys.map((k) => (
            <option key={k.id} value={k.id}>
              {k.name} {k.isActive === false ? "(disabled)" : ""}
            </option>
          ))}
        </select>
        <label style={{ fontFamily: "var(--font-body)", fontWeight: 700 }}>Model:</label>
        <input className="kid-input" style={{ width: "auto", minWidth: 240 }} value={model} onChange={(e) => setModel(e.target.value)} placeholder="provider/model" />
        {keyId && <Badge variant="success" size="sm" dot>ready</Badge>}
      </div>

      <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(420px, 1fr))" }}>
        {TOOLS.map((t) => {
          const snippet = t.snippet(baseUrl, selectedKey, model);
          return (
            <div key={t.id} className="kid-card kid-wobble">
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: "0.5rem" }}>
                <div style={{ display: "flex", gap: "0.6rem", alignItems: "center" }}>
                  <div style={{ fontSize: "2rem" }}>{t.emoji}</div>
                  <div>
                    <strong style={{ fontSize: "1.2rem" }}>{t.name}</strong>
                    <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", fontSize: "0.9rem" }}>{t.description}</div>
                  </div>
                </div>
                <button
                  className="kid-btn kid-btn--primary"
                  style={{ padding: "0.3rem 0.6rem", fontSize: "0.9rem" }}
                  onClick={() => copyText(snippet, `${t.name} config`)}
                >
                  📋 Copy
                </button>
              </div>
              <pre
                style={{
                  fontFamily: "var(--font-body)", fontSize: "0.8rem", marginTop: "0.75rem",
                  background: "var(--color-bg-alt)", border: "1px solid var(--nb-border)",
                  padding: "0.6rem 0.7rem", overflowX: "auto", whiteSpace: "pre-wrap", wordBreak: "break-word",
                }}
              >
                {snippet}
              </pre>
            </div>
          );
        })}
      </div>
    </div>
  );
}
