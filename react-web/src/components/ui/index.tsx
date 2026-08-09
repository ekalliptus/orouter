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
    sm: "0.35rem 0.75rem",
    md: "0.6rem 1.2rem",
    lg: "0.8rem 1.5rem",
  };

  const fontSizes: Record<string, string> = {
    sm: "0.9rem",
    md: "1.05rem",
    lg: "1.2rem",
  };

  return (
    <button
      className={`kid-btn kid-wobble ${className}`}
      style={{
        backgroundColor: bgColors[variant] || bgColors.primary,
        color: textColors[variant] || textColors.primary,
        padding: paddings[size] || paddings.md,
        fontSize: fontSizes[size] || fontSizes.md,
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
  const bgMap: Record<string, string> = {
    success: "var(--color-success)",
    danger: "var(--color-danger)",
    warning: "var(--color-warning)",
    info: "var(--color-info)",
    neutral: "var(--color-surface-3)",
  };

  return (
    <span
      className="kid-wobble"
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "0.35rem",
        padding: size === "sm" ? "0.15rem 0.45rem" : "0.3rem 0.65rem",
        fontSize: size === "sm" ? "0.8rem" : "0.95rem",
        fontWeight: 700,
        fontFamily: "var(--font-body)",
        backgroundColor: bgMap[variant] || bgMap.neutral,
        color: variant === "neutral" ? "var(--color-text-main)" : "#ffffff",
        border: "2px solid var(--nb-border)",
        boxShadow: "2px 2px 0 0 var(--nb-border)",
      }}
    >
      {dot && (
        <span
          style={{
            width: 8,
            height: 8,
            borderRadius: "50%",
            backgroundColor: "#ffffff",
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
  tilt = true,
  style,
  ...props
}) => {
  return (
    <div
      className={`kid-card kid-wobble ${tilt ? "kid-tilt" : ""} ${className}`}
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
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => !disabled && onChange(!checked)}
      style={{
        width: 52,
        height: 28,
        borderRadius: 9999,
        backgroundColor: checked ? "var(--color-success)" : "var(--color-surface-3)",
        border: "3px solid var(--nb-border)",
        boxShadow: "var(--nb-shadow-sm)",
        position: "relative",
        cursor: disabled ? "not-allowed" : "pointer",
        padding: 0,
        transition: "background-color 0.2s",
      }}
    >
      <span
        style={{
          width: 18,
          height: 18,
          borderRadius: 9999,
          backgroundColor: "#ffffff",
          border: "2px solid var(--nb-border)",
          position: "absolute",
          top: 2,
          left: checked ? 26 : 2,
          transition: "left 0.2s",
        }}
      />
    </button>
  );
};
