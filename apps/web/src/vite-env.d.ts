/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_CRM_GRPC_WEB_TARGET?: string;
  readonly VITE_CRM_DEV_BEARER_TOKEN?: string;
  readonly VITE_CRM_DEV_TENANT_ID?: string;
  readonly VITE_CRM_DEV_ACTOR_LABEL?: string;
  readonly VITE_CRM_DEV_TENANT_LABEL?: string;
  readonly VITE_CRM_DEV_CAPABILITIES?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
