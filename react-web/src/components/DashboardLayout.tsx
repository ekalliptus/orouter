import { Outlet } from "react-router";
import Sidebar from "./Sidebar";
import Header from "./Header";
import Toasts from "./Toasts";

export default function DashboardLayout() {
  return (
    <div className="flex h-screen w-full overflow-hidden bg-bg text-text-main transition-colors duration-300">
      <Sidebar />
      <div className="flex flex-1 flex-col min-w-0 isolate relative">
        <Header />
        <main className="flex-1 p-6 lg:p-10 overflow-y-auto custom-scrollbar">
          <div className="landing-grid" aria-hidden="true" />
          <div className="max-w-7xl mx-auto">
            <Outlet />
          </div>
        </main>
      </div>
      <Toasts />
    </div>
  );
}
