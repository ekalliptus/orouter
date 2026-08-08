// Route guard: if the user is unauthenticated, redirect to /login. While the
// status probe is in flight we show a themed loading doodle.
import { Navigate } from "react-router";
import { useAuth } from "@/lib/useAuth";

export default function RequireAuth({ children }: { children: React.ReactNode }) {
  const { authed } = useAuth();
  if (authed === null) {
    return (
      <div style={{ minHeight: "100vh", display: "flex", alignItems: "center", justifyContent: "center" }}>
        <div style={{ fontSize: "3rem" }} className="animate-pulse">✏️</div>
      </div>
    );
  }
  if (!authed) return <Navigate to="/login" replace />;
  return <>{children}</>;
}
