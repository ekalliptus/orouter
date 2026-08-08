// App root: routes + one-time bootstrap (theme init, i18n init, locale reload
// on navigation). react-router v7 <Routes>/<Route>. The dashboard shell is
// wrapped by DashboardLayout; the login page stands alone.
import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate, useLocation } from "react-router";
import { useThemeStore } from "@/store/themeStore";
import { initRuntimeI18n, reloadTranslations } from "@/i18n/runtime";
import DashboardLayout from "@/components/DashboardLayout";
import RequireAuth from "@/components/RequireAuth";
import LoginPage from "@/pages/LoginPage";
import EndpointPage from "@/pages/EndpointPage";
import ProvidersPage from "@/pages/ProvidersPage";
import KeysPage from "@/pages/KeysPage";
import ComingSoonPage from "@/pages/ComingSoonPage";

function LocaleReloader() {
  // Mirrors RuntimeI18nProvider: re-read the locale cookie + retranslate the
  // DOM whenever the route changes. usePathname() → useLocation().pathname.
  const { pathname } = useLocation();
  useEffect(() => {
    void reloadTranslations();
  }, [pathname]);
  return null;
}

export default function App() {
  const initTheme = useThemeStore((s) => s.initTheme);

  useEffect(() => {
    initTheme();
    void initRuntimeI18n();
  }, [initTheme]);

  return (
    <BrowserRouter>
      <LocaleReloader />
      <Routes>
        <Route path="/" element={<Navigate to="/dashboard" replace />} />
        <Route path="/login" element={<LoginPage />} />
        <Route path="/dashboard" element={<RequireAuth><DashboardLayout /></RequireAuth>}>
          <Route index element={<EndpointPage />} />
          <Route path="providers" element={<ProvidersPage />} />
          <Route path="keys" element={<KeysPage />} />
          <Route path="usage" element={<ComingSoonPage title="Usage" emoji="📊" />} />
          <Route path="combos" element={<ComingSoonPage title="Combos" emoji="🧩" />} />
          <Route path="cli-tools" element={<ComingSoonPage title="CLI Tools" emoji="🛠️" />} />
          <Route path="profile" element={<ComingSoonPage title="Settings" emoji="⚙️" />} />
        </Route>
        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
