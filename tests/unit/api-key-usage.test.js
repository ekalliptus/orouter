import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { beforeAll, afterAll, describe, expect, it, vi } from "vitest";

let token;
vi.mock("next/headers", () => ({ cookies: async () => ({ get: () => token ? { value: token } : undefined }) }));
let db, usage, GET, createToken, directory, originalKeys;
const a = "11111111-1111-4111-8111-111111111111";
const b = "22222222-2222-4222-8222-222222222222";
const empty = "33333333-3333-4333-8333-333333333333";
const rawA = "sk-same8-key-a", rawB = "sk-same8-key-b";
const now = Date.parse("2026-09-29T12:00:00.000Z");
const snapshot = () => ["apiKeys", "usageHistory", "usageDaily", "settings", "_meta"].map((table) => db.all(`SELECT * FROM ${table}`));

beforeAll(async () => {
  directory = fs.mkdtempSync(path.join(os.tmpdir(), "orouter-key-usage-"));
  vi.stubEnv("DATA_DIR", directory);
  vi.stubEnv("JWT_SECRET", "isolated-key-usage-test-secret");
  vi.stubEnv("NINEROUTER_STATS_TTL_MS", "0");
  vi.resetModules();
  db = await (await import("@/lib/db/driver.js")).getAdapter();
  usage = await import("@/lib/db/repos/usageRepo.js");
  ({ GET } = await import("@/app/api/keys/[id]/usage/route.js"));
  ({ createDashboardAuthToken: createToken } = await import("@/lib/auth/dashboardSession.js"));
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(now);
  for (const [id, key] of [[a, rawA], [b, rawB], [empty, "unused-key"]]) {
    db.run("INSERT INTO apiKeys(id, key, name, createdAt, isActive, maxDevices, boundDevices, allowedModels, expiresAt) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)",
      [id, key, "Same name", new Date(now).toISOString(), 0, 2, '["device-a"]', '["model-a"]', "2026-01-01T00:00:00.000Z"]);
  }
  const insert = (key, age, model, input, cost, status = "ok") => {
    db.run("INSERT INTO usageHistory(timestamp, apiKey, model, provider, promptTokens, completionTokens, cost, status, tokens) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)",
      [new Date(now - age).toISOString(), key, model, "provider", input, 2, cost, status, JSON.stringify({ prompt_tokens: input, completion_tokens: 2, cache_read_input_tokens: 1 })]);
  };
  insert(rawA, 1000, "model-a", 10, 0.1);
  insert(rawA, 1000, "model-b", 20, 0.2, "error");
  insert(rawB, 1000, "model-a", 900, 9);
  insert(rawA, 86400000, "boundary", 30, 0.3);
  insert(rawA, 86400001, "old", 40, 0.4);
  insert(rawA, 61 * 86400000, "expired-history", 50, 0.5);
  insert(rawA, -1000, "future", 60, 0.6);
  insert(null, 1000, "local", 70, 0.7);
  const day = { byApiKey: Object.fromEntries([[rawA, 3], [rawB, 7]].map(([key, count]) => [`${key}|model-a|provider`, {
    apiKey: key, rawModel: "model-a", provider: "provider", requests: count, promptTokens: count, cost: count,
  }])) };
  db.run("INSERT INTO usageDaily(dateKey, data) VALUES(?, ?)", ["2026-09-29", JSON.stringify(day)]);
  originalKeys = snapshot();
});

afterAll(() => {
  expect(snapshot()).toEqual(originalKeys);
  db.close?.();
  vi.useRealTimers();
  vi.unstubAllEnvs();
  fs.rmSync(directory, { recursive: true, force: true });
});

async function request(id = a, query = "") {
  return GET(new Request(`http://localhost/api/keys/${id}/usage${query}`), { params: Promise.resolve({ id }) });
}

