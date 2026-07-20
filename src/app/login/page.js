"use client";

import { useState, useEffect } from "react";
import { Card, Button, Input } from "@/shared/components";

export default function LoginPage() {
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [resetHint, setResetHint] = useState("");
  const [retryAfter, setRetryAfter] = useState(0);
  const [loading, setLoading] = useState(false);
  const [hasPassword, setHasPassword] = useState(null);
  const [authMode, setAuthMode] = useState("password");
  const [oidcConfigured, setOidcConfigured] = useState(false);
  const [oidcLoginLabel, setOidcLoginLabel] = useState("Sign in with OIDC");
  const [mustChange, setMustChange] = useState(false);
  const [newPassword, setNewPassword] = useState("");

  // Countdown for rate-limit
  useEffect(() => {
    if (retryAfter <= 0) return;
    const id = setInterval(() => setRetryAfter((s) => (s > 0 ? s - 1 : 0)), 1000);
    return () => clearInterval(id);
  }, [retryAfter]);

  useEffect(() => {
    async function checkAuth() {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 5000);
      const baseUrl = typeof window !== "undefined" ? window.location.origin : "";

      try {
        const res = await fetch(`${baseUrl}/api/auth/status`, {
          signal: controller.signal,
        });
        clearTimeout(timeoutId);

        if (res.ok) {
          const data = await res.json();
          if (data.requireLogin === false) {
            window.location.assign("/dashboard");
            return;
          }
          setHasPassword(!!data.hasPassword);
          setAuthMode(data.authMode || "password");
          setOidcConfigured(data.oidcConfigured === true);
          setOidcLoginLabel(data.oidcLoginLabel || "Sign in with OIDC");
        } else {
          // Safe fallback on non-OK response to avoid infinite loading state.
          setHasPassword(true);
        }
      } catch (err) {
        clearTimeout(timeoutId);
        setHasPassword(true);
      }
    }
    checkAuth();
  }, []);

  const handleLogin = async (e) => {
    e.preventDefault();
    setLoading(true);
    setError("");
    setResetHint("");

    try {
      const res = await fetch("/api/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ password }),
      });

      if (res.ok) {
        const data = await res.json();
        if (data.mustChangePassword) {
          setMustChange(true);
          return;
        }
        window.location.assign("/dashboard");
      } else {
        const data = await res.json();
        setError(data.error || "Invalid password");
        if (data.resetHint) setResetHint(data.resetHint);
        if (data.retryAfter) setRetryAfter(Number(data.retryAfter));
      }
    } catch (err) {
      setError("An error occurred. Please try again.");
    } finally {
      setLoading(false);
    }
  };

  // Force a new password before entering the dashboard (default + remote).
  const handleSetNewPassword = async (e) => {
    e.preventDefault();
    setLoading(true);
    setError("");
    try {
      const res = await fetch("/api/settings", {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ currentPassword: password, newPassword }),
      });
      if (res.ok) {
        window.location.assign("/dashboard");
      } else {
        const data = await res.json();
        setError(data.error || "Failed to set password");
      }
    } catch (err) {
      setError("An error occurred. Please try again.");
    } finally {
      setLoading(false);
    }
  };

  const handleOidcLogin = () => {
    window.location.href = "/api/auth/oidc/start";
  };

  const oidcAvailable = oidcConfigured && ["oidc", "both"].includes(authMode);
  const passwordAvailable = authMode !== "oidc" || !oidcConfigured;

  // Show loading state while checking password
  if (hasPassword === null) {
    return (
      <div className="relative flex min-h-screen items-center justify-center overflow-hidden bg-bg p-4">
        <div className="landing-grid pointer-events-none absolute inset-0" aria-hidden="true" />
        <div className="aurora-orb absolute -left-24 -top-24 size-[28rem] bg-emerald-400" aria-hidden="true" />
        <div className="aurora-orb absolute -bottom-28 -right-20 size-[30rem] bg-brand-500" aria-hidden="true" />
        <div className="aurora-glass-strong relative rounded-[22px] px-12 py-10 text-center">
          <div className="mx-auto size-8 animate-spin rounded-full border-2 border-brand-500/25 border-b-brand-500" />
          <p className="console-label mt-4">Initializing control plane</p>
        </div>
      </div>
    );
  }

  return (
    <div className="relative min-h-screen overflow-hidden bg-bg p-4 sm:p-6 lg:p-8">
      <div className="landing-grid pointer-events-none absolute inset-0" aria-hidden="true" />
      <div className="aurora-orb absolute -left-24 -top-24 size-[28rem] bg-emerald-400" aria-hidden="true" />
      <div className="aurora-orb absolute -bottom-28 -right-20 size-[30rem] bg-brand-500" aria-hidden="true" />
      <main className="relative z-10 mx-auto grid min-h-[calc(100vh-2rem)] max-w-6xl overflow-hidden rounded-[24px] border border-border-subtle bg-bg/45 shadow-[var(--shadow-glass)] lg:min-h-[calc(100vh-4rem)] lg:grid-cols-[1.1fr_0.9fr]">
        <section className="hidden border-r border-border-subtle bg-sidebar/45 p-10 lg:flex lg:flex-col lg:justify-center xl:p-14">
          <p className="console-label mb-5">Control Plane</p>
          <div className="mb-8 flex items-center gap-4">
            <div className="flex size-12 items-center justify-center rounded-[14px] bg-gradient-to-br from-brand-400 to-brand-600 shadow-[var(--shadow-warm)]">
              <span className="material-symbols-outlined text-2xl text-white">hub</span>
            </div>
            <div>
              <h1 className="text-3xl font-semibold tracking-tight text-text-main">ORouter</h1>
              <p className="console-label text-emerald-600 dark:text-emerald-400">Secure Gateway</p>
            </div>
          </div>
          <h2 className="max-w-lg text-4xl font-semibold leading-tight tracking-[-0.03em] text-text-main xl:text-5xl">
            Route AI infrastructure from one local control plane.
          </h2>
          <p className="mt-5 max-w-lg text-base leading-7 text-text-muted">
            Connect providers, manage routing, and keep credentials under your control.
          </p>
          <div className="mt-10 grid grid-cols-3 gap-3" aria-label="Platform capabilities">
            {["LOCAL FIRST", "MULTI PROVIDER", "SELF HOSTED"].map((label) => (
              <div key={label} className="rounded-[12px] border border-border-subtle bg-surface/55 p-3">
                <span className="mb-3 block size-1.5 rounded-full bg-emerald-500" aria-hidden="true" />
                <span className="console-label">{label}</span>
              </div>
            ))}
          </div>
        </section>
        <section className="flex items-center justify-center p-4 sm:p-8 lg:p-10">
          <Card elev className="w-full max-w-md rounded-[22px] p-6 sm:p-8">
            <div className="mb-7 flex items-center justify-between border-b border-border-subtle pb-5">
              <div className="traffic-lights" aria-hidden="true">
                <span className="traffic-light red" />
                <span className="traffic-light yellow" />
                <span className="traffic-light green" />
              </div>
              <span className="console-label">Authorized Access</span>
            </div>
            <div className="mb-7">
              <h1 className="text-2xl font-semibold tracking-tight text-text-main">ORouter</h1>
              <p className="mt-2 text-sm text-text-muted">
                {authMode === "oidc" && oidcConfigured
                  ? "Sign in with your OIDC provider to access the dashboard"
                  : "Enter your password to access the dashboard"}
              </p>
            </div>
          {mustChange ? (
            <form onSubmit={handleSetNewPassword} className="flex flex-col gap-4">
              <p className="text-sm text-amber-600 dark:text-amber-400 text-center">
                Set a new password before accessing the dashboard remotely.
              </p>
              <div className="flex flex-col gap-2">
                <label className="text-sm font-medium">New password</label>
                <Input
                  type="password"
                  placeholder="Enter new password"
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  required
                  autoFocus
                />
                {error && <p className="text-xs text-red-500">{error}</p>}
              </div>
              <Button type="submit" variant="primary" className="w-full" loading={loading} disabled={!newPassword}>
                Set password
              </Button>
            </form>
          ) : (
          <div className="flex flex-col gap-4">
            {oidcAvailable && (
              <Button type="button" variant="primary" className="w-full" onClick={handleOidcLogin}>
                {oidcLoginLabel}
              </Button>
            )}

            {oidcAvailable && passwordAvailable && <div className="h-px bg-border/60" />}

            {passwordAvailable ? (
              <form onSubmit={handleLogin} className="flex flex-col gap-4">
                {((authMode === "oidc" && !oidcConfigured) || (authMode === "both" && !oidcConfigured)) && (
                  <p className="text-xs text-amber-600 dark:text-amber-400 text-center">
                    OIDC login is enabled, but the issuer/client fields are not configured yet. Password login is still available for recovery.
                  </p>
                )}

                {authMode === "both" && oidcConfigured && (
                  <p className="text-xs text-text-muted text-center">
                    Password and OIDC login are both enabled.
                  </p>
                )}

                <div className="flex flex-col gap-2">
                  <label className="text-sm font-medium">Password</label>
                  <Input
                    type="password"
                    placeholder="Enter password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    required
                    autoFocus={!oidcAvailable}
                  />
                  {error && <p className="text-xs text-red-500">{error}</p>}
                  {retryAfter > 0 && (
                    <p className="text-xs text-amber-600 dark:text-amber-400">
                      Locked. Retry in <span className="font-mono">{retryAfter}s</span>.
                    </p>
                  )}
                  {resetHint && (
                    <p className="text-xs text-text-muted">
                      Forgot password? Open <code className="bg-sidebar px-1 rounded">9router</code> CLI on the host → <b>Settings</b> → <b>Reset Password to Default</b>.
                    </p>
                  )}
                </div>

                <Button
                  type="submit"
                  variant="primary"
                  className="w-full"
                  loading={loading}
                  disabled={retryAfter > 0}
                >
                  {retryAfter > 0 ? `Wait ${retryAfter}s` : "Login"}
                </Button>

                <p className="text-xs text-center text-text-muted mt-2">
                  Default password is <code className="bg-sidebar px-1 rounded">123456</code>
                </p>
                {hasPassword === false && (
                  <p className="text-xs text-center text-amber-600 dark:text-amber-400">
                    Security risk: no password set. You will be asked to set one when logging in remotely.
                  </p>
                )}
              </form>
            ) : (
              error && <p className="text-xs text-red-500">{error}</p>
            )}
          </div>
          )}
          </Card>
        </section>
      </main>
    </div>
  );
}
