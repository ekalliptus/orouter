import { Link, useLocation } from "react-router";
import { cn } from "@/shared/utils/cn";
import Logo from "./Logo";

interface SidebarProps {
  onClose?: () => void;
}

const navItems = [
  { href: "/dashboard", label: "Endpoint & Key", icon: "api" },
  { href: "/dashboard/providers", label: "Providers", icon: "dns" },
  { href: "/dashboard/combos", label: "Combos", icon: "layers" },
  { href: "/dashboard/usage", label: "Usage", icon: "bar_chart" },
  { href: "/dashboard/cli-tools", label: "CLI Tools", icon: "terminal" },
];

const systemItems = [
  { href: "/dashboard/profile", label: "Settings", icon: "settings" },
];

export default function Sidebar({ onClose }: SidebarProps) {
  const location = useLocation();
  const pathname = location.pathname;

  const isActive = (href: string) => {
    if (href === "/dashboard") {
      return pathname === "/dashboard" || pathname === "/dashboard/";
    }
    return pathname.startsWith(href);
  };

  return (
    <aside className="aurora-glass-strong flex min-h-full w-64 flex-col border-r border-border bg-sidebar transition-colors duration-300">
      {/* Traffic lights */}
      <div className="flex items-center gap-2 px-6 pt-5 pb-2">
        <div className="w-3 h-3 rounded-full bg-[#FF5F56]" />
        <div className="w-3 h-3 rounded-full bg-[#FFBD2E]" />
        <div className="w-3 h-3 rounded-full bg-[#27C93F]" />
      </div>

      {/* Logo */}
      <div className="px-6 py-4 flex flex-col gap-2">
        <Link to="/dashboard" onClick={onClose} className="flex items-center gap-3">
          <Logo size={36} />
          <div className="flex flex-col">
            <h1 className="text-lg font-extrabold tracking-tight text-text-main uppercase">ORouter</h1>
            <span className="console-label text-emerald-600 dark:text-emerald-400">Control Plane · Online</span>
          </div>
        </Link>
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-4 py-2 space-y-1 overflow-y-auto custom-scrollbar">
        {navItems.map((item) => (
          <Link
            key={item.href}
            to={item.href}
            onClick={onClose}
            className={cn(
              "group relative flex items-center gap-3 rounded-lg border border-transparent px-3 py-2 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/55",
              isActive(item.href)
                ? "border-brand-500/20 bg-brand-500/10 text-brand-600 before:absolute before:inset-y-2 before:left-0 before:w-0.5 before:rounded-full before:bg-brand-500 dark:text-brand-300"
                : "text-text-muted hover:bg-surface-2/70 hover:text-text-main"
            )}
          >
            <span
              className={cn(
                "material-symbols-outlined text-[20px]",
                isActive(item.href) ? "fill-1" : "group-hover:text-primary transition-colors"
              )}
            >
              {item.icon}
            </span>
            <span className="text-[14px] font-medium">{item.label}</span>
          </Link>
        ))}

        {/* System section */}
        <div className="pt-4 mt-2 space-y-1 border-t border-border-subtle">
          <p className="console-label mb-2 px-3 text-xs uppercase tracking-wider text-text-muted font-mono">
            System
          </p>

          {systemItems.map((item) => (
            <Link
              key={item.href}
              to={item.href}
              onClick={onClose}
              className={cn(
                "group relative flex items-center gap-3 rounded-lg border border-transparent px-3 py-2 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/55",
                isActive(item.href)
                  ? "border-brand-500/20 bg-brand-500/10 text-brand-600 before:absolute before:inset-y-2 before:left-0 before:w-0.5 before:rounded-full before:bg-brand-500 dark:text-brand-300"
                  : "text-text-muted hover:bg-surface-2/70 hover:text-text-main"
              )}
            >
              <span
                className={cn(
                  "material-symbols-outlined text-[20px]",
                  isActive(item.href) ? "fill-1" : "group-hover:text-primary transition-colors"
                )}
              >
                {item.icon}
              </span>
              <span className="text-[14px] font-medium">{item.label}</span>
            </Link>
          ))}
        </div>
      </nav>
    </aside>
  );
}