describe("API key usage with isolated SQLite", () => {
  it("separates identical first8 and names using exact key IDs, including paused/expired keys", async () => {
    const first = await usage.getApiKeyUsage(a);
    const second = await usage.getApiKeyUsage(b);
    expect(first.summary).toMatchObject({ requests: 3, promptTokens: 60, completionTokens: 6, cachedTokens: 3, lastUsed: new Date(now - 1000).toISOString() });
    expect(first.summary.cost).toBeCloseTo(0.6);
    expect(second.summary).toMatchObject({ requests: 1, promptTokens: 900, cost: 9 });
    expect(first.models.map((row) => row.model).sort()).toEqual(["boundary", "model-a", "model-b"]);
    expect(first.requests.some((row) => row.status === "error")).toBe(true);
    expect(JSON.stringify(first)).not.toContain(rawA);
    expect(JSON.stringify(second)).not.toContain(rawB);
  });
  it.each(["7d", "30d", "60d"])("bounds period %s, excluding future records", async (period) => {
    const result = await usage.getApiKeyUsage(a, { period });
    expect(result.summary.requests).toBe(4);
    expect(result.summary.promptTokens).toBe(100);
  });
  it("paginates deterministically, counts all scoped rows, handles empty pages", async () => {
    const first = await usage.getApiKeyUsage(a, { pageSize: 1 });
    const second = await usage.getApiKeyUsage(a, { pageSize: 1, page: 2 });
    expect(first.requests[0].id).not.toBe(second.requests[0].id);
    expect(first.requests[0].model).toBe("model-b");
    expect(first.pagination).toEqual({ page: 1, pageSize: 1, total: 3, totalPages: 3 });
    expect((await usage.getApiKeyUsage(a, { page: 20 })).requests).toEqual([]);
    expect((await usage.getApiKeyUsage(empty)).summary).toMatchObject({ requests: 0, cost: 0, lastUsed: null });
  });
  it.each(["24h", "7d", "all"])("global %s stats use stable IDs without leaking credential object keys", async (period) => {
    const stats = await usage.getUsageStats(period);
    const rows = Object.values(stats.byApiKey);
    expect(rows.filter((row) => row.apiKeyId === a).length).toBeGreaterThan(0);
    expect(rows.filter((row) => row.apiKeyId === b).length).toBe(1);
    expect(rows.find((row) => row.apiKeyId === b).promptTokens).toBe(period === "24h" ? 900 : 7);
    expect(rows.filter((row) => row.apiKeyId === a).reduce((sum, row) => sum + row.requests, 0)).toBe(3);
    expect(JSON.stringify(stats)).not.toContain(rawA);
    expect(JSON.stringify(stats)).not.toContain(rawB);
    expect(rows.find((row) => row.apiKeyId === a).apiKeyKey).toBe(a);
  });
  it("requires a signed authenticated dashboard session, not an API key", async () => {
    for (const value of [undefined, "invalid", rawA, await createToken({ authenticated: false })]) {
      token = value;
      expect((await request()).status).toBe(401);
    }
    token = await createToken();
    const response = await request();
    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toContain("no-store");
    expect((await response.json()).key).toEqual({ id: a, name: "Same name" });
    vi.setSystemTime(now + 25 * 3600000);
    expect((await request()).status).toBe(401);
    vi.setSystemTime(now);
  });
  it("returns 404 for unknown IDs and validates query boundaries", async () => {
    token = await createToken();
    expect((await request("unknown-key")).status).toBe(404);
    expect((await request("' OR 1=1 --")).status).toBe(400);
    for (const query of ["period=all", "period=__proto__", "page=0", "page=100001", "page=1.5", "pageSize=101", "pageSize=-1", "page=1&page=2", "apiKey=secret", "page=1e2", "period="]) {
      expect((await request(a, `?${query}`)).status, query).toBe(400);
    }
    expect((await request(a, "?pageSize=100&page=100000&period=60d")).status).toBe(200);
  });
});
