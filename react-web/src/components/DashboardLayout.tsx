import { Outlet } from "react-router";
import Sidebar from "./Sidebar";
import Header from "./Header";
import Toasts from "./Toasts";

export default function DashboardLayout() {
  return (
    <div className="flex min-h-screen bg-bg text-text-main transition-colors duration-300">
      <Sidebar />
      <div className="flex flex-1 flex-col min-w-0">
        <Header />
        <main className="flex-1 p-6 overflow-y-auto">
          <Outlet />
        </main>
      </div>
      <Toasts />
    </div>
  );
}
