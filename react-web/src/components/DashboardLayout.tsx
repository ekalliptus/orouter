// Shell for authenticated pages: Sidebar + Header + content outlet + Toasts.
// Mirrors the old app's DashboardLayout.js role (Sidebar/Header/toast container
// wrapping the route group), but in react-router an <Outlet/> fills the page
// slot instead of Next.js children.
import { Outlet } from "react-router";
import Sidebar from "./Sidebar";
import Header from "./Header";
import Toasts from "./Toasts";

export default function DashboardLayout() {
  return (
    <div style={{ display: "flex", minHeight: "100vh" }}>
      <Sidebar />
      <div style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0 }}>
        <Header />
        <main style={{ flex: 1, padding: "1.5rem" }}>
          <Outlet />
        </main>
      </div>
      <Toasts />
    </div>
  );
}
