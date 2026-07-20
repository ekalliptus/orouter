import { readFileSync, readdirSync } from "node:fs";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = new URL("../", import.meta.url);
const rootPath = fileURLToPath(root);
const read = (file) => readFileSync(new URL(file, root), "utf8");

const mustSayORouter = [
  "src/app/layout.js",
  "src/app/manifest.js",
  "src/shared/constants/config.js",
  "README.md",
  "package.json",
];
for (const file of mustSayORouter) {
  if (!read(file).includes("ORouter")) throw new Error(`${file} is missing ORouter branding`);
}

const forbiddenCurrentCopy = [
  "src/app/layout.js",
  "src/app/manifest.js",
  "src/shared/constants/config.js",
  "src/app/landing/page.js",
  "src/app/landing/components/FlowAnimation.js",
  "src/app/landing/components/Footer.js",
  "src/app/landing/components/GetStarted.js",
  "src/app/landing/components/HeroSection.js",
  "src/app/landing/components/HowItWorks.js",
  "src/app/landing/components/Navigation.js",
  "src/app/(dashboard)/dashboard/endpoint/EndpointPageClient.js",
  "src/app/(dashboard)/dashboard/token-saver/TokenSaverClient.js",
  "src/app/(dashboard)/dashboard/usage/components/ProviderTopology.js",
  "src/shared/components/DonateModal.js",
  "src/shared/constants/skills.js",
];
for (const file of forbiddenCurrentCopy) {
  if (/9Router|github\.com\/(?:decolua|ekalliptus)\/9router/.test(read(file))) {
    throw new Error(`${file} retains current 9Router branding`);
  }
}

const literalsDir = new URL("public/i18n/literals/", root);
for (const entry of readdirSync(literalsDir, { withFileTypes: true })) {
  if (entry.isFile() && extname(entry.name) === ".json" && read(`public/i18n/literals/${entry.name}`).includes("9Router")) {
    throw new Error(`public/i18n/literals/${entry.name} retains current 9Router branding`);
  }
}

const walk = (dir) => readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
  const path = join(dir, entry.name);
  if (entry.name === "node_modules" || entry.name.startsWith(".next")) return [];
  return entry.isDirectory() ? walk(path) : [path];
});
const compatibilitySource = ["src", "open-sse", "cli"]
  .flatMap((dir) => walk(join(rootPath, dir)))
  .filter((file) => [".js", ".mjs"].includes(extname(file)))
  .map((file) => readFileSync(file, "utf8"))
  .join("\n");
for (const value of ["x-9router-token-saver", "x-9router-connection-id", ".9router"]) {
  if (!compatibilitySource.includes(value)) throw new Error(`compatibility identifier removed: ${value}`);
}

const packageJson = JSON.parse(read("package.json"));
if (packageJson.name !== "orouter-app" || packageJson.description !== "ORouter web dashboard") {
  throw new Error("package.json ORouter identity is invalid");
}
for (const value of ["bun i -g 9router", 'npmPackageName: "9router"', "decolua/9router", "~/.9router"]) {
  const sources = value === "decolua/9router" || value === "~/.9router" ? `${read("README.md")}\n${compatibilitySource}` : read("src/shared/constants/config.js");
  if (!sources.includes(value)) throw new Error(`distribution compatibility identifier removed: ${value}`);
}

console.log("ORouter branding and compatibility invariants passed");
