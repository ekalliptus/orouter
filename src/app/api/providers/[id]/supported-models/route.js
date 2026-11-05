import { NextResponse } from "next/server";
import { getProviderConnections } from "@/models";
import { ANTIGRAVITY_OAUTH_CLIENT, ANTIGRAVITY_CLI_USER_AGENT } from "open-sse/providers/shared.js";

// GET /api/providers/[id]/supported-models
// Reports which models each connection of a provider is entitled to, sourced
// live from Google's fetchAvailableModels (aidev_client CLI fingerprint — the
// only surface that reports argon-tier models). Cached 5 minutes per
// connection to keep page loads cheap.

const CACHE = new Map(); // connectionId -> { at, payload }
const TTL_MS = 5 * 60 * 1000;
const MODELS_URL = "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels";

const SUPPORTED_PROVIDERS = new Set(["antigravity"]);

export const dynamic = "force-dynamic";

async function fetchEnabledModels(connection) {
  let accessToken = connection.accessToken;
  for (let attempt = 0; attempt < 2; attempt++) {
    const res = await fetch(MODELS_URL, {
      method: "POST",
      headers: {
        "Authorization": `Bearer ${accessToken}`,
        "Content-Type": "application/json",
        "User-Agent": ANTIGRAVITY_CLI_USER_AGENT,
      },
      body: JSON.stringify({ project: "aicode-consumers" }),
      signal: AbortSignal.timeout(15000),
    });
    if (res.ok) {
      const json = await res.json();
      const entries = Object.entries(json.models || {});
      return entries.map(([id, info]) => ({
        id,
        enabled: info?.disabled !== true,
        recommended: !!info?.recommended,
        quotaRemaining: typeof info?.quotaInfo?.remainingFraction === "number" ? info.quotaInfo.remainingFraction : null,
        resetAt: info?.quotaInfo?.resetTime || null,
      }));
    }
    if ((res.status === 401 || res.status === 403) && attempt === 0 && connection.refreshToken) {
      const tokRes = await fetch("https://oauth2.googleapis.com/token", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({
          grant_type: "refresh_token",
          refresh_token: connection.refreshToken,
          client_id: ANTIGRAVITY_OAUTH_CLIENT.clientId,
          client_secret: ANTIGRAVITY_OAUTH_CLIENT.clientSecret,
        }),
        signal: AbortSignal.timeout(15000),
      });
      if (tokRes.ok) {
        accessToken = (await tokRes.json()).access_token;
        continue;
      }
    }
    throw Error(`upstream ${res.status}`);
  }
  throw Error("upstream exhausted retries");
}

export async function GET(request, { params }) {
  const { id } = await params;
  if (!SUPPORTED_PROVIDERS.has(id)) {
    return NextResponse.json({ error: "supported-models reporting is not available for this provider" }, { status: 400 });
  }
  try {
    const connections = (await getProviderConnections({ provider: id })).filter((c) => c.isActive !== false);
    const results = await Promise.all(connections.map(async (conn) => {
      const cached = CACHE.get(conn.id);
      if (cached && Date.now() - cached.at < TTL_MS) return cached.payload;
      let payload;
      try {
        const models = await fetchEnabledModels(conn);
        payload = { id: conn.id, email: conn.email || conn.name || null, models, error: null };
      } catch (err) {
        payload = { id: conn.id, email: conn.email || conn.name || null, models: [], error: err.message };
      }
      // Failed lookups cache briefly (60s) so a dead token doesn't hammer Google.
      CACHE.set(conn.id, { at: Date.now(), payload, ttl: payload.error ? 60000 : TTL_MS });
      return payload;
    }));
    // Honor per-entry TTL: failed lookups expire sooner.
    const now = Date.now();
    for (const r of results) {
      const entry = CACHE.get(r.id);
      if (entry && now - entry.at > entry.ttl) CACHE.delete(r.id);
    }
    return NextResponse.json({ connections: results });
  } catch (err) {
    return NextResponse.json({ error: err.message || "Failed to load supported models" }, { status: 500 });
  }
}
