import { bindings, defineConfig } from "cf/config";
import * as entrypoint from "./worker/index.mjs" with { type: "cf-worker" };

// Dedicated CRM account. Never use OKX, sing-box or the personal account.
// This is a source-controlled pilot configuration, not a deployed release.
export default defineConfig({
  accountId: "5bd5e1e1e813f061afc259e91334262d",
  worker: {
    name: "crm-dev",
    compatibilityDate: "2026-11-07",
    entrypoint,
    env: {
      DB: bindings.d1({
        id: "35fc7ee9-9710-485f-8fb3-f5a398b229b4",
        name: "crm-dev",
      }),
    },
    assets: {
      notFoundHandling: "single-page-application",
      runWorkerFirst: ["/api/*"],
    },
    observability: { enabled: true, traces: { enabled: true } },
  },
});
