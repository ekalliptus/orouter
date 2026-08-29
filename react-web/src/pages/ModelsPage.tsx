// Models catalog page (GET /api/models): every provider's models from the
// embedded snapshot with kind, native-routing flag, upstream mapping and
// $/Mtok pricing. Search + kind filter + copy the routable "provider/model" id.
import { useEffect, useMemo, useState } from "react";
import { useNotificationStore } from "@/store/notificationStore";
import { Badge } from "@/components/ui";

interface CatalogModel {
  id: string;
  name?: string;
  kind: string;
  nativeChat?: boolean | null;
  upstreamId?: string;
  inputPrice?: number | null;
  outputPrice?: number | null;
}
interface CatalogProvider {
  provider: string;
  hasNativeTransport: boolean;
  models: CatalogModel[];
}

const KINDS = ["llm", "embedding", "image", "tts", "stt", "video"];

function fmtPrice(p?: number | null): string {
  if (p === null || p === undefined) return "—";
  return `$${p}`;
}

export default function ModelsPage() {
  const [providers, setProviders] = useState<CatalogProvider[]>([]);
  const [loading, setLoading] = useState(true);
  const [provider, setProvider] = useState("");
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState("llm");
  const notify = useNotificationStore();

  useEffect(() => {
    fetch("/api/models", { credentials: "include" })
      .then((r) => r.json())
      .then((d: { providers?: CatalogProvider[] }) => {
        const list = d.providers ?? [];
        setProviders(list);
        // Prefer a provider that actually has models + native transport.
        const first = list.find((p) => p.models.length > 0) ?? list[0];
        if (first) setProvider(first.provider);
      })
      .catch(() => notify.error("Failed to load model catalog"))
      .finally(() => setLoading(false));
  }, [notify]);

  const current = useMemo(
    () => providers.find((p) => p.provider === provider),
    [providers, provider]
  );

  const models = useMemo(() => {
    let list = current?.models ?? [];
    if (kind !== "all") list = list.filter((m) => m.kind === kind);
    const q = search.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (m) =>
          m.id.toLowerCase().includes(q) ||
          (m.name ?? "").toLowerCase().includes(q) ||
          (m.upstreamId ?? "").toLowerCase().includes(q)
      );
    }
    return list;
  }, [current, kind, search]);

  function copyText(text: string, label: string) {
    navigator.clipboard?.writeText(text).then(
      () => notify.success(`Copied ${label}`),
      () => notify.error("Failed to copy")
    );
  }

  return (
    <div className="fade-in flex flex-col gap-4" style={{ maxWidth: 1100 }}>
      {/* Filters — page title lives in the Header (Node parity) */}
      <div className="kid-card kid-wobble" style={{ display: "flex", flexWrap: "wrap", gap: "0.6rem", alignItems: "center", padding: "0.8rem 1rem" }}>
        <select className="kid-input" style={{ width: "auto", minWidth: 220 }} value={provider} onChange={(e) => setProvider(e.target.value)}>
          {providers.map((p) => (
            <option key={p.provider} value={p.provider}>
              {p.provider} ({p.models.length}){p.hasNativeTransport ? " ⚡" : ""}
            </option>
          ))}
        </select>
        <input
          className="kid-input"
          style={{ flex: 1, minWidth: 200 }}
          placeholder="Search model id or name…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <select className="kid-input" style={{ width: "auto" }} value={kind} onChange={(e) => setKind(e.target.value)}>
          <option value="all">All kinds</option>
          {KINDS.map((k) => (
            <option key={k} value={k}>{k}</option>
          ))}
        </select>
      </div>

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading catalog…</p>}

      {/* Legend */}
      {!loading && (
        <div style={{ display: "flex", gap: "0.6rem", flexWrap: "wrap", alignItems: "center", fontFamily: "var(--font-body)", fontSize: "0.9rem", color: "var(--color-text-muted)" }}>
          <Badge variant="success" size="sm" dot>native (Rust direct)</Badge>
          <Badge variant="neutral" size="sm" dot>Node translator</Badge>
          <span>· ⚡ in the provider list = OpenAI-compatible native transport</span>
          <span>· {models.length} model(s) shown</span>
        </div>
      )}

      {/* Table */}
      {!loading && models.length > 0 && (
        <div className="kid-card kid-wobble" style={{ padding: 0, overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", textAlign: "left", fontFamily: "var(--font-body)" }}>
            <thead>
              <tr style={{ borderBottom: "3px solid var(--nb-border)", fontSize: "0.7rem", textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--color-text-muted)" }}>
                <th style={{ padding: "0.6rem 0.8rem" }}>Model</th>
                <th style={{ padding: "0.6rem 0.8rem" }}>Kind</th>
                <th style={{ padding: "0.6rem 0.8rem" }}>Route</th>
                <th style={{ padding: "0.6rem 0.8rem" }}>Upstream</th>
                <th style={{ padding: "0.6rem 0.8rem", textAlign: "right" }}>In / Out $/Mtok</th>
                <th style={{ padding: "0.6rem 0.8rem", textAlign: "right" }}></th>
              </tr>
            </thead>
            <tbody>
              {models.map((m) => {
                const routable = `${provider}/${m.id}`;
                const native = m.nativeChat !== false; // absent flag = native passthrough
                const hasUpstream = !!m.upstreamId && m.upstreamId !== m.id;
                return (
                  <tr key={m.id} style={{ borderBottom: "2px solid var(--color-surface-3)" }}>
                    <td style={{ padding: "0.55rem 0.8rem" }}>
                      <code style={{ fontSize: "0.9rem", background: "var(--color-bg-alt)", padding: "0.15rem 0.35rem", border: "1px solid var(--nb-border)" }}>{m.id}</code>
                      {m.name && m.name !== m.id && (
                        <div style={{ fontSize: "0.85rem", color: "var(--color-text-muted)" }}>{m.name}</div>
                      )}
                    </td>
                    <td style={{ padding: "0.55rem 0.8rem" }}>
                      <Badge variant={m.kind === "llm" ? "info" : "neutral"} size="sm">{m.kind}</Badge>
                    </td>
                    <td style={{ padding: "0.55rem 0.8rem" }}>
                      {native
                        ? <Badge variant="success" size="sm" dot>native</Badge>
                        : <Badge variant="neutral" size="sm" dot>Node</Badge>}
                    </td>
                    <td style={{ padding: "0.55rem 0.8rem", fontSize: "0.85rem", color: "var(--color-text-muted)" }}>
                      {hasUpstream ? <code>{m.upstreamId}</code> : "—"}
                    </td>
                    <td style={{ padding: "0.55rem 0.8rem", textAlign: "right", whiteSpace: "nowrap" }}>
                      {fmtPrice(m.inputPrice)} / {fmtPrice(m.outputPrice)}
                    </td>
                    <td style={{ padding: "0.55rem 0.8rem", textAlign: "right" }}>
                      <button
                        className="kid-btn"
                        style={{ padding: "0.2rem 0.5rem", fontSize: "0.85rem" }}
                        onClick={() => copyText(routable, routable)}
                        title={`Copy ${routable}`}
                      >
                        📋 {provider}/{m.id}
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {!loading && models.length === 0 && (
        <div className="kid-card kid-wobble kid-tilt" style={{ textAlign: "center", ["--tilt" as string]: "0.6deg" }}>
          <div style={{ fontSize: "2.5rem" }}>🔍</div>
          <p style={{ fontFamily: "var(--font-body)" }}>No models match this filter.</p>
        </div>
      )}
    </div>
  );
}
