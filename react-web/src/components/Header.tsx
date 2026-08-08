// Top bar: app title, theme toggle, a locale hint. The header search input
// (useHeaderSearchStore) is wired but hidden until a page registers it.
// Kept minimal for M1; M3 adds the real user menu.
import { useThemeStore } from "@/store/themeStore";

export default function Header() {
  const theme = useThemeStore((s) => s.theme);
  const toggleTheme = useThemeStore((s) => s.toggleTheme);

  return (
    <header
      className="kid-wobble"
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: "1rem",
        padding: "0.75rem 1.25rem",
        background: "var(--color-surface)",
        borderBottom: "3px solid var(--nb-border)",
        boxShadow: "var(--nb-shadow-sm)",
      }}
    >
      <div style={{ fontSize: "1.25rem", fontWeight: 700 }}>
        ✏️ My Router Drawing
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: "0.6rem" }}>
        <span style={{ fontFamily: "var(--font-body)", color: "var(--color-text-muted)" }}>
          {theme === "dark" ? "night drawing" : "day drawing"}
        </span>
        <button
          className="kid-btn kid-btn--accent"
          onClick={toggleTheme}
          aria-label="Toggle theme"
          style={{ padding: "0.4rem 0.7rem", fontSize: "1.1rem" }}
        >
          {theme === "dark" ? "☀️" : "🌙"}
        </button>
      </div>
    </header>
  );
}
