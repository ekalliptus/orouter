"use client";

import { useId } from "react";
import { cn } from "@/shared/utils/cn";

export default function Input({ label, type = "text", placeholder, value, onChange, error, hint, icon, disabled = false, required = false, className, inputClassName, id, ...props }) {
  const generatedId = useId();
  const inputId = id || generatedId;
  const descriptionId = `${inputId}-description`;
  return (
    <div className={cn("flex flex-col gap-1.5", className)}>
      {label && <label htmlFor={inputId} className="text-sm font-medium text-text-main">{label}{required && <span className="text-danger ml-1">*</span>}</label>}
      <div className="relative">
        {icon && <div aria-hidden="true" className="absolute inset-y-0 left-0 flex items-center pl-3 pointer-events-none text-text-muted"><span className="material-symbols-outlined text-[20px]">{icon}</span></div>}
        <input
          id={inputId} type={type} placeholder={placeholder} value={value} onChange={onChange}
          disabled={disabled} required={required} aria-invalid={error ? true : undefined}
          aria-describedby={error || hint ? descriptionId : undefined}
          className={cn(
            "w-full min-h-10 py-2.5 px-3 text-text-main bg-surface rounded-lg border border-border placeholder:text-text-subtle",
            "focus:outline-none focus:ring-2 focus:ring-primary/25 focus:border-primary transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-[16px] sm:text-sm",
            icon && "pl-10", error && "border-danger", inputClassName
          )}
          {...props}
        />
      </div>
      {error && <p id={descriptionId} role="alert" className="text-xs text-danger">{error}</p>}
      {hint && !error && <p id={descriptionId} className="text-xs text-text-muted">{hint}</p>}
    </div>
  );
}
