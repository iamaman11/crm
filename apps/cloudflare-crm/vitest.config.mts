import {fileURLToPath} from "node:url";
import {defineConfig} from "vitest/config";

// One native Vitest discovery boundary; historical client suites are not active CRM tests.
export default defineConfig({
  root: fileURLToPath(new URL("../../", import.meta.url)),
  test: {
    include: [
      "packages/client/src/**/cloudflare-*.test.ts",
      "apps/cloudflare-crm/test/**/*.vitest.test.{mjs,mts,ts,tsx}",
    ],
    passWithNoTests: false,
  },
});
