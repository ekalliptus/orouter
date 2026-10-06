// Shared validation for API-key policy fields (POST /api/keys, PUT /api/keys/[id]).
// Returns { patch, error } — patch holds only recognized, valid fields.
export function parsePolicyInput(body) {
  const patch = {};
  const { maxDevices, allowedModels, boundDevices, expiresAt, tokenLimit } = body;

  if (maxDevices !== undefined) {
    const n = Number(maxDevices);
    if (!Number.isFinite(n) || n < 0) return { error: "maxDevices must be a number >= 0" };
    patch.maxDevices = Math.floor(n);
  }

  if (allowedModels !== undefined) {
    if (!Array.isArray(allowedModels) || allowedModels.some((m) => typeof m !== "string")) {
      return { error: "allowedModels must be an array of strings" };
    }
    patch.allowedModels = allowedModels;
  }

  if (boundDevices !== undefined) {
    if (!Array.isArray(boundDevices) || boundDevices.some((d) => typeof d !== "string")) {
      return { error: "boundDevices must be an array of strings" };
    }
    patch.boundDevices = boundDevices;
  }

  if (expiresAt !== undefined) {
    if (expiresAt === null || expiresAt === "") {
      patch.expiresAt = null;
    } else if (Number.isNaN(new Date(expiresAt).getTime())) {
      return { error: "expiresAt must be a valid date" };
    } else {
      patch.expiresAt = new Date(expiresAt).toISOString();
    }
  }

  if (tokenLimit !== undefined) {
    const n = Number(tokenLimit);
    if (!Number.isFinite(n) || n < 0) return { error: "tokenLimit must be a number >= 0" };
    patch.tokenLimit = Math.floor(n);
  }

  return { patch };
}
