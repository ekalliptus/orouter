"use server";

import { NextResponse } from "next/server";
import { exec } from "child_process";
import { promisify } from "util";
import fs from "fs/promises";
import path from "path";
import os from "os";

const execAsync = promisify(exec);

const PROVIDER_NAME = "9router";

// Kimi Code CLI (newer, npm @moonshot-ai/kimi-code) uses ~/.kimi-code/config.toml.
// The older kimi-cli used ~/.kimi-cli/config.toml. We support both: prefer the new
// path for writes, detect/install-check both for reads.
const getCandidateDirs = () => [
  path.join(os.homedir(), ".kimi-code"),
  path.join(os.homedir(), ".kimi-cli"),
];
const getCandidateConfigPaths = () => getCandidateDirs().map((d) => path.join(d, "config.toml"));

// First existing config path (prefer .kimi-code). Null if none exist yet.
const findExistingConfigPath = async () => {
  for (const p of getCandidateConfigPaths()) {
    try {
      await fs.access(p);
      return p;
    } catch {}
  }
  return null;
};

// Path we WRITE to: existing config if present, else the new .kimi-code default.
const getWriteConfigPath = async () => {
  const existing = await findExistingConfigPath();
  if (existing) return existing;
  return path.join(os.homedir(), ".kimi-code", "config.toml");
};

// Simple TOML parser for key = "value" and [section] patterns (mirrors deepseek-tui-settings)
const parseToml = (content) => {
  const result = {};
  let currentSection = result;
  const lines = content.split(/\r?\n/);
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const sectionMatch = trimmed.match(/^\[([^\]]+)\]$/);
    if (sectionMatch) {
      const sectionName = sectionMatch[1];
      if (!result[sectionName]) result[sectionName] = {};
      currentSection = result[sectionName];
      continue;
    }
    const keyValueMatch = trimmed.match(/^(\w+)\s*=\s*"([^"]*)"$/);
    if (keyValueMatch) {
      currentSection[keyValueMatch[1]] = keyValueMatch[2];
      continue;
    }
    const unquotedMatch = trimmed.match(/^(\w+)\s*=\s*(.+)$/);
    if (unquotedMatch) {
      currentSection[unquotedMatch[1]] = unquotedMatch[2].trim();
    }
  }
  return result;
};

// Serialize a flat object back to TOML key="value" lines (for a single section body).
const serializeSectionBody = (obj) =>
  Object.entries(obj)
    .map(([k, v]) => `${k} = "${String(v).replace(/"/g, '\\"')}"`)
    .join("\n");

// Build/replace the [providers.9router] section, preserving the rest of the user's config.
// Kimi Code registers custom OpenAI-compatible endpoints as a [providers.<name>] section
// with type / base_url / api_key / model. We merge into existing config rather than wiping it.
const buildConfigWith9Router = (existingToml, baseUrl, apiKey, model) => {
  const normalizedBaseUrl = baseUrl.endsWith("/v1") ? baseUrl : `${baseUrl}/v1}`;
  const section = `type = "openai"\nbase_url = "${normalizedBaseUrl}"\napi_key = "${apiKey}"\nmodel = "${model}"`;

  const sectionHeader = `[providers.${PROVIDER_NAME}]`;
  const lines = (existingToml || "").split(/\r?\n/);

  // Find and replace an existing [providers.9router] block, else append.
  const out = [];
  let i = 0;
  let replaced = false;
  while (i < lines.length) {
    const trimmed = lines[i].trim();
    if (trimmed === sectionHeader) {
      // Drop the existing block (the header + following key=value lines until blank/next section)
      i++;
      while (i < lines.length) {
        const t = lines[i].trim();
        if (t === "" || t.startsWith("#") || t.startsWith("[")) break;
        i++;
      }
      out.push(sectionHeader);
      out.push(section);
      out.push("");
      replaced = true;
      continue;
    }
    out.push(lines[i]);
    i++;
  }
  if (!replaced) {
    if (out.length && out[out.length - 1] !== "") out.push("");
    out.push(sectionHeader);
    out.push(section);
    out.push("");
  }
  return out.join("\n");
};

