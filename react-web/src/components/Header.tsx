import { useLocation } from "react-router";
import { useThemeStore } from "@/store/themeStore";

const getPageInfo = (pathname: string) => {
  if (!pathname) return { title: "Dashboard", description: "", breadcrumbs: [] };

  if (pathname.includes("/providers"))
    return {
      title: "Providers",
      description: "Manage your AI provider connections",
      icon: "dns",
      breadcrumbs: [],
    };
  if (pathname.includes("/combos"))
    return {
      title: "Combos",
      description: "Model combos with fallback",
      icon: "layers",
      breadcrumbs: [],
    };
  if (pathname.includes("/usage"))
    return {
      title: "Usage & Analytics",
      description: "Monitor your API usage, token consumption, and request logs",
      icon: "bar_chart",
      breadcrumbs: [],
    };
  if (pathname.includes("/cli-tools"))
    return {
      title: "CLI Tools",
      description: "Configure CLI tools",
      icon: "terminal",
      breadcrumbs: [],
    };
  if (pathname.includes("/profile"))
    return {
      title: "Settings",
      description: "Manage your preferences",
      icon: "settings",
      breadcrumbs: [],
    };
  return {
    title: "Endpoint & Key",
    description: "API endpoint configuration and keys",
    icon: "api",
    breadcrumbs: [],
  };
};

export default function Header() {
  const location = useLocation();
  const pathname = location.pathname;
  const pageInfo = getPageInfo(pathname);

  const theme = useThemeStore((s) => s.theme);
  const toggleTheme = useThemeStore((s) => s.toggleTheme);

  return (
    <header className="sticky top-0 z-30 flex items-center justify-between px-4 lg:px-8 pt-3 pb-2 border-b border-border-subtle bg-bg/80 backdrop-blur-sm transition-colors">
      <div className="flex items-center gap-3">
        {pageInfo.icon && (
          <span className="material-symbols-outlined text-primary text-xl lg:text-2xl">
            {pageInfo.icon}
          </span>
        )}
        <div className="flex flex-col">
          <h1 className="text-base lg:text-2xl font-semibold tracking-tight text-text-main">
            {pageInfo.title}
          </h1>
          {pageInfo.description && (
            <p className="hidden lg:block text-sm text-text-muted">{pageInfo.description}</p>
          )}
        </div>
      </div>

      <div className="flex items-center gap-2">
        <button
          onClick={toggleTheme}
          aria-label="Toggle theme"
          className="flex h-10 w-10 items-center justify-center rounded-full text-text-muted hover:bg-surface-2 hover:text-text-main transition-colors cursor-pointer"
        >
          <span className="material-symbols-outlined text-[22px]">
            {theme === "dark" ? "light_mode" : "dark_mode"}
          </span>
        </button>
      </div>
    </header>
  );
}
