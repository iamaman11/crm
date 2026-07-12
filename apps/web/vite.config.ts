/// <reference types="vitest" />
import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

const GATEWAY_SERVICE_PREFIX = "/crm.gateway.v1.ApplicationGatewayService";

export default defineConfig(({ mode }) => {
  const environment = loadEnv(mode, process.cwd(), "VITE_");
  const grpcWebTarget =
    environment.VITE_CRM_GRPC_WEB_TARGET ?? "http://127.0.0.1:50051";

  return {
    plugins: [react()],
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
