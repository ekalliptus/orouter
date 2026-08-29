import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "accent" | "danger" | "ghost";
  size?: "sm" | "md" | "lg";
}

export const Button: React.FC<ButtonProps> = ({
  children,
  variant = "primary",
  size = "md",
  className = "",
  style,
  ...props
}) => {
  const bgColors: Record<string, string> = {
    primary: "var(--color-primary)",
    secondary: "var(--color-surface)",
    accent: "var(--color-accent)",
    danger: "var(--color-danger)",
    ghost: "transparent",
  };

  const textColors: Record<string, string> = {
    primary: "#ffffff",
    secondary: "var(--color-text-main)",
    accent: "var(--color-text-main)",
    danger: "#ffffff",
    ghost: "var(--color-text-main)",
  };

  const paddings: Record<string, string> = {
    sm: "0.3rem 0.75rem",
    md: "0.42rem 1rem",
    lg: "0.55rem 1.5rem",
  };

  const fontSizes: Record<string, string> = {
    sm: "0.72rem",
    md: "0.875rem",
    lg: "0.9rem",
  };

  return (
    <button
      className={`kid-btn kid-wobble ${className}`}
      style={{
        backgroundColor: bgColors[variant] || bgColors.primary,
        color: textColors[variant] || textColors.primary,
        padding: paddings[size] || paddings.md,
        fontSize: fontSizes[size] || fontSizes.md,
        fontWeight: 600,
        border: variant === "ghost" ? "none" : "3px solid var(--nb-border)",
        boxShadow: variant === "ghost" ? "none" : "var(--nb-shadow-sm)",
        ...style,
      }}
      {...props}
    >
      {children}
    </button>
  );
};

interface BadgeProps {
  children: React.ReactNode;
  variant?: "success" | "danger" | "warning" | "info" | "neutral";
  size?: "sm" | "md";
  dot?: boolean;
}

export const Badge: React.FC<BadgeProps> = ({
  children,
  variant = "neutral",
  size = "md",
  dot = false,
}) => {
  // Node parity: tinted pill (bg color/10 + colored text); neutral uses
  // surface-2. Warning/neutral keep main text for contrast.
  const colorMap: Record<string, string> = {
    success: "var(--color-success)",
    danger: "var(--color-danger)",
    warning: "var(--color-warning)",
    info: "var(--color-info)",
    neutral: "var(--color-surface-3)",
  };
  const c = colorMap[variant] || colorMap.neutral;
  const textColor =
    variant === "neutral" || variant === "warning"
      ? "var(--color-text-main)"
      : c;

  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "0.3rem",
        padding: size === "sm" ? "0.12rem 0.5rem" : "0.22rem 0.62rem",
        fontSize: size === "sm" ? "0.68rem" : "0.75rem",
        fontWeight: 600,
        fontFamily: "var(--font-body)",
        borderRadius: 9999,
        backgroundColor:
          variant === "neutral"
            ? "var(--color-surface-2)"
            : `color-mix(in srgb, ${c} 12%, transparent)`,
        color: textColor,
      }}
    >
      {dot && (
        <span
          style={{
            width: 6,
            height: 6,
            borderRadius: "50%",
            backgroundColor: c,
            display: "inline-block",
          }}
        />
      )}
      {children}
    </span>
  );
};

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
  tilt?: boolean;
}

export const Card: React.FC<CardProps> = ({
  children,
  className = "",
  tilt = false,
  style,
  ...props
}) => {
  return (
    <div
      className={`kid-card ${tilt ? "kid-tilt" : ""} ${className}`}
      style={style}
      {...props}
    >
      {children}
    </div>
  );
};

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export const Toggle: React.FC<ToggleProps> = ({
  checked,
  onChange,
  disabled = false,
}) => {
  // Node parity: pill track, on = brand orange, off = surface-3, clean thumb.
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => !disabled && onChange(!checked)}
      style={{
        width: 44,
        height: 24,
        borderRadius: 9999,
        backgroundColor: checked ? "var(--color-primary)" : "var(--color-surface-3)",
        border: "none",
        position: "relative",
        cursor: disabled ? "not-allowed" : "pointer",
        padding: 0,
        transition: "background-color 0.2s",
      }}
    >
      <span
        style={{
          width: 20,
          height: 20,
          borderRadius: 9999,
          backgroundColor: "#ffffff",
          position: "absolute",
          top: 2,
          left: checked ? 22 : 2,
          transition: "left 0.2s ease",
        }}
      />
    </button>
  );
};
