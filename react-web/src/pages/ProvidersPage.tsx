// M1 placeholder for /dashboard/providers. Real provider CRUD lands in M3
// (the page is 1000 LOC in the old app). For now it renders the provider store
// state in the kid style, so when /api/providers exists the cards populate
// automatically.
import { useEffect } from "react";
import { useProviderStore } from "@/store/providerStore";

export default function ProvidersPage() {
  const providers = useProviderStore((s) => s.providers);
  const loading = useProviderStore((s) => s.loading);
  const error = useProviderStore((s) => s.error);
  const fetchProviders = useProviderStore((s) => s.fetchProviders);

  useEffect(() => {
    fetchProviders();
  }, [fetchProviders]);

  return (
    <div className="fade-in">
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: "0.5rem" }}>
        <h1 style={{ fontSize: "2rem", margin: 0 }}>🤖 Providers</h1>
        <button className="kid-btn kid-btn--pink kid-wobble">+ Add provider</button>
      </div>

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading providers…</p>}
      {error && (
        <div className="kid-card kid-wobble" style={{ background: "var(--color-danger)", color: "#fff", marginTop: "1rem" }}>
          {error} (the Rust backend doesn't serve /api/providers yet)
        </div>
      )}

      {providers.length === 0 && !loading && !error && (
        <div className="kid-card kid-wobble kid-tilt" style={{ marginTop: "1rem", textAlign: "center", ["--tilt" as string]: "0.9deg" }}>
          <div style={{ fontSize: "2.5rem" }}>🎨</div>
          <p style={{ fontFamily: "var(--font-body)" }}>No providers yet. Add your first one in M3!</p>
        </div>
      )}

      <div className="kid-tilt-list" style={{ display: "grid", gap: "1rem", marginTop: "1rem", gridTemplateColumns: "repeat(auto-fill, minmax(260px, 1fr))" }}>
        {providers.map((p) => (
          <div key={p._id} className="kid-card kid-wobble">
            <div style={{ fontWeight: 700, fontSize: "1.2rem" }}>{p.name ?? p.provider}</div>
            <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)" }}>
              {p.provider} · {p.authType} · {p.isActive ? "active" : "off"}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
