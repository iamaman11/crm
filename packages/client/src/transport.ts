import { createClient, type Client, type Interceptor } from "@connectrpc/connect";
import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { ApplicationGatewayService } from "../gen/crm/gateway/v1/gateway_pb";
import type { SessionProvider } from "./session";

const TENANT_HEADER = "x-tenant-id";
const REQUEST_ID_HEADER = "x-request-id";
const CORRELATION_ID_HEADER = "x-correlation-id";
const TRACE_ID_HEADER = "x-trace-id";

export interface ApplicationGatewayClientOptions {
  baseUrl: string;
  sessionProvider: SessionProvider;
  idFactory?: () => string;
}

export function createApplicationGatewayClient(
  options: ApplicationGatewayClientOptions,
): Client<typeof ApplicationGatewayService> {
  const idFactory = options.idFactory ?? defaultRequestId;
  const sessionInterceptor: Interceptor = (next) => async (request) => {
    const session = options.sessionProvider.getSnapshot();
    const requestId = idFactory();

    if (session.status === "authenticated") {
      request.header.set("authorization", `Bearer ${session.bearerToken}`);
      request.header.set(TENANT_HEADER, session.tenantId);
    }
    request.header.set(REQUEST_ID_HEADER, requestId);
    request.header.set(CORRELATION_ID_HEADER, requestId);
    request.header.set(TRACE_ID_HEADER, requestId);

    return await next(request);
  };

  const transport = createGrpcWebTransport({
    baseUrl: normalizeBaseUrl(options.baseUrl),
    interceptors: [sessionInterceptor],
  });

  return createClient(ApplicationGatewayService, transport);
}

function normalizeBaseUrl(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function defaultRequestId(): string {
  return globalThis.crypto.randomUUID();
}
