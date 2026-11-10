import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { PartyService } from "../gen/crm/parties/v2/party_pb";

// Browser same-origin Cloudflare Access supplies identity.
// No caller-selected tenant/actor or manual JSON DTO.
export function createCloudflareCrmClient(baseUrl: string) {
  const transport = createConnectTransport({ baseUrl, useBinaryFormat: true });
  return { parties: createClient(PartyService, transport) };
}
