// Run: node scripts/check-visual-redesign.mjs. Offline; no app bootstrap or DB imports.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import vm from "node:vm";
const require = createRequire(import.meta.url);
const React = require("react");
const { renderToStaticMarkup } = require("react-dom/server");
const { transformSync } = require("next/dist/build/swc");
const root = fileURLToPath(new URL("../", import.meta.url));
const read = (path) => readFileSync(resolve(root, path), "utf8");
function component(path) {
  const { code } = transformSync(read(path), {
    filename: path, jsc: { parser: { syntax: "ecmascript", jsx: true }, transform: { react: { runtime: "automatic" } }, target: "es2020" }, module: { type: "commonjs" },
  });
  const compiled = { exports: {} };
  const safeRequire = (id) => {
    if (id === "@/shared/utils/cn") return { cn: (...args) => args.filter(Boolean).join(" ") };
    if (id === "./utils") return { formatResetTime: () => "-" };
    if (id === "./Button") return component("src/shared/components/Button.js");
    assert.ok(["react", "react/jsx-runtime"].includes(id), `Unexpected dependency: ${id}`);
    return require(id);
  };
  vm.runInNewContext(code, { module: compiled, exports: compiled.exports, require: safeRequire });
  return compiled.exports.default;
}
const render = (path, props) => renderToStaticMarkup(React.createElement(component(path), props));
const input = render("src/shared/components/Input.js", { label: "Password", required: true, error: "Required" });
assert.match(input, /required=""/);
assert.match(input, /aria-invalid="true"/);
const id = input.match(/<input id="([^"]+)"/)[1];
assert.ok(input.includes(`for="${id}"`));
assert.ok(input.includes(`aria-describedby="${id}-description"`));
const select = render("src/shared/components/Select.js", { label: "Provider", required: true, options: [{ value: "local", label: "Local" }] });
assert.match(select, /required=""/);
assert.match(select, /<option value="local">Local<\/option>/);
const button = render("src/shared/components/Button.js", { loading: true, children: "Save" });
assert.match(button, /disabled=""/);
assert.match(button, />Save<|Save</);
const quotaPath = "src/app/(dashboard)/dashboard/usage/components/ProviderLimits/QuotaProgressBar.js";
for (const [percentage, expected] of [[-5, 0], [45, 45], [130, 100]]) {
  const html = render(quotaPath, { percentage, label: "Quota" });
  assert.ok(html.includes(`aria-valuenow="${expected}"`));
  assert.ok(html.includes(`width:${expected}%`));
  assert.ok(!html.includes("absolute inset-0"), "Quota fill must not be covered by the track");
}
assert.ok(!render(quotaPath, { unlimited: true }).includes('role="progressbar"'));
const css = read("src/app/globals.css");
assert.ok(!/crayon|neobrut|nb-shadow|border-radius: 0 !important/i.test(css));
assert.match(css, /prefers-reduced-motion/);
function luminance(hex) {
  const channels = hex.match(/[a-f\d]{2}/gi).map((v) => parseInt(v, 16) / 255).map((v) => v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4);
  return channels.reduce((sum, v, i) => sum + v * [.2126, .7152, .0722][i], 0);
}
const light = css.split(":root {")[1].split("\n}")[0];
const dark = css.split(".dark {")[1].split("\n}")[0];
for (const mode of [light, dark]) {
  const tokens = Object.fromEntries([...mode.matchAll(/--color-([\w-]+): (#[a-f\d]{6});/gi)].map((m) => [m[1], m[2]]));
  for (const text of ["text-main", "text-muted", "text-subtle", "primary"]) {
    for (const background of ["bg", "surface", "sidebar"]) {
      const values = [luminance(tokens[text]), luminance(tokens[background])].sort((a, b) => b - a);
      assert.ok((values[0] + .05) / (values[1] + .05) >= 4.5, `${text}/${background} contrast`);
    }
  }
}
const modal = read("src/shared/components/Modal.js");
assert.match(modal, /dialog\.showModal\(\)/);
assert.match(modal, /previousFocus\.focus\(\)/);
assert.match(modal, /onCancel=/);
console.log("Visual regression checks passed: rendered controls, quota bounds, semantic contrast, modal contracts, reduced motion.");
