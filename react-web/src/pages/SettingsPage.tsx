// Settings / Profile page: change password, toggle requireApiKey, view router info.
// Backed by GET/PATCH /api/settings.
import { useEffect, useState } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { useNotificationStore } from "@/store/notificationStore";

export default function SettingsPage() {
  const settings = useSettingsStore((s) => s.settings);
  const loading = useSettingsStore((s) => s.loading);
  const fetchSettings = useSettingsStore((s) => s.fetchSettings);
  const patchSettings = useSettingsStore((s) => s.patchSettings);
  const notify = useNotificationStore();

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [savingPw, setSavingPw] = useState(false);

  useEffect(() => {
    fetchSettings();
  }, [fetchSettings]);

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

  async function handleToggleApiKey(requireApiKey: boolean) {
    const updated = await patchSettings({ requireApiKey });
    if (updated) {
      notify.success(`API Key requirement ${requireApiKey ? "enabled" : "disabled"}`);
    } else {
      notify.error("Failed to update settings");
    }
  }

  return (
    <div className="fade-in" style={{ maxWidth: 640 }}>
      <h1 style={{ fontSize: "2rem", marginTop: 0 }}>⚙️ Settings</h1>

      {loading && <p style={{ fontFamily: "var(--font-body)" }}>Loading settings…</p>}

      {/* Password change card */}
      <form onSubmit={handlePasswordChange} className="kid-card kid-wobble kid-tilt" style={{ marginBottom: "1.5rem", ["--tilt" as string]: "-0.6deg" }}>
        <div style={{ fontWeight: 700, fontSize: "1.2rem", marginBottom: "0.75rem" }}>🔑 Change Dashboard Password</div>
        <div style={{ display: "grid", gap: "0.6rem" }}>
          <div>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>Current password</label>
            <input className="kid-input" type="password" placeholder="Current password..." value={currentPassword} onChange={(e) => setCurrentPassword(e.target.value)} disabled={savingPw} />
          </div>
          <div>
            <label style={{ fontFamily: "var(--font-body)", fontSize: "0.9rem" }}>New password</label>
            <input className="kid-input" type="password" placeholder="New password..." value={newPassword} onChange={(e) => setNewPassword(e.target.value)} disabled={savingPw} />
          </div>
        </div>
        <button type="submit" className="kid-btn kid-btn--primary" style={{ marginTop: "0.75rem" }} disabled={savingPw || !newPassword.trim()}>
          {savingPw ? "Saving…" : "Update Password"}
        </button>
      </form>

      {/* Router security options */}
      {settings && (
        <div className="kid-card kid-wobble kid-tilt" style={{ ["--tilt" as string]: "0.8deg" }}>
          <div style={{ fontWeight: 700, fontSize: "1.2rem", marginBottom: "0.75rem" }}>🛡️ Router Security</div>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <div>
              <strong>Require API Key for LLM Requests</strong>
              <div style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", fontSize: "0.95rem" }}>
                Fail-closed: /v1/* endpoints reject requests without a valid sk- key.
              </div>
            </div>
            <button
              className="kid-btn"
              style={{
                background: (settings.requireApiKey as boolean) ? "var(--color-success)" : "var(--color-surface)",
                color: (settings.requireApiKey as boolean) ? "#fff" : "var(--color-text-main)",
                padding: "0.4rem 0.8rem",
              }}
              onClick={() => handleToggleApiKey(!(settings.requireApiKey as boolean))}
            >
              {(settings.requireApiKey as boolean) ? "ON" : "OFF"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
