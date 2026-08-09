// Rich Providers Page — ports 9Router's full providers page layout
// (Category sections, status badges, connection count, Test & Delete actions).
import { useEffect, useState } from "react";
import { useProviderStore, type ProviderConnection } from "@/store/providerStore";
import { useNotificationStore } from "@/store/notificationStore";
import { Button, Badge, Card } from "@/components/ui";
import ProviderIcon from "@/components/ProviderIcon";

const PROVIDER_CATEGORIES: Record<string, string[]> = {
  "API Key Providers": [
    "openrouter", "openai", "deepseek", "groq", "mistral", "xai",
    "anthropic", "gemini", "together", "fireworks", "siliconflow", "cohere",
    "nebius", "cerebras", "chutes", "perplexity"
  ],
  "Free / Open Providers": [
    "ollama-local", "kilo", "free-tier"
  ],
};

export default function ProvidersPage() {
  const providers = useProviderStore((s) => s.providers);
  const loading = useProviderStore((s) => s.loading);
  const error = useProviderStore((s) => s.error);
  const fetchProviders = useProviderStore((s) => s.fetchProviders);
  const createProvider = useProviderStore((s) => s.createProvider);
  const deleteProvider = useProviderStore((s) => s.deleteProvider);
  const testProvider = useProviderStore((s) => s.testProvider);
  const notify = useNotificationStore();

  const [showAddForm, setShowAddForm] = useState(false);
  const [selectedProvider, setSelectedProvider] = useState("openrouter");
  const [apiKey, setApiKey] = useState("");
  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [testingId, setTestingId] = useState<string | null>(null);

  useEffect(() => {
    fetchProviders();
  }, [fetchProviders]);

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!selectedProvider.trim() || !apiKey.trim() || !name.trim()) return;
    setCreating(true);
    const created = await createProvider({
      provider: selectedProvider.trim(),
      apiKey: apiKey.trim(),
      name: name.trim(),
    });
    setCreating(false);
    if (created) {
      notify.success(`Added provider "${created.name}"`);
      setApiKey("");
      setName("");
      setShowAddForm(false);
    } else {
      notify.error("Failed to add provider");
    }
  }

  async function handleTest(id: string, providerName: string) {
    setTestingId(id);
    const result = await testProvider(id);
    setTestingId(null);
    if (!result) {
      notify.error("Test request failed");
      return;
    }
    if (result.valid) notify.success(`"${providerName}" connection is working!`);
    else notify.error(`"${providerName}" failed: ${result.error ?? "Invalid key or endpoint"}`, "Test Connection");
  }

  async function handleDelete(id: string, providerName: string) {
    if (!confirm(`Delete connection "${providerName}"?`)) return;
    const ok = await deleteProvider(id);
    if (ok) notify.success("Connection deleted");
    else notify.error("Failed to delete connection");
  }

  const activeCount = providers.filter((p) => p.isActive !== false && p.testStatus === "active").length;
  const errorCount = providers.filter((p) => p.testStatus === "error").length;

  return (
    <div className="fade-in flex flex-col gap-6" style={{ maxWidth: 1000 }}>
      {/* Page Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "1rem" }}>
        <div>
          <h1 style={{ fontSize: "2rem", margin: 0 }}>🤖 AI Providers ({providers.length})</h1>
          <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", margin: 0 }}>
            Manage your AI service credentials & connections.
          </p>
        </div>
        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          <Badge variant="success" dot>{activeCount} Active</Badge>
          {errorCount > 0 && <Badge variant="danger" dot>{errorCount} Error</Badge>}
          <Button variant="primary" size="sm" onClick={() => setShowAddForm(!showAddForm)}>
            {showAddForm ? "✕ Close Form" : "＋ Add Provider"}
          </Button>
        </div>
      </div>

      {/* Add Provider Card */}
      {showAddForm && (
        <Card tilt={false} style={{ backgroundColor: "var(--color-brand-50)", borderColor: "var(--color-brand-500)" }}>
          <h2 style={{ fontSize: "1.3rem", marginTop: 0, marginBottom: "1rem" }}>＋ Add New AI Provider Connection</h2>
          <form onSubmit={handleCreate} style={{ display: "grid", gap: "0.75rem" }}>
            <div style={{ display: "grid", gap: "0.75rem", gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))" }}>
              <div>
                <label style={{ display: "block", fontFamily: "var(--font-body)", marginBottom: "0.2rem", fontWeight: 700 }}>
                  Provider
                </label>
                <select
                  className="kid-input"
                  value={selectedProvider}
                  onChange={(e) => setSelectedProvider(e.target.value)}
                  disabled={creating}
                >
                  <option value="openrouter">OpenRouter (Recommended)</option>
                  <option value="openai">OpenAI</option>
                  <option value="deepseek">DeepSeek</option>
                  <option value="groq">Groq</option>
                  <option value="anthropic">Anthropic Claude</option>
                  <option value="gemini">Google Gemini</option>
                  <option value="mistral">Mistral AI</option>
                  <option value="xai">xAI (Grok)</option>
                  <option value="together">Together AI</option>
                  <option value="fireworks">Fireworks AI</option>
                  <option value="siliconflow">SiliconFlow</option>
                  <option value="cohere">Cohere</option>
                  <option value="ollama-local">Ollama (Local)</option>
                </select>
              </div>

              <div>
                <label style={{ display: "block", fontFamily: "var(--font-body)", marginBottom: "0.2rem", fontWeight: 700 }}>
                  Connection Name
                </label>
                <input
                  className="kid-input"
                  placeholder="e.g. Primary OpenRouter"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  disabled={creating}
                />
              </div>

              <div style={{ gridColumn: "1 / -1" }}>
                <label style={{ display: "block", fontFamily: "var(--font-body)", marginBottom: "0.2rem", fontWeight: 700 }}>
                  API Key / Token
                </label>
                <input
                  className="kid-input"
                  type="password"
                  placeholder="sk-..."
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  disabled={creating}
                />
              </div>
            </div>

            <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem", marginTop: "0.5rem" }}>
              <Button variant="secondary" type="button" onClick={() => setShowAddForm(false)} disabled={creating}>
                Cancel
              </Button>
              <Button variant="primary" type="submit" disabled={creating || !name.trim() || !apiKey.trim()}>
                {creating ? "Adding…" : "Save Connection"}
              </Button>
            </div>
          </form>
        </Card>
      )}

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading providers…</p>}
      {error && <Card style={{ backgroundColor: "var(--color-danger)", color: "#fff" }}>{error}</Card>}

      {/* Connected Providers List */}
      {!loading && providers.length === 0 && (
        <Card style={{ textAlign: "center", padding: "3rem 1rem" }}>
          <div style={{ fontSize: "3rem" }}>🤖</div>
          <h2 style={{ margin: "0.5rem 0" }}>No AI Providers Configured</h2>
          <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)" }}>
            Add an OpenRouter, OpenAI, or DeepSeek API key to start routing AI requests!
          </p>
          <Button variant="primary" onClick={() => setShowAddForm(true)} style={{ marginTop: "1rem" }}>
            ＋ Add Your First Provider
          </Button>
        </Card>
      )}

      {/* Grouped by Category */}
      {!loading && providers.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
          {Object.entries(PROVIDER_CATEGORIES).map(([categoryName, providerIds]) => {
            const matched = providers.filter((p) => providerIds.includes(p.provider) || categoryName.includes("API Key"));
            if (categoryName !== "API Key Providers" && matched.length === 0) return null;

            return (
              <div key={categoryName}>
                <h2 style={{ fontSize: "1.2rem", marginBottom: "0.75rem", color: "var(--color-text-muted)" }}>
                  {categoryName}
                </h2>
                <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(300px, 1fr))" }}>
                  {matched.map((p: ProviderConnection) => {
                    const connId = p.id ?? p._id;
                    const statusVariant = p.testStatus === "active" ? "success" : p.testStatus === "error" ? "danger" : "neutral";
                    return (
                      <Card key={connId}>
                        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: "0.5rem" }}>
                          <div style={{ display: "flex", gap: "0.75rem", alignItems: "center" }}>
                            <ProviderIcon providerId={p.provider} size={36} />
                            <div>
                              <strong style={{ fontSize: "1.25rem", display: "block" }}>{p.name ?? p.provider}</strong>
                              <span style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", fontSize: "0.9rem" }}>
                                {p.provider} · {p.authType}
                              </span>
                            </div>
                          </div>
                          <Badge variant={statusVariant} size="sm" dot>
                            {typeof p.testStatus === "string" ? p.testStatus : "unknown"}
                          </Badge>
                        </div>

                        {p.lastError ? (
                          <div style={{ fontFamily: "var(--font-body)", fontSize: "0.85rem", color: "var(--color-danger)", marginTop: "0.5rem", background: "var(--color-bg-alt)", padding: "0.3rem 0.5rem", border: "1px solid var(--nb-border)" }}>
                            {String(p.lastError)}
                          </div>
                        ) : null}

                        <div style={{ display: "flex", gap: "0.5rem", marginTop: "1rem", justifyContent: "flex-end" }}>
                          <Button
                            variant="accent"
                            size="sm"
                            onClick={() => handleTest(connId, p.name ?? p.provider)}
                            disabled={testingId === connId}
                          >
                            {testingId === connId ? "Testing…" : "🧪 Test"}
                          </Button>
                          <Button
                            variant="danger"
                            size="sm"
                            onClick={() => handleDelete(connId, p.name ?? p.provider)}
                          >
                            🗑
                          </Button>
                        </div>
                      </Card>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
