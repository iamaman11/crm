/// <reference types="vitest" />
import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import { readFileSync } from "node:fs";

const GATEWAY_SERVICE_PREFIX = "/crm.gateway.v1.ApplicationGatewayService";

export default defineConfig(({ mode }) => {
  const environment = loadEnv(mode, process.cwd(), "VITE_");
  const grpcWebTarget =
    environment.VITE_CRM_GRPC_WEB_TARGET ?? "http://127.0.0.1:50051";

  const browserAcceptance = process.env.G2_E2E === "1";
  const gatewayProxy = {
    target: grpcWebTarget,
    changeOrigin: false,
  };
  const authProxy = {
    target: "http://127.0.0.1:8080",
    changeOrigin: false,
  };
  return {
    plugins: [react()],
    // The production build remains unchanged; only the isolated CI preview
    // receives a temporary HTTPS certificate and same-origin service proxies.
    ...(browserAcceptance
      ? {
          preview: {
            https: {
              key: readFileSync(process.env.G2_TLS_KEY!),
              cert: readFileSync(process.env.G2_TLS_CERT!),
            },
            proxy: {
              [GATEWAY_SERVICE_PREFIX]: gatewayProxy,
              "/auth": authProxy,
            },
          },
        }
      : {}),
    server: {
      proxy: {
        [GATEWAY_SERVICE_PREFIX]: {
          target: grpcWebTarget,
          changeOrigin: false,
        },
      },
    },
    test: {
      exclude: ["**/node_modules/**", "**/dist/**", "e2e/**"],
    },
  };
});