const checkKimiInstalled = async () => {
  try {
    const isWindows = os.platform() === "win32";
    const command = isWindows ? "where kimi" : "which kimi";
    await execAsync(command, { windowsHide: true });
    return true;
  } catch {
    // Fall back to config-file presence (CLI may be installed but not on PATH)
    const existing = await findExistingConfigPath();
    return !!existing;
  }
};

const readConfigToml = async (configPath) => {
  try {
    return await fs.readFile(configPath, "utf-8");
  } catch (error) {
    if (error.code === "ENOENT") return "";
    throw error;
  }
};

// 9Router is configured if a [providers.9router] section exists with a base_url pointing at
// localhost / 127.0.0.1 / 0.0.0.0 / a known tunnel. Tunnel matching is done client-side.
const has9RouterConfig = (config) => {
  if (!config) return false;
  const section = config[`providers.${PROVIDER_NAME}`];
  if (!section?.base_url) return false;
  return /localhost|127\.0\.0\.1|0\.0\.0\.0/.test(section.base_url);
};

export async function GET() {
  try {
    const installed = await checkKimiInstalled();
    const configPath = await findExistingConfigPath();
    if (!installed || !configPath) {
      return NextResponse.json({ installed: false, settings: null, message: "Kimi CLI is not installed" });
    }
    const toml = await readConfigToml(configPath);
    const config = parseToml(toml);
    return NextResponse.json({
      installed: true,
      settings: config,
      has9Router: has9RouterConfig(config),
      configPath,
    });
  } catch (error) {
    console.log("Error checking kimi settings:", error);
    return NextResponse.json({ error: "Failed to check kimi settings" }, { status: 500 });
  }
}

export async function POST(request) {
  try {
    const { baseUrl, apiKey, model } = await request.json();
    if (!baseUrl || !model) {
      return NextResponse.json({ error: "baseUrl and model are required" }, { status: 400 });
    }

    const configPath = await getWriteConfigPath();
    await fs.mkdir(path.dirname(configPath), { recursive: true });

    // Merge into existing config (preserve user's other providers), don't wipe it.
    const existing = await readConfigToml(configPath);
    const newConfig = buildConfigWith9Router(existing, baseUrl, apiKey || "sk_9router", model);
    await fs.writeFile(configPath, newConfig);

    return NextResponse.json({
      success: true,
      message: "Kimi CLI settings applied successfully!",
      configPath,
    });
  } catch (error) {
    console.log("Error updating kimi settings:", error);
    return NextResponse.json({ error: "Failed to update kimi settings" }, { status: 500 });
  }
}

export async function DELETE() {
  try {
    const configPath = await findExistingConfigPath();
    if (!configPath) {
      return NextResponse.json({ success: true, message: "No config file to reset" });
    }
    const existing = await readConfigToml(configPath);
    if (!existing || !existing.includes(`[providers.${PROVIDER_NAME}]`)) {
      return NextResponse.json({ success: true, message: "No 9Router section to remove" });
    }

    // Remove ONLY the [providers.9router] block, keep everything else intact.
    const sectionHeader = `[providers.${PROVIDER_NAME}]`;
    const lines = existing.split(/\r?\n/);
    const out = [];
    let i = 0;
    while (i < lines.length) {
      const trimmed = lines[i].trim();
      if (trimmed === sectionHeader) {
        i++;
        while (i < lines.length) {
          const t = lines[i].trim();
          if (t === "" || t.startsWith("#") || t.startsWith("[")) break;
          i++;
        }
        continue;
      }
      out.push(lines[i]);
      i++;
    }
    await fs.writeFile(configPath, out.join("\n"));

    return NextResponse.json({ success: true, message: "9Router provider removed from Kimi config" });
  } catch (error) {
    console.log("Error resetting kimi settings:", error);
    return NextResponse.json({ error: "Failed to reset kimi settings" }, { status: 500 });
  }
}
