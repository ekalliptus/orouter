// Rich Endpoint & API Keys page — ports 9Router's full EndpointPageClient layout
// (Endpoint URL bar, API Keys table with masked key toggle, Add Key modal, and Security toggles).
import { useEffect, useState } from "react";
import { useKeysStore, type ApiKey } from "@/store/keysStore";
import { useSettingsStore } from "@/store/settingsStore";
import { useNotificationStore } from "@/store/notificationStore";
import { Button, Badge, Card, Toggle } from "@/components/ui";

export default function EndpointPage() {
  const keys = useKeysStore((s) => s.keys);
  const loadingKeys = useKeysStore((s) => s.loading);
  const fetchKeys = useKeysStore((s) => s.fetchKeys);
  const createKey = useKeysStore((s) => s.createKey);
  const deleteKey = useKeysStore((s) => s.deleteKey);
  const setKeyActive = useKeysStore((s) => s.setKeyActive);

  const settings = useSettingsStore((s) => s.settings);
  const fetchSettings = useSettingsStore((s) => s.fetchSettings);
  const patchSettings = useSettingsStore((s) => s.patchSettings);
  const notify = useNotificationStore();

  const [showAddModal, setShowAddModal] = useState(false);
  const [newKeyName, setNewKeyName] = useState("");
  const [creatingKey, setCreatingKey] = useState(false);
  const [visibleKeys, setVisibleKeys] = useState<Set<string>>(new Set());

  useEffect(() => {
    fetchKeys();
    fetchSettings();
  }, [fetchKeys, fetchSettings]);

  // Compute origin endpoint
  const endpointUrl = typeof window !== "undefined"
    ? `${window.location.protocol}//${window.location.host}/v1`
    : "http://localhost:20130/v1";

  function copyText(text: string, label: string) {
    navigator.clipboard?.writeText(text).then(
      () => notify.success(`Copied ${label} to clipboard`),
      () => notify.error("Failed to copy")
    );
  }

  function toggleKeyVisibility(id: string) {
    setVisibleKeys((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  async function handleCreateKey(e: React.FormEvent) {
    e.preventDefault();
    if (!newKeyName.trim()) return;
    setCreatingKey(true);
    const created = await createKey(newKeyName.trim());
    setCreatingKey(false);
    if (created) {
      notify.success(`API Key "${created.name}" created!`);
      setNewKeyName("");
      setShowAddModal(false);
    } else {
      notify.error("Failed to create API Key");
    }
  }

  async function handleDeleteKey(id: string, name: string) {
    if (!confirm(`Delete API Key "${name}"?`)) return;
    const ok = await deleteKey(id);
    if (ok) notify.success("Key deleted");
    else notify.error("Failed to delete key");
  }

  async function handleToggleKey(k: ApiKey) {
    const next = !(k.isActive !== false);
    const ok = await setKeyActive(k.id, next);
    if (ok) notify.success(`Key "${k.name}" ${next ? "enabled" : "disabled"}`);
    else notify.error("Failed to update key");
  }

  async function handleToggleSetting(key: string, value: boolean) {
    const updated = await patchSettings({ [key]: value });
    if (updated) {
      notify.success(`Updated ${key}`);
    } else {
      notify.error("Failed to update setting");
    }
  }

  return (
    <div className="fade-in flex flex-col gap-6" style={{ maxWidth: 1000 }}>
      {/* Endpoint URL Box */}
      <Card tilt={false} style={{ backgroundColor: "var(--color-brand-50)", borderColor: "var(--color-brand-500)" }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "1rem" }}>
          <div>
            <div style={{ fontSize: "0.85rem", fontWeight: 700, textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--color-brand-700)" }}>
              OpenAI Base URL
            </div>
            <code style={{ fontSize: "1.25rem", fontWeight: 700, color: "var(--color-text-main)", background: "transparent" }}>
              {endpointUrl}
            </code>
          </div>
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <Button variant="accent" size="sm" onClick={() => copyText(endpointUrl, "Base URL")}>
              📋 Copy Base URL
            </Button>
          </div>
        </div>
      </Card>

      {/* API Keys Table Section */}
      <Card tilt={false}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1rem", flexWrap: "wrap", gap: "0.5rem" }}>
          <div>
            <h2 style={{ fontSize: "1.3rem", margin: 0 }}>🗝️ API Keys ({keys.length})</h2>
            <span style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", fontSize: "0.95rem" }}>
              Authenticate your developer tools against ORouter.
            </span>
          </div>
          <Button variant="primary" size="sm" onClick={() => setShowAddModal(true)}>
            ＋ Create New Key
          </Button>
        </div>

        {loadingKeys && <p style={{ fontFamily: "var(--font-body)" }}>Loading API keys…</p>}

        {!loadingKeys && keys.length === 0 && (
          <div style={{ textAlign: "center", padding: "2rem 1rem", fontFamily: "var(--font-body)" }}>
            <div style={{ fontSize: "2.5rem" }}>🔑</div>
            <p>No API keys yet. Create your first key to start making AI calls!</p>
          </div>
        )}

        {!loadingKeys && keys.length > 0 && (
          <div style={{ overflowX: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", textAlign: "left", fontFamily: "var(--font-body)" }}>
              <thead>
                <tr style={{ borderBottom: "3px solid var(--nb-border)", fontSize: "0.7rem", textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--color-text-muted)" }}>
                  <th style={{ padding: "0.6rem 0.8rem" }}>Name</th>
                  <th style={{ padding: "0.6rem 0.8rem" }}>API Key</th>
                  <th style={{ padding: "0.6rem 0.8rem" }}>Status</th>
                  <th style={{ padding: "0.6rem 0.8rem", textAlign: "right" }}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {keys.map((k: ApiKey) => {
                  const isVisible = visibleKeys.has(k.id);
                  const displayKey = isVisible ? k.key : `${k.key.slice(0, 10)}...${k.key.slice(-6)}`;
                  return (
                    <tr key={k.id} style={{ borderBottom: "2px solid var(--color-surface-3)" }}>
                      <td style={{ padding: "0.75rem 0.8rem", fontWeight: 700 }}>{k.name}</td>
                      <td style={{ padding: "0.75rem 0.8rem" }}>
                        <code style={{ background: "var(--color-bg-alt)", padding: "0.2rem 0.4rem", border: "1px solid var(--nb-border)", fontSize: "0.9rem" }}>
                          {displayKey}
                        </code>
                      </td>
                      <td style={{ padding: "0.75rem 0.8rem" }}>
                        <div style={{ display: "inline-flex", alignItems: "center", gap: "0.5rem" }}>
                          <Badge variant={k.isActive !== false ? "success" : "danger"} size="sm" dot>
                            {k.isActive !== false ? "Active" : "Disabled"}
                          </Badge>
                          <Toggle
                            checked={k.isActive !== false}
                            onChange={() => handleToggleKey(k)}
                          />
                        </div>
                      </td>
                      <td style={{ padding: "0.75rem 0.8rem", textAlign: "right" }}>
                        <div style={{ display: "inline-flex", gap: "0.4rem" }}>
                          <Button variant="secondary" size="sm" onClick={() => toggleKeyVisibility(k.id)}>
                            {isVisible ? "🙈 Hide" : "👁️ Show"}
                          </Button>
                          <Button variant="accent" size="sm" onClick={() => copyText(k.key, "API Key")}>
                            📋 Copy
                          </Button>
                          <Button variant="danger" size="sm" onClick={() => handleDeleteKey(k.id, k.name)}>
                            🗑
                          </Button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      {/* Security & Settings Section */}
      {settings && (
        <Card tilt={false}>
          <h2 style={{ fontSize: "1.3rem", marginTop: 0, marginBottom: "1rem" }}>🛡️ Endpoint Security Settings</h2>
          <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <strong style={{ fontSize: "1.05rem" }}>Require API Key for Requests</strong>
                <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", margin: 0, fontSize: "0.95rem" }}>
                  Fail-closed: /v1/* endpoints reject requests without a valid sk- key.
                </p>
              </div>
              <Toggle
                checked={!!settings.requireApiKey}
                onChange={(checked) => handleToggleSetting("requireApiKey", checked)}
              />
            </div>

            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", borderTop: "2px dashed var(--color-surface-3)", paddingTop: "1rem" }}>
              <div>
                <strong style={{ fontSize: "1.05rem" }}>Require Dashboard Login</strong>
                <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", margin: 0, fontSize: "0.95rem" }}>
                  Protect web dashboard access with password authentication.
                </p>
              </div>
              <Toggle
                checked={!!settings.requireLogin}
                onChange={(checked) => handleToggleSetting("requireLogin", checked)}
              />
            </div>
          </div>
        </Card>
      )}

      {/* Add Key Modal */}
      {showAddModal && (
        <div style={{ position: "fixed", inset: 0, backgroundColor: "rgba(0,0,0,0.5)", zIndex: 100, display: "flex", alignItems: "center", justifyContent: "center", padding: "1rem" }}>
          <form onSubmit={handleCreateKey} className="kid-card kid-wobble" style={{ width: "min(400px, 100%)", background: "var(--color-surface)" }}>
            <h3 style={{ fontSize: "1.4rem", marginTop: 0 }}>＋ Create New API Key</h3>
            <label style={{ display: "block", fontFamily: "var(--font-body)", marginBottom: "0.4rem" }}>Key Name</label>
            <input
              className="kid-input"
              placeholder="e.g. My Cursor IDE"
              value={newKeyName}
              onChange={(e) => setNewKeyName(e.target.value)}
              disabled={creatingKey}
              autoFocus
            />
            <div style={{ display: "flex", gap: "0.5rem", justifyContent: "flex-end", marginTop: "1.25rem" }}>
              <Button variant="secondary" type="button" onClick={() => setShowAddModal(false)} disabled={creatingKey}>
                Cancel
              </Button>
              <Button variant="primary" type="submit" disabled={creatingKey || !newKeyName.trim()}>
                {creatingKey ? "Creating…" : "Create Key"}
              </Button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}
