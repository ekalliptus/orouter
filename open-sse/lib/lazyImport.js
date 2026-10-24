// Cached dynamic import with on-demand npm install into <DATA_DIR>/runtime
// (de-minified from upstream 9router 0.5.109 chunk 20742). Lets optional
// heavy deps (playwright-core, wreq-js) stay out of the main install.

import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { createRequire } from "node:module";

const PINNED_VERSIONS = {
  "playwright-core": "^1.62.1",
  "wreq-js": "^3.2.0",
};

function runtimeDir() {
  const configured = process.env.DATA_DIR;
  const base = configured
    ? configured
    : process.platform === "win32"
      ? path.join(process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming"), "9router")
      : path.join(os.homedir(), ".9router");
  return path.join(base, "runtime");
}

function isModuleNotFoundError(err) {
  const code = String(err?.code || "");
  return code === "MODULE_NOT_FOUND" || code === "ERR_MODULE_NOT_FOUND";
}

const pending = new Map();

async function ensureInstalled(name) {
  try {
    return await import(name);
  } catch (err) {
    if (!isModuleNotFoundError(err)) throw err;
  }
  const requireFromRuntime = createRequire(path.join(runtimeDir(), "package.json"));
  try {
    return await import(pathToFileURL(requireFromRuntime.resolve(name)).href);
  } catch (err) {
    if (!isModuleNotFoundError(err)) throw err;
  }
  const dir = runtimeDir();
  fs.mkdirSync(dir, { recursive: true });
  const version = PINNED_VERSIONS[name] || "latest";
  const npmBin = process.platform === "win32" ? "npm.cmd" : "npm";
  const result = await new Promise((resolve) => {
    const child = spawn(npmBin, ["install", "--no-audit", "--no-fund", "--prefer-online", `${name}@${version}`], {
      cwd: dir,
      stdio: ["ignore", "pipe", "pipe"],
      shell: process.platform === "win32",
    });
    let stderr = "";
    child.stderr?.on("data", (chunk) => {
      stderr += String(chunk);
    });
    child.on("error", (err) => resolve({ ok: false, stderr: String(err.message || err) }));
    child.on("close", (code) => resolve({ ok: code === 0, stderr }));
  });
  if (!result.ok) {
    const detail =
      (result.stderr.match(/npm ERR! ?(.+)/) || [])[1] ||
      result.stderr.split("\n").filter(Boolean).pop() ||
      "unknown npm error";
    throw Error(
      `${name} is missing and the on-demand install failed (${String(detail).slice(0, 160)}) — install it manually: cd "${dir}" && npm install ${name}@${version}`
    );
  }
  return import(pathToFileURL(requireFromRuntime.resolve(name)).href);
}

export function lazyImport(name) {
  if (!pending.has(name)) {
    pending.set(name, ensureInstalled(name).finally(() => pending.delete(name)));
  }
  return pending.get(name);
}
