/** Build-only: pin the published browser UMD bytes before Wrangler/esbuild.
 * axe.source uses Function#toString, so bundler-generated helpers (__name)
 * become dangling references in the real browser. Never use a runtime CDN. */
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";

const require = createRequire(import.meta.url);
const root = dirname(require.resolve("axe-core"));
const packageJson = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
if (packageJson.version !== "4.14.0") {
  throw new Error("BROWSER_AXE_UNEXPECTED_VERSION");
}
const raw = readFileSync(resolve(root, "axe.min.js"), "utf8");
if (raw.length < 100_000 || !raw.includes("axe")) {
  throw new Error("BROWSER_AXE_INVALID_SOURCE");
}
const output = new URL("../src/axe-source.generated.ts", import.meta.url);
writeFileSync(output,
  "/** @generated from integrity-pinned axe-core@4.14.0/axe.min.js by postinstall; do not edit. */\n" +
  "export const AXE_SOURCE = " + JSON.stringify(raw) + ";\n",
  "utf8",
);
