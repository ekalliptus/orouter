// Login page. POSTs to the Rust backend's /api/auth/login, which sets the
// httpOnly `auth_token` JWT cookie. On success we navigate to the dashboard.
// Mirrors the shape of the old Next.js login (src/app/api/auth/login/route.js):
// password verified server-side, fail-limiter enforced server-side.
import { useState } from "react";
import { useNavigate } from "react-router";
import { useNotificationStore } from "@/store/notificationStore";
import { fetchAuthStatus } from "@/lib/useAuth";
import CrayonFilter from "@/components/CrayonFilter";

export default function LoginPage() {
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();
  const notify = useNotificationStore((s) => s.success);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      const res = await fetch("/api/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ password }),
      });
      const data = (await res.json()) as { success?: boolean; error?: string; mustChangePassword?: boolean };
      if (!res.ok || !data.success) {
        setError(data.error ?? "Login failed");
        return;
      }
      notify("Welcome to your crayon router!", "Let's draw");
      // Refresh the cached auth flag BEFORE navigating: RequireAuth reads the
      // module-level cache, which still says false from the pre-login probe.
      await fetchAuthStatus(true);
      navigate("/dashboard");
    } catch {
      setError("Could not reach the backend. Is the Rust server running?");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div style={{ minHeight: "100vh", display: "flex", alignItems: "center", justifyContent: "center", padding: "1.5rem" }}>
      <CrayonFilter />
      <form
        onSubmit={handleSubmit}
        className="kid-card kid-wobble-strong kid-tilt fade-in"
        style={{ width: "min(440px, 100%)", ["--tilt" as string]: "-1deg", background: "var(--color-surface)" }}
      >
        <div style={{ textAlign: "center", marginBottom: "1.5rem" }}>
          <div style={{ fontSize: "3rem" }}>🖍️</div>
          <h1 style={{ fontSize: "2rem", margin: "0.25rem 0" }}>ORouter</h1>
          <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", margin: 0 }}>
            Draw your way to every AI
          </p>
        </div>

        {error && (
          <div className="kid-card kid-wobble" style={{ background: "var(--color-danger)", color: "#fff", padding: "0.7rem 0.9rem", marginBottom: "1rem", boxShadow: "var(--nb-shadow-sm)" }}>
            {error}
          </div>
        )}

        <label htmlFor="password" style={{ display: "block", fontFamily: "var(--font-body)", fontSize: "1.1rem", marginBottom: "0.4rem" }}>
          Secret password
        </label>
        <input
          id="password"
          type="password"
          className="kid-input kid-wobble"
          placeholder="type your secret..."
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
          disabled={loading}
        />

        <button type="submit" className="kid-btn kid-btn--primary kid-wobble" style={{ width: "100%", marginTop: "1.25rem", fontSize: "1.2rem" }} disabled={loading || !password}>
          {loading ? "Drawing..." : "Open my drawing 🎨"}
        </button>

        <p style={{ fontFamily: "var(--font-body)", fontSize: "0.95rem", color: "var(--color-text-subtle)", textAlign: "center", marginTop: "1rem" }}>
          Default password is <code>123456</code> until you set one
        </p>
      </form>
    </div>
  );
}
