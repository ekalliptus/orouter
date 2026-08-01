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

// Escape a value for a TOML basic string ("..."). Backslash first, then quotes.
const escToml = (s) => String(s ?? "").replace(/\\/g, "\\\\").replace(/"/g, '\\"');

// Kimi Code schema (per Moonshot docs) needs TWO sections for a custom OpenAI-compatible
// endpoint: a [providers.<id>] transport block (type/base_url/api_key) AND a [models.<id>]
// block linking a model to that provider (provider/model/max_context_size/capabilities).
// A `model` key inside [providers.*] is NOT recognized — that caused "No models configured".
const PROVIDER_SECTION = `[providers.${PROVIDER_NAME}]`;
const MODEL_SECTION_KEY_PREFIX = "models.";
// Derive a stable model id from the model string (e.g. "cc/claude-opus-5" -> "claude-opus-5").
const deriveModelId = (model) =>
  String(model).replace(/^[a-zA-Z0-9]+\//, "").replace(/[^a-zA-Z0-9._-]/g, "-").toLowerCase() || "9router-model";

// Replace (or append) a single TOML section block, preserving everything else.
const replaceOrAppendSection = (lines, header, bodyLines) => {
  const out = [];
  let i = 0;
  let replaced = false;
  while (i < lines.length) {
    const trimmed = lines[i].trim();
    if (trimmed === header) {
      // Drop existing block: header + following key=value/comment lines until blank or next section.
      i++;
      while (i < lines.length) {
        const t = lines[i].trim();
        if (t === "" || t.startsWith("[")) break;
        i++;
      }
      out.push(header);
      out.push(...bodyLines);
      out.push("");
      replaced = true;
      continue;
    }
    out.push(lines[i]);
    i++;
  }
  if (!replaced) {
    if (out.length && out[out.length - 1] !== "") out.push("");
    out.push(header);
    out.push(...bodyLines);
    out.push("");
  }
  return out;
};

// Build the config: replace [providers.9router] + replace/add a [models.<id>] linked to it.
// Preserves all the user's other providers and models. Values are escaped for TOML.
const buildConfigWith9Router = (existingToml, baseUrl, apiKey, model) => {
  const normalizedBaseUrl = baseUrl.endsWith("/v1") ? baseUrl : `${baseUrl}/v1}`;
  const providerBody = [
    `type = "openai_legacy"`,
    `base_url = "${escToml(normalizedBaseUrl)}"`,
    `api_key = "${escToml(apiKey)}"`,
  ];
  const modelId = deriveModelId(model);
  const modelBody = [
    `provider = "${escToml(PROVIDER_NAME)}"`,
    `model = "${escToml(model)}"`,
    `max_context_size = 200000`,
    `capabilities = ["thinking"]`,
  ];

  let lines = (existingToml || "").split(/\r?\n/);
  // Replace provider section, then replace/add the model section (drop any prior 9router model), then add ours.
  lines = replaceOrAppendSection(lines, PROVIDER_SECTION, providerBody);
  // Remove any existing [models.*] blocks whose provider = "9router" (avoid stale dupes), then add ours.
  lines = stripStale9RouterModels(lines);
  lines = replaceOrAppendSection(lines, `[${MODEL_SECTION_KEY_PREFIX}${modelId}]`, modelBody);
  return lines.join("\n");
};

// Drop [models.<id>] blocks that point at the 9router provider (cleaning up our own prior writes
// when the model id changes). Leaves the user's other model blocks untouched.
const stripStale9RouterModels = (lines) => {
  const out = [];
  let i = 0;
  while (i < lines.length) {
    const trimmed = lines[i].trim();
    const isModelSection = /^\[models\.[^\]]+\]$/.test(trimmed);
    if (isModelSection) {
      // Peek the block to see if it belongs to 9router.
      let j = i + 1;
      let blockRefs9Router = false;
      while (j < lines.length) {
        const t = lines[j].trim();
        if (t === "" || t.startsWith("[")) break;
        if (/^provider\s*=/.test(t) && t.includes(`"${PROVIDER_NAME}"`)) blockRefs9Router = true;
        j++;
      }
      if (blockRefs9Router) {
        i = j; // skip this block entirely
        continue;
      }
    }
    out.push(lines[i]);
    i++;
  }
  return out;
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

    // Remove the [providers.9router] block AND any [models.*] blocks pointing at it,
    // keeping the user's other providers/models intact.
    const sectionHeader = `[providers.${PROVIDER_NAME}]`;
    let lines = existing.split(/\r?\n/);
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
    lines = stripStale9RouterModels(out);
    await fs.writeFile(configPath, lines.join("\n"));

    return NextResponse.json({ success: true, message: "9Router provider removed from Kimi config" });
  } catch (error) {
    console.log("Error resetting kimi settings:", error);
    return NextResponse.json({ error: "Failed to reset kimi settings" }, { status: 500 });
  }
}
