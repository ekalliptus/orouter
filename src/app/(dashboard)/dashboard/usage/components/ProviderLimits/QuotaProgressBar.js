"use client";

import { cn } from "@/shared/utils/cn";
import { formatResetTime } from "./utils";

// Calculate color based on remaining percentage.
// Neobrutalist palette: bold flat fills (600 scale) + solid track, so the bar
// reads as confident/tegak rather than the washed-out /10 translucent fills.
const getColorClasses = (remainingPercentage) => {
  if (remainingPercentage > 70) {
    return {
      text: "text-emerald-700 dark:text-emerald-400",
      bg: "bg-emerald-500",
      bgLight: "bg-emerald-100 dark:bg-emerald-950/60",
      emoji: "🟢"
    };
  }

  if (remainingPercentage >= 30) {
    return {
      text: "text-amber-700 dark:text-amber-400",
      bg: "bg-amber-400",
      bgLight: "bg-amber-100 dark:bg-amber-950/60",
      emoji: "🟡"
    };
  }

  // 0-29% including 0% (out of quota) - show red
  return {
    text: "text-red-700 dark:text-red-400",
    bg: "bg-red-500",
    bgLight: "bg-red-100 dark:bg-red-950/60",
    emoji: "🔴"
  };
};

// Format reset time display
const formatResetTimeDisplay = (resetTime) => {
  if (!resetTime) return null;
  
  try {
    const resetDate = new Date(resetTime);
    const now = new Date();
    const isToday = resetDate.toDateString() === now.toDateString();
    const isTomorrow = resetDate.toDateString() === new Date(now.getTime() + 86400000).toDateString();
    
    const timeStr = resetDate.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      hour12: true,
    });
    
    if (isToday) return `Today, ${timeStr}`;
    if (isTomorrow) return `Tomorrow, ${timeStr}`;
    
    return resetDate.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      hour12: true,
    });
  } catch {
    return null;
  }
};

export default function QuotaProgressBar({
  percentage = 0,
  label = "",
  used = 0,
  total = 0,
  unlimited = false,
  resetTime = null,
  recurring = true,
}) {
  const colors = getColorClasses(percentage);
  const countdown = formatResetTime(resetTime);
  const resetDisplay = formatResetTimeDisplay(resetTime);

  // recurring defaults true. One-shot packs (e.g. CodeBuddy CN bonus packs)
  // set recurring:false: resetTime is a hard expiry, so word it as "expires".
  const resetWord = recurring ? "Reset" : "Expires";

  // percentage is already remaining percentage (from ProviderLimitCard)
  const remaining = percentage;
  
  return (
    <div className="space-y-2">
      {/* Label and percentage */}
      <div className="flex items-center justify-between text-sm">
        <span className="font-semibold text-text-primary">
          {label}
        </span>
        <div className="flex items-center gap-1.5">
          <span className="text-xs">{colors.emoji}</span>
          <span className={cn("font-medium", colors.text)}>
            {remaining}%
          </span>
        </div>
      </div>

      {/* Progress bar — neobrutalist: thicker, solid bold fill, hard black border + offset shadow.
          The track is rounded-full (preserved by the global exception) so it stays a pill, but
          the border + shadow give it the tegak/confident read the old translucent /10 lacked. */}
      {!unlimited && (
        <div
          className={cn(
            "relative h-3.5 w-full overflow-hidden border-2 border-black bg-white shadow-[3px_3px_0_0_#000]",
            "dark:bg-neutral-900"
          )}
        >
          <div
            className={cn("h-full !rounded-none transition-all duration-300", colors.bg)}
            style={{ width: `${Math.min(remaining, 100)}%` }}
          />
          {/* subtle inner track tint so the unfilled portion isn't pure white */}
          <div className={cn("pointer-events-none absolute inset-0 -z-0", colors.bgLight)} />
        </div>
      )}

      {/* Usage details and countdown */}
      <div className="flex items-center justify-between text-xs text-text-muted">
        <span>
          {used.toLocaleString()} / {total.toLocaleString()} requests
        </span>
        {countdown !== "-" && (
          <div className="flex items-center gap-1">
            <span>•</span>
            <span className="font-medium">{resetWord} in {countdown}</span>
          </div>
        )}
      </div>

      {/* Reset time display */}
      {resetDisplay && (
        <div className="text-xs text-text-muted/70">
          {resetWord} at {resetDisplay}
        </div>
      )}
    </div>
  );
}
