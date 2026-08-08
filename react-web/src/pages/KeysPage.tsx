// API keys page: list, create (POST /api/keys), delete, copy to clipboard.
// Backed by useKeysStore. The key string is shown once after creation (it is
// persisted, but copying at create-time is the common UX).
import { useEffect, useState } from "react";
import { useKeysStore } from "@/store/keysStore";
import { useNotificationStore } from "@/store/notificationStore";

export default function KeysPage() {
  const keys = useKeysStore((s) => s.keys);
  const loading = useKeysStore((s) => s.loading);
  const error = useKeysStore((s) => s.error);
  const fetchKeys = useKeysStore((s) => s.fetchKeys);
  const createKey = useKeysStore((s) => s.createKey);
  const deleteKey = useKeysStore((s) => s.deleteKey);
  const notify = useNotificationStore();

  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    fetchKeys();
  }, [fetchKeys]);

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    setCreating(true);
    const created = await createKey(name.trim());
    setCreating(false);
    if (created) {
      notify.success(`Key "${created.name}" created`);
      setName("");
      // Copy the new key to the clipboard for convenience.
      navigator.clipboard?.writeText(created.key).catch(() => {});
    } else {
      notify.error("Failed to create key");
    }
  }

  async function handleDelete(id: string, keyName: string) {
    if (!confirm(`Delete key "${keyName}"? This can't be undone.`)) return;
    const ok = await deleteKey(id);
    if (ok) notify.success("Key deleted");
    else notify.error("Failed to delete key");
  }

  function copy(key: string) {
    navigator.clipboard?.writeText(key).then(
      () => notify.success("Copied to clipboard"),
      () => notify.error("Couldn't copy")
    );
  }

  return (
    <div className="fade-in">
      <h1 style={{ fontSize: "2rem", marginTop: 0 }}>🔑 API Keys</h1>
      <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", marginTop: "-0.5rem" }}>
        Keys your tools use to call the <code>/v1/chat/completions</code> endpoint.
      </p>

      {/* Create form */}
      <form onSubmit={handleCreate} className="kid-card kid-wobble kid-tilt" style={{ display: "flex", gap: "0.75rem", alignItems: "flex-end", flexWrap: "wrap", marginBottom: "1.5rem" }}>
        <div style={{ flex: "1 1 240px" }}>
          <label htmlFor="keyname" style={{ display: "block", fontFamily: "var(--font-body)", marginBottom: "0.3rem" }}>New key name</label>
          <input id="keyname" className="kid-input" placeholder="e.g. my-cursor" value={name} onChange={(e) => setName(e.target.value)} disabled={creating} />
        </div>
        <button type="submit" className="kid-btn kid-btn--primary" disabled={creating || !name.trim()}>＋ Create</button>
      </form>

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading keys…</p>}
      {error && <div className="kid-card kid-wobble" style={{ background: "var(--color-danger)", color: "#fff" }}>{error}</div>}

      {/* Key list */}
      {keys.length === 0 && !loading && !error && (
        <div className="kid-card kid-wobble kid-tilt" style={{ textAlign: "center", ["--tilt" as string]: "0.9deg" }}>
          <div style={{ fontSize: "2.5rem" }}>🗝️</div>
          <p style={{ fontFamily: "var(--font-body)" }}>No keys yet — create one above!</p>
        </div>
      )}

      <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))" }}>
        {keys.map((k) => (
          <div key={k.id} className="kid-card kid-wobble">
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", gap: "0.5rem" }}>
              <strong style={{ fontSize: "1.2rem" }}>{k.name}</strong>
              <span style={{ fontFamily: "var(--font-body)", fontSize: "0.85rem", color: k.isActive === false ? "var(--color-danger)" : "var(--color-success)" }}>
                {k.isActive === false ? "off" : "active"}
              </span>
            </div>
            <code style={{ display: "block", fontFamily: "var(--font-body)", background: "var(--color-bg-alt)", padding: "0.4rem 0.6rem", marginTop: "0.5rem", wordBreak: "break-all", border: "2px solid var(--nb-border)" }}>
              {k.key.slice(0, 24)}…
            </code>
            <div style={{ display: "flex", gap: "0.5rem", marginTop: "0.75rem" }}>
              <button className="kid-btn kid-btn--accent" style={{ padding: "0.4rem 0.7rem", fontSize: "0.95rem" }} onClick={() => copy(k.key)}>📋 Copy</button>
              <button className="kid-btn" style={{ padding: "0.4rem 0.7rem", fontSize: "0.95rem", background: "var(--color-danger)", color: "#fff" }} onClick={() => handleDelete(k.id, k.name)}>🗑 Delete</button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
