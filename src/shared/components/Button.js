"use client";

import { cn } from "@/shared/utils/cn";

const variants = {
  primary: "bg-gradient-to-br from-brand-400 to-brand-600 text-white shadow-[0_10px_28px_rgb(220_91_61_/_24%)] hover:brightness-105 disabled:from-surface-3 disabled:to-surface-3 disabled:text-text-muted",
  secondary: "bg-surface-2/80 text-text-main border border-border hover:bg-surface-3",
  outline: "border border-border text-text-main bg-transparent hover:bg-surface-2/70 hover:border-brand-500/40",
  ghost: "text-text-muted hover:bg-surface-2/70 hover:text-text-main",
  danger: "bg-red-500 text-white hover:bg-red-600 shadow-sm disabled:bg-surface-3 disabled:text-text-muted",
  success: "bg-emerald-600 text-white hover:bg-emerald-700 shadow-sm disabled:bg-surface-3 disabled:text-text-muted",
};

const sizes = {
  sm: "h-7 px-3 text-xs rounded-[8px]",
  md: "h-9 px-4 text-sm rounded-[10px]",
  lg: "h-11 px-6 text-sm rounded-[10px]",
};

export default function Button({
  children,
  variant = "primary",
  size = "md",
  icon,
  iconRight,
  disabled = false,
  loading = false,
  fullWidth = false,
  className,
  ...props
}) {
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center gap-2 font-semibold transition-all duration-150 ease-out cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/55 focus-visible:ring-offset-2 focus-visible:ring-offset-bg",
        "active:scale-[0.97] disabled:opacity-50 disabled:cursor-not-allowed disabled:active:scale-100",
        variants[variant],
        sizes[size],
        fullWidth && "w-full",
        className
      )}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? (
        <span className="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
      ) : icon ? (
        <span className="material-symbols-outlined text-[18px]">{icon}</span>
      ) : null}
      {children}
      {iconRight && !loading && (
        <span className="material-symbols-outlined text-[18px]">{iconRight}</span>
      )}
    </button>
  );
}
