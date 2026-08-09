const ICON_ALIASES: Record<string, string> = {
  "perplexity-agent": "perplexity",
  "gitlab-duo": "gitlab",
  "vercel-ai-gateway": "vercel",
};

const failedIds = new Set<string>();

function normalizeId(providerId?: string): string {
  if (!providerId || typeof providerId !== "string") return "";
  return providerId.trim().toLowerCase();
}

export function resolveProviderIconId(providerId?: string): string {
  const id = normalizeId(providerId);
  if (!id) return "";
  if (failedIds.has(id)) return "";
  const aliased = ICON_ALIASES[id] || id;
  if (failedIds.has(aliased)) return "";
  return aliased;
}

export function getProviderIconSrc(providerId?: string): string | null {
  const id = resolveProviderIconId(providerId);
  return id ? `/providers/${id}.png` : null;
}

export function markProviderIconMissing(providerId?: string): void {
  const id = normalizeId(providerId);
  if (id) failedIds.add(id);
  const aliased = ICON_ALIASES[id];
  if (aliased) failedIds.add(aliased);
}
