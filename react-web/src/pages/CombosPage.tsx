// Combos page: list + create + delete model combos.
// Backed by GET/POST/DELETE /api/combos.
import { useEffect, useState } from "react";
import { useNotificationStore } from "@/store/notificationStore";

interface Combo {
  id: string;
  name: string;
  kind?: string;
  models: string[];
  createdAt: string;
}

export default function CombosPage() {
  const [combos, setCombos] = useState<Combo[]>([]);
  const [loading, setLoading] = useState(true);
  const [name, setName] = useState("");
  const [modelsInput, setModelsInput] = useState("");
  const [creating, setCreating] = useState(false);
  const notify = useNotificationStore();

  async function fetchCombos() {
    setLoading(true);
    try {
      const res = await fetch("/api/combos", { credentials: "include" });
      const data = (await res.json()) as { combos?: Combo[] };
      if (res.ok) setCombos(data.combos ?? []);
    } catch {
      notify.error("Failed to fetch combos");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    fetchCombos();
  }, []);

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    setCreating(true);
    const models = modelsInput
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean);
    try {
      const res = await fetch("/api/combos", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ name: name.trim(), models }),
      });
      const data = await res.json();
      if (!res.ok) {
        notify.error(data.error ?? "Failed to create combo");
      } else {
        notify.success(`Combo "${name}" created`);
        setName("");
        setModelsInput("");
        fetchCombos();
      }
    } catch {
      notify.error("Failed to create combo");
    } finally {
      setCreating(false);
    }
  }

  async function handleDelete(id: string, comboName: string) {
    if (!confirm(`Delete combo "${comboName}"?`)) return;
    try {
      const res = await fetch(`/api/combos/${id}`, {
        method: "DELETE",
        credentials: "include",
      });
      if (res.ok) {
        notify.success("Combo deleted");
        setCombos((s) => s.filter((c) => c.id !== id));
      } else {
        notify.error("Failed to delete combo");
      }
    } catch {
      notify.error("Failed to delete combo");
    }
  }

  return (
    <div className="fade-in">
      {/* Create form */}
      <form onSubmit={handleCreate} className="kid-card kid-wobble kid-tilt" style={{ marginBottom: "1.5rem", ["--tilt" as string]: "-0.5deg" }}>
        <div style={{ fontWeight: 700, fontSize: "1.15rem", marginBottom: "0.5rem" }}>＋ Create Combo</div>
        <div style={{ display: "grid", gap: "0.6rem" }}>
          <div>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Combo name</label>
            <input className="kid-input" placeholder="e.g. gaskeun" value={name} onChange={(e) => setName(e.target.value)} disabled={creating} />
          </div>
          <div>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Models (one per line, e.g. openrouter/openai/gpt-4o)</label>
            <textarea className="kid-input" rows={3} placeholder="openrouter/openai/gpt-4o&#10;deepseek/deepseek-chat" value={modelsInput} onChange={(e) => setModelsInput(e.target.value)} disabled={creating} />
          </div>
        </div>
        <button type="submit" className="kid-btn kid-btn--primary" style={{ marginTop: "0.75rem" }} disabled={creating || !name.trim()}>
          {creating ? "Creating…" : "Create Combo"}
        </button>
      </form>

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading combos…</p>}

      {combos.length === 0 && !loading && (
        <div className="kid-card kid-wobble kid-tilt" style={{ textAlign: "center", ["--tilt" as string]: "0.8deg" }}>
          <div style={{ fontSize: "2.5rem" }}>🧩</div>
          <p style={{ fontFamily: "var(--font-body)" }}>No combos yet. Create one above!</p>
        </div>
      )}

      <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))" }}>
        {combos.map((c) => (
          <div key={c.id} className="kid-card kid-wobble">
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
              <strong style={{ fontSize: "1.2rem" }}>{c.name}</strong>
              <button className="kid-btn" style={{ padding: "0.2rem 0.5rem", fontSize: "0.85rem", background: "var(--color-danger)", color: "#fff" }} onClick={() => handleDelete(c.id, c.name)}>
                🗑
              </button>
            </div>
            <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", marginTop: "0.5rem" }}>
              {c.models.length} model(s):
            </div>
            <ul style={{ fontFamily: "var(--font-body)", fontSize: "0.95rem", paddingLeft: "1.2rem", margin: "0.25rem 0" }}>
              {c.models.map((m, idx) => (
                <li key={idx}><code>{m}</code></li>
              ))}
            </ul>
          </div>
        ))}
      </div>
    </div>
  );
}
