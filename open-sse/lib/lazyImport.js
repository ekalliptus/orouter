// Cached dynamic import with on-demand npm install into <DATA_DIR>/runtime
// (port of upstream 9router 0.5.109 chunk 20742). Lets optional heavy deps
// (playwright-core, wreq-js) stay out of the main install.
//
// Implementation note: real node require() via createRequire is used instead
// of import() — inside the webpack server bundle a variable import() gets
// rewritten and CJS interop drops named exports (chromium etc.), while a
// createRequire'd require is untouched by webpack and returns module.exports
// directly.

import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import nodeModule from "node:module";

// Default-import + property access: webpack eliminated the named
// `import { createRequire } from "node:module"` binding down to `(void 0)`
// in the server bundle, so require() calls crashed at runtime.
const createRequire = nodeModule.createRequire;

const PINNED_VERSIONS = {
  "playwright-core": "^1.62.1",
  "wreq-js": "^3.2.0",
};

function dataDir() {
  if (process.env.DATA_DIR) return process.env.DATA_DIR;
  if (process.platform === "win32") {
    return path.join(process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming"), "9router");
  }
  return path.join(os.homedir(), ".9router");
}

function runtimeDir() {
  return path.join(dataDir(), "runtime");
}

function isModuleNotFoundError(err) {
  const code = String(err?.code || "");
  return code === "MODULE_NOT_FOUND" || code === "ERR_MODULE_NOT_FOUND";
}

// CJS deps resolve to module.exports directly; keep ESM namespaces intact.
function normalize(mod) {
  return mod?.default && typeof mod.default === "object" && Object.keys(mod).length === 1 ? mod.default : mod;
}

const pending = new Map();

async function ensureInstalled(name) {
  const requireFromRuntime = createRequire(path.join(runtimeDir(), "package.json"));
  const requireFromCwd = createRequire(path.join(process.cwd(), "package.json"));

  try {
    return normalize(requireFromCwd(name));
  } catch (err) {
    if (!isModuleNotFoundError(err)) throw err;
  }
  try {
    return normalize(requireFromRuntime(name));
  } catch (err) {
    if (!isModuleNotFoundError(err)) throw err;
  }

  fs.mkdirSync(runtimeDir(), { recursive: true });
  const version = PINNED_VERSIONS[name] || "latest";
  const npmBin = process.platform === "win32" ? "npm.cmd" : "npm";
  const result = await new Promise((resolve) => {
    const child = spawn(npmBin, ["install", "--no-audit", "--no-fund", "--prefer-online", `${name}@${version}`], {
      cwd: runtimeDir(),
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
      `${name} is missing and the on-demand install failed (${String(detail).slice(0, 160)}) — install it manually: cd "${runtimeDir()}" && npm install ${name}@${version}`
    );
  }
  return normalize(requireFromRuntime(name));
}

export function lazyImport(name) {
  if (!pending.has(name)) {
    pending.set(name, ensureInstalled(name).finally(() => pending.delete(name)));
  }
  return pending.get(name);
}
