// Providers page: list + create + delete + test connection.
// Backed by useProviderStore (createProvider/deleteProvider/testProvider).
// The Rust backend serves GET/POST /api/providers, PUT/DELETE /api/providers/:id,
// POST /api/providers/:id/test.
import { useEffect, useState } from "react";
import { useProviderStore } from "@/store/providerStore";
import { useNotificationStore } from "@/store/notificationStore";

const STATUS_META: Record<string, { emoji: string; color: string }> = {
  active: { emoji: "✅", color: "var(--color-success)" },
  error: { emoji: "❌", color: "var(--color-danger)" },
  unknown: { emoji: "❓", color: "var(--color-text-subtle)" },
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

  const [provider, setProvider] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [testingId, setTestingId] = useState<string | null>(null);

  useEffect(() => {
    fetchProviders();
  }, [fetchProviders]);

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!provider.trim() || !apiKey.trim() || !name.trim()) return;
    setCreating(true);
    const created = await createProvider({ provider: provider.trim(), apiKey: apiKey.trim(), name: name.trim() });
    setCreating(false);
    if (created) {
      notify.success(`Added "${created.name}"`);
      setProvider(""); setApiKey(""); setName("");
    } else {
      notify.error("Failed to add provider");
    }
  }

  async function handleTest(id: string, name: string) {
    setTestingId(id);
    const result = await testProvider(id);
    setTestingId(null);
    if (!result) { notify.error("Test failed"); return; }
    if (result.valid) notify.success(`"${name}" is working!`);
    else notify.error(`"${name}" failed: ${result.error ?? "unknown"}`, "Connection test");
  }

  async function handleDelete(id: string, name: string) {
    if (!confirm(`Delete "${name}"? This can't be undone.`)) return;
    const ok = await deleteProvider(id);
    if (ok) notify.success("Provider deleted");
    else notify.error("Failed to delete provider");
  }

  return (
    <div className="fade-in">
      <h1 style={{ fontSize: "2rem", marginTop: 0 }}>🤖 Providers</h1>

      {/* Create form */}
      <form onSubmit={handleCreate} className="kid-card kid-wobble kid-tilt" style={{ marginBottom: "1.5rem", ["--tilt" as string]: "-0.7deg" }}>
        <div style={{ fontWeight: 700, fontSize: "1.15rem", marginBottom: "0.75rem" }}>＋ Add a provider</div>
        <div style={{ display: "grid", gap: "0.6rem", gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))" }}>
          <div>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Provider id</label>
            <input className="kid-input" placeholder="e.g. openrouter" value={provider} onChange={(e) => setProvider(e.target.value)} disabled={creating} />
          </div>
          <div>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Name</label>
            <input className="kid-input" placeholder="e.g. My OpenRouter" value={name} onChange={(e) => setName(e.target.value)} disabled={creating} />
          </div>
          <div style={{ gridColumn: "1 / -1" }}>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>API Key</label>
            <input className="kid-input" type="password" placeholder="sk-..." value={apiKey} onChange={(e) => setApiKey(e.target.value)} disabled={creating} />
          </div>
        </div>
        <button type="submit" className="kid-btn kid-btn--primary" style={{ marginTop: "0.75rem" }} disabled={creating || !provider.trim() || !apiKey.trim() || !name.trim()}>
          {creating ? "Adding…" : "Add provider"}
        </button>
      </form>

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading providers…</p>}
      {error && <div className="kid-card kid-wobble" style={{ background: "var(--color-danger)", color: "#fff" }}>{error}</div>}

      {/* Provider cards */}
      {providers.length === 0 && !loading && !error && (
        <div className="kid-card kid-wobble kid-tilt" style={{ textAlign: "center", ["--tilt" as string]: "0.9deg" }}>
          <div style={{ fontSize: "2.5rem" }}>🎨</div>
          <p style={{ fontFamily: "var(--font-body)" }}>No providers yet. Add your first one above!</p>
        </div>
      )}

      <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(300px, 1fr))" }}>
        {providers.map((p) => {
          const id = p._id;
          const status = STATUS_META[(p.testStatus as string) ?? "unknown"] ?? STATUS_META.unknown;
          return (
            <div key={id} className="kid-card kid-wobble">
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", gap: "0.5rem" }}>
                <strong style={{ fontSize: "1.2rem" }}>{p.name ?? p.provider}</strong>
                <span title={p.testStatus as string} style={{ fontSize: "1.3rem" }}>{status.emoji}</span>
              </div>
              <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", marginTop: "0.2rem" }}>
                {p.provider} · {p.authType} · {p.isActive === false ? "off" : "active"}
              </div>
              {(p.lastError as string) && (
                <div style={{ fontFamily: "var(--font-body)", fontSize: "0.85rem", color: "var(--color-danger)", marginTop: "0.4rem", wordBreak: "break-word" }}>
                  {p.lastError as string}
                </div>
              )}
              <div style={{ display: "flex", gap: "0.5rem", marginTop: "0.75rem" }}>
                <button className="kid-btn kid-btn--blue" style={{ padding: "0.4rem 0.7rem", fontSize: "0.95rem" }} onClick={() => handleTest(id, p.name ?? p.provider)} disabled={testingId === id}>
                  {testingId === id ? "Testing…" : "🧪 Test"}
                </button>
                <button className="kid-btn" style={{ padding: "0.4rem 0.7rem", fontSize: "0.95rem", background: "var(--color-danger)", color: "#fff" }} onClick={() => handleDelete(id, p.name ?? p.provider)}>
                  🗑
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
