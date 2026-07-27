import { useState, useEffect } from "react";
import PropTypes from "prop-types";

export default function CooldownTimer({ until }) {
  const [remaining, setRemaining] = useState("");

  useEffect(() => {
    if (typeof document === "undefined") return undefined;
    const target = new Date(until).getTime();
    let interval = null;

    const updateRemaining = () => {
      const diff = target - Date.now();
      if (diff <= 0) {
        setRemaining("");
        // Cooldown is over — stop the timer instead of keeping it ticking at 1Hz
        // forever (previously it ran indefinitely after diff<=0). On a provider
        // list with several cooling-down models this removes N always-on timers.
        if (interval) {
          clearInterval(interval);
          interval = null;
        }
        return;
      }
      const secs = Math.floor(diff / 1000);
      if (secs < 60) {
        setRemaining(`${secs}s`);
      } else if (secs < 3600) {
        setRemaining(`${Math.floor(secs / 60)}m ${secs % 60}s`);
      } else {
        const hrs = Math.floor(secs / 3600);
        const mins = Math.floor((secs % 3600) / 60);
        setRemaining(`${hrs}h ${mins}m`);
      }
    };

    const start = () => {
      if (interval == null) {
        updateRemaining();
        interval = setInterval(updateRemaining, 1000);
      }
    };
    const stop = () => {
      if (interval) {
        clearInterval(interval);
        interval = null;
      }
    };
    const onVisibility = () => {
      if (document.hidden) stop();
      else start();
    };

    start();
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      stop();
    };
  }, [until]);

  if (!remaining) return null;

  return (
    <span className="text-xs text-orange-500 font-mono">
      ⏱ {remaining}
    </span>
  );
}

CooldownTimer.propTypes = {
  until: PropTypes.string.isRequired,
};
