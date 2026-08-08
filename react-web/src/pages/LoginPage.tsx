// M1 login page. Real auth lands in M3 (JWT cookie from the Rust backend);
// for now this just demonstrates the theme and navigates into the dashboard.
// The form is intentionally huge and friendly — kid style.
import { useState } from "react";
import { useNavigate } from "react-router";
import { useNotificationStore } from "@/store/notificationStore";
import CrayonFilter from "@/components/CrayonFilter";

export default function LoginPage() {
  const [password, setPassword] = useState("");
  const navigate = useNavigate();
  const notify = useNotificationStore((s) => s.success);

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    // M3 will POST /api/auth/login here and set the httpOnly JWT cookie.
    notify("Welcome to your crayon router!", "Let's draw");
    navigate("/dashboard");
  }

  return (
    <div
      style={{
        minHeight: "100vh",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: "1.5rem",
      }}
    >
      <CrayonFilter />
      <form
        onSubmit={handleSubmit}
        className="kid-card kid-wobble-strong kid-tilt fade-in"
        style={{ width: "min(440px, 100%", ["--tilt" as string]: "-1deg", background: "var(--color-surface)" }}
      >
        <div style={{ textAlign: "center", marginBottom: "1.5rem" }}>
          <div style={{ fontSize: "3rem" }}>🖍️</div>
          <h1 style={{ fontSize: "2rem", margin: "0.25rem 0" }}>ORouter</h1>
          <p style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)", margin: 0 }}>
            Draw your way to every AI
          </p>
        </div>

        <label
          htmlFor="password"
          style={{ display: "block", fontFamily: "var(--font-body)", fontSize: "1.1rem", marginBottom: "0.4rem" }}
        >
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
        />

        <button type="submit" className="kid-btn kid-btn--primary kid-wobble" style={{ width: "100%", marginTop: "1.25rem", fontSize: "1.2rem" }}>
          Open my drawing 🎨
        </button>

        <p style={{ fontFamily: "var(--font-body)", fontSize: "0.95rem", color: "var(--color-text-subtle)", textAlign: "center", marginTop: "1rem" }}>
          (no password needed yet — this is a preview)
        </p>
      </form>
    </div>
  );
}
