// Rich Providers Page — ports 9Router's providers layout onto the crayon UI:
// search filter, add connection, per-connection edit (name/email/apiKey),
// enable/disable, priority up/down (server renumbers), Test, Test All, delete.
import { useEffect, useMemo, useState } from "react";
import { useProviderStore, type ProviderConnection } from "@/store/providerStore";
import { useNotificationStore } from "@/store/notificationStore";
import { Button, Badge, Card, Toggle } from "@/components/ui";
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
  const putProvider = useProviderStore((s) => s.putProvider);
  const deleteProvider = useProviderStore((s) => s.deleteProvider);
  const testProvider = useProviderStore((s) => s.testProvider);
  const notify = useNotificationStore();

  const [showAddForm, setShowAddForm] = useState(false);
  const [selectedProvider, setSelectedProvider] = useState("openrouter");
  const [apiKey, setApiKey] = useState("");
  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testAllRunning, setTestAllRunning] = useState(false);
  const [search, setSearch] = useState("");

  // Edit modal state
  const [editing, setEditing] = useState<ProviderConnection | null>(null);
  const [editName, setEditName] = useState("");
  const [editEmail, setEditEmail] = useState("");
  const [editApiKey, setEditApiKey] = useState("");
  const [savingEdit, setSavingEdit] = useState(false);

  useEffect(() => {
    fetchProviders();
  }, [fetchProviders]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return providers;
    return providers.filter(
      (p) =>
        (p.name ?? "").toLowerCase().includes(q) ||
        p.provider.toLowerCase().includes(q)
    );
  }, [providers, search]);

  const activeCount = providers.filter((p) => p.isActive !== false && p.testStatus === "active").length;
  const errorCount = providers.filter((p) => p.testStatus === "error").length;

  function connId(p: ProviderConnection): string {
    return p.id ?? p._id;
  }

  function priorityOf(p: ProviderConnection): number {
    return typeof p.priority === "number" ? p.priority : 99;
  }

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

  function openEdit(p: ProviderConnection) {
    setEditing(p);
    setEditName(p.name ?? "");
    setEditEmail(typeof p.email === "string" ? p.email : "");
    setEditApiKey("");
  }

  async function handleSaveEdit(e: React.FormEvent) {
    e.preventDefault();
    if (!editing) return;
    setSavingEdit(true);
    const patch: Record<string, unknown> = { name: editName.trim() };
    if (editEmail.trim()) patch.email = editEmail.trim();
    if (editApiKey.trim()) patch.apiKey = editApiKey.trim();
    const updated = await putProvider(connId(editing), patch);
    setSavingEdit(false);
    if (updated) {
      notify.success("Connection updated");
      setEditing(null);
    } else {
      notify.error("Failed to update connection");
    }
  }

  async function handleToggleActive(p: ProviderConnection) {
    const next = !(p.isActive !== false);
    const updated = await putProvider(connId(p), { isActive: next });
    if (updated) notify.success(`"${p.name ?? p.provider}" ${next ? "enabled" : "disabled"}`);
    else notify.error("Failed to update connection");
  }

  // Swap this connection's priority with its neighbour; the backend renumbers
  // the whole provider group on a priority change.
  async function handleMove(p: ProviderConnection, dir: -1 | 1) {
    const siblings = providers
      .filter((x) => x.provider === p.provider)
      .sort((a, b) => priorityOf(a) - priorityOf(b));
    const idx = siblings.findIndex((x) => connId(x) === connId(p));
    const swapIdx = idx + dir;
    if (swapIdx < 0 || swapIdx >= siblings.length) return;
    const updated = await putProvider(connId(p), { priority: priorityOf(siblings[swapIdx]) });
    if (updated) {
      await fetchProviders({ force: true });
    } else {
      notify.error("Failed to reorder");
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

  async function handleTestAll() {
    setTestAllRunning(true);
    let pass = 0;
    let fail = 0;
    for (const p of filtered) {
      const result = await testProvider(connId(p));
      if (result?.valid) pass += 1;
      else fail += 1;
    }
    setTestAllRunning(false);
    notify.success(`Test all done: ${pass} pass, ${fail} fail`, "Batch Test");
  }

  async function handleDelete(id: string, providerName: string) {
    if (!confirm(`Delete connection "${providerName}"?`)) return;
    const ok = await deleteProvider(id);
    if (ok) notify.success("Connection deleted");
    else notify.error("Failed to delete connection");
  }

  return (
    <div className="fade-in flex flex-col gap-6" style={{ maxWidth: 1000 }}>
      {/* Actions row — page title lives in the Header (Node parity) */}
      <div style={{ display: "flex", justifyContent: "flex-end", alignItems: "center", flexWrap: "wrap", gap: "0.5rem" }}>
        <Badge variant="success" dot>{activeCount} Active</Badge>
        {errorCount > 0 && <Badge variant="danger" dot>{errorCount} Error</Badge>}
        <Button variant="accent" size="sm" onClick={handleTestAll} disabled={testAllRunning || filtered.length === 0}>
          {testAllRunning ? (
            <><span className="material-symbols-outlined" style={{ fontSize: 16 }}>science</span> Testing all…</>
          ) : (
            <><span className="material-symbols-outlined" style={{ fontSize: 16 }}>science</span> Test All</>
          )}
        </Button>
        <Button variant="primary" size="sm" onClick={() => setShowAddForm(!showAddForm)}>
          {showAddForm ? "✕ Close Form" : "＋ Add Provider"}
        </Button>
      </div>

      {/* Search */}
      <input
        className="kid-input"
        placeholder="Search connections…"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        style={{ maxWidth: 360 }}
      />

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
                  <option value="tokenrouter">TokenRouter</option>
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
      {!loading && filtered.length === 0 && (
        <Card style={{ textAlign: "center", padding: "3rem 1rem" }}>
          <div style={{ fontSize: "3rem" }}>🤖</div>
          <h2 style={{ margin: "0.5rem 0" }}>{providers.length === 0 ? "No AI Providers Configured" : "No matches"}</h2>
          <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)" }}>
            {providers.length === 0
              ? "Add an OpenRouter, OpenAI, or DeepSeek API key to start routing AI requests!"
              : "Try a different search term."}
          </p>
          {providers.length === 0 && (
            <Button variant="primary" onClick={() => setShowAddForm(true)} style={{ marginTop: "1rem" }}>
              ＋ Add Your First Provider
            </Button>
          )}
        </Card>
      )}

      {/* Grouped by Category */}
      {!loading && filtered.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
          {Object.entries(PROVIDER_CATEGORIES).map(([categoryName, providerIds]) => {
            const matched = filtered.filter((p) => providerIds.includes(p.provider) || categoryName.includes("API Key"));
            if (categoryName !== "API Key Providers" && matched.length === 0) return null;

            return (
              <div key={categoryName}>
                <h2 style={{ fontSize: "1.2rem", marginBottom: "0.75rem", color: "var(--color-text-muted)" }}>
                  {categoryName}
                </h2>
                <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(300px, 1fr))" }}>
                  {matched.map((p: ProviderConnection) => {
                    const id = connId(p);
                    const statusVariant = p.testStatus === "active" ? "success" : p.testStatus === "error" ? "danger" : "neutral";
                    const isActive = p.isActive !== false;
                    return (
                      <Card key={id} style={!isActive ? { opacity: 0.55 } : undefined}>
                        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: "0.5rem" }}>
                          <div style={{ display: "flex", gap: "0.75rem", alignItems: "center" }}>
                            <ProviderIcon providerId={p.provider} size={36} />
                            <div>
                              <strong style={{ fontSize: "1.25rem", display: "block" }}>{p.name ?? p.provider}</strong>
                              <span style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", fontSize: "0.9rem" }}>
                                {p.provider} · priority {priorityOf(p)}
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

                        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: "0.85rem", gap: "0.5rem", flexWrap: "wrap" }}>
                          <div style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
                            <Button variant="secondary" size="sm" onClick={() => handleMove(p, -1)} title="Raise priority">▲</Button>
                            <Button variant="secondary" size="sm" onClick={() => handleMove(p, 1)} title="Lower priority">▼</Button>
                            <Toggle checked={isActive} onChange={() => handleToggleActive(p)} />
                          </div>
                          <div style={{ display: "flex", gap: "0.4rem" }}>
                            <Button variant="secondary" size="sm" onClick={() => openEdit(p)}>
                              <span className="material-symbols-outlined" style={{ fontSize: 16 }}>edit</span>
                            </Button>
                            <Button
                              variant="accent"
                              size="sm"
                              onClick={() => handleTest(id, p.name ?? p.provider)}
                              disabled={testingId === id}
                            >
                              {testingId === id ? (
                                <span className="material-symbols-outlined animate-spin" style={{ fontSize: 16 }}>progress_activity</span>
                              ) : (
                                <span className="material-symbols-outlined" style={{ fontSize: 16 }}>science</span>
                              )}
                            </Button>
                            <Button
                              variant="danger"
                              size="sm"
                              onClick={() => handleDelete(id, p.name ?? p.provider)}
                            >
                              <span className="material-symbols-outlined" style={{ fontSize: 16 }}>delete</span>
                            </Button>
                          </div>
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

      {/* Edit Connection Modal */}
      {editing && (
        <div style={{ position: "fixed", inset: 0, backgroundColor: "rgba(0,0,0,0.5)", zIndex: 100, display: "flex", alignItems: "center", justifyContent: "center", padding: "1rem" }}>
          <form onSubmit={handleSaveEdit} className="kid-card kid-wobble" style={{ width: "min(420px, 100%)", background: "var(--color-surface)" }}>
            <h3 style={{ fontSize: "1.4rem", marginTop: 0 }}>✏️ Edit Connection</h3>
            <div style={{ display: "grid", gap: "0.6rem" }}>
              <div>
                <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Name</label>
                <input className="kid-input" value={editName} onChange={(e) => setEditName(e.target.value)} disabled={savingEdit} autoFocus />
              </div>
              <div>
                <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Email (optional)</label>
                <input className="kid-input" type="email" value={editEmail} onChange={(e) => setEditEmail(e.target.value)} disabled={savingEdit} />
              </div>
              <div>
                <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>New API Key (leave blank to keep)</label>
                <input className="kid-input" type="password" placeholder="sk-…" value={editApiKey} onChange={(e) => setEditApiKey(e.target.value)} disabled={savingEdit} />
              </div>
            </div>
            <div style={{ display: "flex", gap: "0.5rem", justifyContent: "flex-end", marginTop: "1.25rem" }}>
              <Button variant="secondary" type="button" onClick={() => setEditing(null)} disabled={savingEdit}>
                Cancel
              </Button>
              <Button variant="primary" type="submit" disabled={savingEdit || !editName.trim()}>
                {savingEdit ? "Saving…" : "Save Changes"}
              </Button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}
