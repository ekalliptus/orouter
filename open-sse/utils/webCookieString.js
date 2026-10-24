// Cookie-string helpers for webCookie providers (de-minified from upstream
// 9router 0.5.109 webpack chunks 82543 + 10079).

const COOKIE_ATTRS = new Set(["path", "domain", "expires", "max-age", "secure", "httponly", "samesite"]);

function cleanCookieValue(raw) {
  const s = (raw || "").trim();
  return s ? s.replace(/^bearer\s+/i, "").replace(/^cookie:/i, "").trim() : "";
}

// Accepts "name=value; name2=value2", a bare __Secure-1PSID value, or a JSON
// object/array of cookies — returns a normalized "n=v; n2=v2" string.
export function normalizeCookieInput(raw, defaultName = "__Secure-1PSID") {
  const s = (raw || "").trim();
  if (!s) return "";
  const fromJson = (() => {
    if (!s.startsWith("{")) return [];
    try {
      const parsed = JSON.parse(s);
      const jar = parsed?.cookies && typeof parsed.cookies === "object" && !Array.isArray(parsed.cookies)
        ? parsed.cookies
        : parsed;
      return Object.entries(jar)
        .filter(([, v]) => typeof v === "string" && v.trim().length > 0)
        .map(([k, v]) => `${k}=${String(v).trim()}`);
    } catch {
      return [];
    }
  })();
  if (fromJson.length > 0) return fromJson.join("; ");
  if (s.includes("=")) return s;
  return `${defaultName}=${s}`;
}

// Cookie header string → playwright cookie objects for the given domain.
export function toPlaywrightCookies(cookieStr, domain) {
  const cookies = [];
  for (const part of String(cookieStr || "").split(";")) {
    const pair = part.trim();
    if (!pair) continue;
    const eq = pair.indexOf("=");
    if (eq <= 0) continue;
    const name = pair.slice(0, eq).trim();
    const value = pair.slice(eq + 1).trim();
    if (!name || !value || COOKIE_ATTRS.has(name.toLowerCase())) continue;
    const normalizedDomain = String(domain || "").replace(/^\./, "");
    cookies.push({
      name,
      value,
      domain: name.startsWith("__Host-") ? normalizedDomain : domain,
      path: "/",
      secure: true,
    });
  }
  return cookies;
}

// Extract one cookie's value out of a full cookie header string (or accept
// the bare value directly).
export function extractCookieValue(raw, name) {
  const s = cleanCookieValue(raw);
  if (!s) return "";
  if (s.includes(";")) {
    const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const match = s.match(new RegExp("(?:^|;\\s*)" + escaped + "=([^;\\s]+)"));
    return match ? match[1] : "";
  }
  const prefix = `${name}=`;
  return s.startsWith(prefix) ? s.slice(prefix.length) : "";
}
