// Left navigation. The nav items mirror the old app's Sidebar.js canonical
// set (lines 22–41), but for M1 most point at "coming soon" placeholders —
// only Endpoint/Keys and Providers are real routes. Theme + locale toggle live
// in the Header, not here. Styled as a kid's notebook tab strip.
import { NavLink } from "react-router";

interface NavItem {
  to: string;
  label: string;
  emoji: string;
  soon?: boolean;
}

const NAV: NavItem[] = [
  { to: "/dashboard", label: "Endpoint", emoji: "🔌" },
  { to: "/dashboard/keys", label: "API Keys", emoji: "🔑" },
  { to: "/dashboard/providers", label: "Providers", emoji: "🤖" },
  { to: "/dashboard/usage", label: "Usage", emoji: "📊" },
  { to: "/dashboard/combos", label: "Combos", emoji: "🧩", soon: true },
  { to: "/dashboard/cli-tools", label: "CLI Tools", emoji: "🛠️", soon: true },
  { to: "/dashboard/profile", label: "Settings", emoji: "⚙️", soon: true },
];

export default function Sidebar() {
  return (
    <aside
      className="kid-wobble"
      style={{
        width: 232,
        flexShrink: 0,
        background: "var(--color-sidebar)",
        borderRight: "3px solid var(--nb-border)",
        padding: "1rem 0.75rem",
        minHeight: "100vh",
      }}
    >
      <div className="kid-tilt" style={{ marginBottom: "1.5rem", padding: "0 0.5rem" }}>
        <div style={{ fontSize: "1.6rem", fontWeight: 700, lineHeight: 1 }}>
          🖍️ ORouter
        </div>
        <div style={{ fontFamily: "var(--font-body)", fontSize: "0.95rem", color: "var(--color-text-muted)" }}>
          crayon edition
        </div>
      </div>

      <nav className="flex flex-col gap-2">
        {NAV.map((item) =>
          item.soon ? (
            <div
              key={item.to}
              className="kid-tilt"
              style={{
                padding: "0.6rem 0.8rem",
                opacity: 0.5,
                display: "flex",
                alignItems: "center",
                gap: "0.5rem",
                fontFamily: "var(--font-body)",
                fontSize: "1.05rem",
              }}
              title="Coming soon"
            >
              <span>{item.emoji}</span>
              <span>{item.label}</span>
            </div>
          ) : (
            <NavLink
              key={item.to}
              to={item.to}
              className="kid-tilt"
              style={({ isActive }) => ({
                display: "flex",
                alignItems: "center",
                gap: "0.5rem",
                padding: "0.6rem 0.8rem",
                fontFamily: "var(--font-body)",
                fontSize: "1.05rem",
                background: isActive ? "var(--color-accent)" : "var(--color-surface)",
                border: "3px solid var(--nb-border)",
                boxShadow: "var(--nb-shadow-sm)",
                color: "var(--color-text-main)",
                textDecoration: "none",
              })}
            >
              <span>{item.emoji}</span>
              <span>{item.label}</span>
            </NavLink>
          )
        )}
      </nav>
    </aside>
  );
}
