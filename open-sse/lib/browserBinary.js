// Resolve a Chromium binary for playwright (de-minified from upstream
// 9router 0.5.109 chunk 4052).

import fs from "node:fs";
import path from "node:path";

export function isBrowserMissingError(err) {
  if (!err) return false;
  const msg = String(err).toLowerCase();
  return (
    msg.includes("executable doesn't exist") ||
    msg.includes("executablenotfound") ||
    msg.includes("playwright install") ||
    (msg.includes("chromium") && msg.includes("download"))
  );
}

export function findChromePath() {
  const env = process.env;
  const candidates = [
    env.NINEROUTER_BROWSER_BIN,
    env.CHROME_PATH,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    ...(env.PROGRAMFILES
      ? [path.join(env.PROGRAMFILES, "Google", "Chrome", "Application", "chrome.exe")]
      : []),
    ...(env["PROGRAMFILES(X86)"]
      ? [path.join(env["PROGRAMFILES(X86)"], "Google", "Chrome", "Application", "chrome.exe")]
      : []),
    ...(env.LOCALAPPDATA
      ? [path.join(env.LOCALAPPDATA, "Google", "Chrome", "Application", "chrome.exe")]
      : []),
  ];
  return candidates.find((p) => !!(p?.trim() && fs.existsSync(p.trim()))) || null;
}

// Launch via playwright, preferring the system Chrome. If the explicit
// executablePath fails, retry once with playwright's own Chromium.
export async function launchChromium(chromium, options = {}) {
  const executablePath = options.executablePath;
  if (!executablePath) return chromium.launch(options);
  try {
    return await chromium.launch(options);
  } catch (err) {
    console.warn(`[browser] ${executablePath} failed to launch (${err?.message || err}) — retrying with Playwright's Chromium`);
    const { executablePath: _drop, ...rest } = options;
    return chromium.launch(rest);
  }
}
