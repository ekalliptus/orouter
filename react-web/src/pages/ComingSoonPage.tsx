// Generic placeholder for the nav items that exist in the old app but are out
// of M1 scope (usage, combos, cli-tools, profile, …). Keeps the sidebar links
// navigable instead of dead.
export default function ComingSoonPage({ title, emoji }: { title: string; emoji: string }) {
  return (
    <div className="fade-in" style={{ textAlign: "center", paddingTop: "3rem" }}>
      <div style={{ fontSize: "4rem" }}>{emoji}</div>
      <h1 style={{ fontSize: "2.25rem" }}>{title}</h1>
      <div className="kid-card kid-wobble kid-tilt" style={{ display: "inline-block", marginTop: "1rem", ["--tilt" as string]: "-1.2deg" }}>
        <span style={{ fontFamily: "var(--font-body)", fontSize: "1.2rem" }}>🎨 Still being drawn… coming in a later milestone!</span>
      </div>
    </div>
  );
}
