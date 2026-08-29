// Settings / Profile page — parity sections from the 9Router profile page,
// crayon edition: Security, Routing Strategy, Token Savers, Network,
// Observability, plus engine info (version) + shutdown.
// Everything persists through GET/PATCH /api/settings; toggles save instantly,
// text/number sections save via their Save button.
import { useEffect, useState } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { useNotificationStore } from "@/store/notificationStore";
import { Toggle } from "@/components/ui";

interface VersionInfo {
  version: string;
  engine: string;
  uptimeSecs: number;
}

function fmtUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="kid-card kid-wobble kid-tilt" style={{ marginBottom: "1.5rem" }}>
      <div style={{ fontWeight: 700, fontSize: "1.2rem", marginBottom: "0.9rem" }}>{title}</div>
      <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>{children}</div>
    </div>
  );
}

function ToggleRow({ label, hint, checked, onChange }: { label: string; hint?: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "1rem" }}>
      <div>
        <strong style={{ fontSize: "1.02rem" }}>{label}</strong>
        {hint && (
          <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", fontSize: "0.9rem" }}>{hint}</div>
        )}
      </div>
      <Toggle checked={checked} onChange={onChange} />
    </div>
  );
}

export default function SettingsPage() {
  const settings = useSettingsStore((s) => s.settings) as Record<string, unknown> | null;
  const loading = useSettingsStore((s) => s.loading);
  const fetchSettings = useSettingsStore((s) => s.fetchSettings);
  const patchSettings = useSettingsStore((s) => s.patchSettings);
  const notify = useNotificationStore();

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [savingPw, setSavingPw] = useState(false);
  const [version, setVersion] = useState<VersionInfo | null>(null);

  // Buffered text fields (saved via button).
  const [routingLimit, setRoutingLimit] = useState("");
  const [comboLimit, setComboLimit] = useState("");
  const [headroomUrl, setHeadroomUrl] = useState("");
  const [proxyUrl, setProxyUrl] = useState("");
  const [noProxy, setNoProxy] = useState("");
  const [retentionDays, setRetentionDays] = useState("");

  useEffect(() => {
    fetchSettings();
    fetch("/api/version", { credentials: "include" })
      .then((r) => (r.ok ? r.json() : null))
      .then((v: VersionInfo | null) => v && setVersion(v))
      .catch(() => {});
  }, [fetchSettings]);

  useEffect(() => {
    if (!settings) return;
    setRoutingLimit(String(settings.stickyRoundRobinLimit ?? 3));
    setComboLimit(String(settings.comboStickyRoundRobinLimit ?? 1));
    setHeadroomUrl(String(settings.headroomUrl ?? ""));
    setProxyUrl(String(settings.outboundProxyUrl ?? ""));
    setNoProxy(String(settings.outboundNoProxy ?? ""));
    setRetentionDays(String(settings.usageHistoryRetentionDays ?? 30));
  }, [settings]);

  function str(key: string): string {
    return String((settings as Record<string, unknown>)?.[key] ?? "");
  }
  function bool(key: string): boolean {
    return !!((settings as Record<string, unknown>)?.[key]);
  }

  async function patch(patchObj: Record<string, unknown>, okMsg: string) {
    const updated = await patchSettings(patchObj);
    if (updated) notify.success(okMsg);
    else notify.error("Failed to save setting");
    return updated;
  }

  async function handlePasswordChange(e: React.FormEvent) {
    e.preventDefault();
    if (!newPassword.trim()) return;
    setSavingPw(true);
    const updated = await patchSettings({ currentPassword, newPassword });
    setSavingPw(false);
    if (updated) {
      notify.success("Password updated successfully!");
      setCurrentPassword("");
      setNewPassword("");
    } else {
      notify.error("Failed to update password. Check current password.");
    }
  }

  async function handleShutdown() {
    if (!confirm("Shut down the ORouter backend now?")) return;
    try {
      await fetch("/api/version/shutdown", { method: "POST", credentials: "include" });
      notify.success("Shutdown requested — the server is stopping.");
    } catch {
      notify.success("Shutdown requested — the server is stopping.");
    }
  }

  return (
    <div className="fade-in" style={{ maxWidth: 720, display: "flex", flexDirection: "column" }}>
      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading settings…</p>}

      {/* Engine info */}
      <Section title="🚂 Engine">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "0.8rem" }}>
          <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", fontSize: "0.95rem" }}>
            <div>ORouter backend <strong style={{ color: "var(--color-text-main)" }}>v{version?.version ?? "…"}</strong> · engine: <strong style={{ color: "var(--color-text-main)" }}>{version?.engine ?? "rust"}</strong></div>
            <div>Uptime: {version ? fmtUptime(version.uptimeSecs) : "…"}</div>
          </div>
          <button className="kid-btn" style={{ background: "var(--color-danger)", color: "#fff" }} onClick={handleShutdown}>
            ⏻ Shut down server
          </button>
        </div>
      </Section>

      {/* Password */}
      <Section title="🔑 Change Dashboard Password">
        <form onSubmit={handlePasswordChange} style={{ display: "grid", gap: "0.6rem" }}>
          <div>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Current password</label>
            <input className="kid-input" type="password" placeholder="Current password..." value={currentPassword} onChange={(e) => setCurrentPassword(e.target.value)} disabled={savingPw} />
          </div>
          <div>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>New password</label>
            <input className="kid-input" type="password" placeholder="New password..." value={newPassword} onChange={(e) => setNewPassword(e.target.value)} disabled={savingPw} />
          </div>
          <button type="submit" className="kid-btn kid-btn--primary" style={{ justifySelf: "start" }} disabled={savingPw || !newPassword.trim()}>
            {savingPw ? "Saving…" : "Update Password"}
          </button>
        </form>
      </Section>

      {/* Security */}
      {settings && (
        <Section title="🛡️ Security">
          <ToggleRow
            label="Require API Key for LLM Requests"
            hint="Fail-closed: /v1/* endpoints reject requests without a valid sk- key."
            checked={bool("requireApiKey")}
            onChange={(v) => patch({ requireApiKey: v }, "requireApiKey updated")}
          />
          <ToggleRow
            label="Require Dashboard Login"
            hint="Non-loopback clients must sign in with a session cookie."
            checked={bool("requireLogin")}
            onChange={(v) => patch({ requireLogin: v }, "requireLogin updated")}
          />
          <ToggleRow
            label="Allow Dashboard via Tunnel"
            hint="When a tunnel is active, require login for dashboard access through it."
            checked={bool("tunnelDashboardAccess")}
            onChange={(v) => patch({ tunnelDashboardAccess: v }, "tunnelDashboardAccess updated")}
          />
        </Section>
      )}

      {/* Routing */}
      {settings && (
        <Section title="🧭 Routing Strategy">
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))" }}>
            <div>
              <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Combo strategy</label>
              <select
                className="kid-input"
                value={str("comboStrategy") || "fallback"}
                onChange={(e) => patch({ comboStrategy: e.target.value }, "comboStrategy updated")}
              >
                <option value="fallback">fallback</option>
                <option value="round-robin">round-robin</option>
                <option value="sticky-round-robin">sticky-round-robin</option>
              </select>
            </div>
            <div>
              <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Sticky RR limit (per account)</label>
              <input className="kid-input" type="number" min={1} value={routingLimit} onChange={(e) => setRoutingLimit(e.target.value)} />
            </div>
            <div>
              <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Sticky RR limit (combo)</label>
              <input className="kid-input" type="number" min={1} value={comboLimit} onChange={(e) => setComboLimit(e.target.value)} />
            </div>
          </div>
          <button
            className="kid-btn kid-btn--primary"
            style={{ justifySelf: "start" }}
            onClick={() => {
              const rl = parseInt(routingLimit, 10);
              const cl = parseInt(comboLimit, 10);
              const p: Record<string, unknown> = {};
              if (!Number.isNaN(rl)) p.stickyRoundRobinLimit = rl;
              if (!Number.isNaN(cl)) p.comboStickyRoundRobinLimit = cl;
              if (Object.keys(p).length) patch(p, "Routing limits saved");
            }}
          >
            Save limits
          </button>
          <div style={{ fontFamily: "var(--font-body)", fontSize: "0.85rem", color: "var(--color-text-muted)" }}>
            Stored globally; hybrid mode forwards them to the Node routing engine.
          </div>
        </Section>
      )}

      {/* Token savers */}
      {settings && (
        <Section title="🪄 Token Savers">
          <ToggleRow
            label="RTK compression"
            hint="Compress tool_result content in-flight to cut prompt tokens."
            checked={bool("rtkEnabled")}
            onChange={(v) => patch({ rtkEnabled: v }, "rtkEnabled updated")}
          />
          <ToggleRow
            label="Caveman mode"
            hint="Rewrite verbose content into terse caveman speak."
            checked={bool("cavemanEnabled")}
            onChange={(v) => patch({ cavemanEnabled: v }, "cavemanEnabled updated")}
          />
          {bool("cavemanEnabled") && (
            <div>
              <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Caveman level</label>
              <select className="kid-input" value={str("cavemanLevel") || "full"} onChange={(e) => patch({ cavemanLevel: e.target.value }, "cavemanLevel updated")}>
                <option value="lite">lite</option>
                <option value="full">full</option>
              </select>
            </div>
          )}
          <ToggleRow
            label="Ponytail mode"
            hint="Additional summarization pass for long tool outputs."
            checked={bool("ponytailEnabled")}
            onChange={(v) => patch({ ponytailEnabled: v }, "ponytailEnabled updated")}
          />
          <ToggleRow
            label="Headroom integration"
            hint="Route through a local headroom proxy sidecar for context compression."
            checked={bool("headroomEnabled")}
            onChange={(v) => patch({ headroomEnabled: v }, "headroomEnabled updated")}
          />
          {bool("headroomEnabled") && (
            <div style={{ display: "grid", gap: "0.6rem" }}>
              <div>
                <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Headroom URL</label>
                <input className="kid-input" value={headroomUrl} onChange={(e) => setHeadroomUrl(e.target.value)} placeholder="http://localhost:8787" />
              </div>
              <ToggleRow
                label="Compress user messages"
                checked={bool("headroomCompressUserMessages")}
                onChange={(v) => patch({ headroomCompressUserMessages: v }, "headroomCompressUserMessages updated")}
              />
              <button
                className="kid-btn"
                style={{ justifySelf: "start" }}
                onClick={() => patch({ headroomUrl: headroomUrl.trim() }, "Headroom URL saved")}
              >
                Save headroom URL
              </button>
            </div>
          )}
          <ToggleRow
            label="PXPIPE"
            hint="Big-prompt preprocessing pipeline (25k+ chars)."
            checked={bool("pxpipeEnabled")}
            onChange={(v) => patch({ pxpipeEnabled: v }, "pxpipeEnabled updated")}
          />
        </Section>
      )}

      {/* Network */}
      {settings && (
        <Section title="🌐 Network">
          <ToggleRow
            label="Outbound proxy"
            hint="Route all upstream provider calls through a proxy."
            checked={bool("outboundProxyEnabled")}
            onChange={(v) => patch({ outboundProxyEnabled: v }, "outboundProxyEnabled updated")}
          />
          <div style={{ display: "grid", gap: "0.6rem" }}>
            <div>
              <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Proxy URL</label>
              <input className="kid-input" value={proxyUrl} onChange={(e) => setProxyUrl(e.target.value)} placeholder="http://127.0.0.1:7890" />
            </div>
            <div>
              <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>No-proxy hosts (comma separated)</label>
              <input className="kid-input" value={noProxy} onChange={(e) => setNoProxy(e.target.value)} placeholder="localhost,127.0.0.1" />
            </div>
            <button
              className="kid-btn"
              style={{ justifySelf: "start" }}
              onClick={() => patch({ outboundProxyUrl: proxyUrl.trim(), outboundNoProxy: noProxy.trim() }, "Proxy settings saved")}
            >
              Save proxy settings
            </button>
          </div>
          <div>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>MITM router base URL</label>
            <input className="kid-input" value={str("mitmRouterBaseUrl")} onChange={(e) => patch({ mitmRouterBaseUrl: e.target.value }, "mitmRouterBaseUrl updated")} placeholder="http://localhost:20128" />
          </div>
        </Section>
      )}

      {/* Observability & retention */}
      {settings && (
        <Section title="🔭 Observability & Retention">
          <ToggleRow
            label="Enable observability"
            hint="Collect request records for the logs view."
            checked={bool("enableObservability")}
            onChange={(v) => patch({ enableObservability: v }, "enableObservability updated")}
          />
          <div style={{ display: "grid", gap: "0.6rem", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))" }}>
            <div>
              <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Max records</label>
              <input className="kid-input" type="number" value={str("observabilityMaxRecords")} onChange={(e) => patch({ observabilityMaxRecords: parseInt(e.target.value, 10) || 0 }, "observabilityMaxRecords updated")} />
            </div>
            <div>
              <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Usage history retention (days)</label>
              <input className="kid-input" type="number" value={retentionDays} onChange={(e) => setRetentionDays(e.target.value)} />
            </div>
          </div>
          <button
            className="kid-btn"
            style={{ justifySelf: "start" }}
            onClick={() => {
              const d = parseInt(retentionDays, 10);
              if (!Number.isNaN(d)) patch({ usageHistoryRetentionDays: d }, "Retention saved");
            }}
          >
            Save retention
          </button>
        </Section>
      )}
    </div>
  );
}
