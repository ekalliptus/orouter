import { v4 as uuidv4 } from "uuid";
import { getAdapter } from "../driver.js";

function rowToKey(row) {
  if (!row) return null;
  let boundDevices = [];
  try {
    boundDevices = row.boundDevices ? JSON.parse(row.boundDevices) : [];
  } catch { boundDevices = []; }
  let allowedModels = [];
  try {
    allowedModels = row.allowedModels ? JSON.parse(row.allowedModels) : [];
  } catch { allowedModels = []; }
  return {
    id: row.id,
    key: row.key,
    name: row.name,
    machineId: row.machineId,
    isActive: row.isActive === 1 || row.isActive === true,
    createdAt: row.createdAt,
    maxDevices: row.maxDevices ?? 0,
    boundDevices,
    allowedModels,
    expiresAt: row.expiresAt || null,
    // Convenience flag for the UI
    expired: row.expiresAt ? new Date(row.expiresAt).getTime() <= Date.now() : false,
  };
}

export async function getApiKeys() {
  const db = await getAdapter();
  const rows = db.all(`SELECT * FROM apiKeys ORDER BY createdAt ASC`);
  return rows.map(rowToKey);
}

export async function getApiKeyById(id) {
  const db = await getAdapter();
  const row = db.get(`SELECT * FROM apiKeys WHERE id = ?`, [id]);
  return rowToKey(row);
}

export async function createApiKey(name, machineId) {
  if (!machineId) throw new Error("machineId is required");
  const db = await getAdapter();
  const { generateApiKeyWithMachine } = await import("@/shared/utils/apiKey");
  const result = generateApiKeyWithMachine(machineId);
  const apiKey = {
    id: uuidv4(),
    name,
    key: result.key,
    machineId,
    isActive: true,
    createdAt: new Date().toISOString(),
    maxDevices: 0,
    boundDevices: [machineId],
    allowedModels: [],
  };
  db.run(
    `INSERT INTO apiKeys(id, key, name, machineId, isActive, createdAt, maxDevices, boundDevices, allowedModels) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    [apiKey.id, apiKey.key, apiKey.name, apiKey.machineId, 1, apiKey.createdAt, 0, JSON.stringify([machineId]), "[]"]
  );
  return apiKey;
}

export async function updateApiKey(id, data) {
  const db = await getAdapter();
  let result = null;
  db.transaction(() => {
    const row = db.get(`SELECT * FROM apiKeys WHERE id = ?`, [id]);
    if (!row) return;
    const merged = { ...rowToKey(row), ...data };
    db.run(
      `UPDATE apiKeys SET key = ?, name = ?, machineId = ?, isActive = ?, maxDevices = ?, boundDevices = ?, allowedModels = ?, expiresAt = ? WHERE id = ?`,
      [
        merged.key,
        merged.name,
        merged.machineId,
        merged.isActive ? 1 : 0,
        merged.maxDevices ?? 0,
        JSON.stringify(merged.boundDevices || []),
        JSON.stringify(merged.allowedModels || []),
        merged.expiresAt || null,
        id,
      ]
    );
    result = merged;
  });
  return result;
}

export async function deleteApiKey(id) {
  const db = await getAdapter();
  const res = db.run(`DELETE FROM apiKeys WHERE id = ?`, [id]);
  return (res?.changes ?? 0) > 0;
}

export async function validateApiKey(key) {
  const db = await getAdapter();
  const row = db.get(`SELECT isActive, expiresAt FROM apiKeys WHERE key = ?`, [key]);
  if (!row) return false;
  if (!(row.isActive === 1 || row.isActive === true)) return false;
  // Expiry check: an expired key is invalid regardless of isActive.
  if (row.expiresAt && new Date(row.expiresAt).getTime() <= Date.now()) return false;
  return true;
}

/**
 * Full key lookup for request-time enforcement (device binding + model
 * allowlist). Returns the parsed key row or null.
 */
export async function getApiKeyRow(key) {
  const db = await getAdapter();
  const row = db.get(`SELECT * FROM apiKeys WHERE key = ?`, [key]);
  return rowToKey(row);
}

/**
 * Bind a device to a key if slots remain. Returns:
 *   { ok: true, bound: boolean }
 *   { ok: false, reason: "device_limit" }
 * No-op when the device is already bound.
 */
export async function bindDevice(key, machineId) {
  if (!machineId) return { ok: true, bound: false };
  const db = await getAdapter();
  let result = null;
  db.transaction(() => {
    const row = db.get(`SELECT isActive, maxDevices, boundDevices FROM apiKeys WHERE key = ?`, [key]);
    if (!row) {
      result = { ok: false, reason: "not_found" };
      return;
    }
    let devices = [];
    try {
      devices = row.boundDevices ? JSON.parse(row.boundDevices) : [];
    } catch { devices = []; }
    if (devices.includes(machineId)) {
      result = { ok: true, bound: false };
      return;
    }
    const max = row.maxDevices ?? 0;
    if (max > 0 && devices.length >= max) {
      result = { ok: false, reason: "device_limit" };
      return;
    }
    devices.push(machineId);
    db.run(`UPDATE apiKeys SET boundDevices = ? WHERE key = ?`, [JSON.stringify(devices), key]);
    result = { ok: true, bound: true };
  });
  return result;
}

/**
 * Model allowlist check. Empty list = all models allowed.
 */
export function isModelAllowed(keyRow, modelId) {
  const list = keyRow?.allowedModels || [];
  if (!Array.isArray(list) || list.length === 0) return true;
  if (!modelId) return true;
  if (list.includes(modelId)) return true;
  // Match bare id against prefixed ids (e.g. "glm-5.3" allowed when
  // allowedModels contains "glm/glm-5.3"), and vice versa.
  const bare = modelId.includes("/") ? modelId.split("/").slice(1).join("/") : modelId;
  return list.some((m) => m === bare || m === modelId || m.endsWith("/" + bare));
}
