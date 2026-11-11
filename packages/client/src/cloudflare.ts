import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { PartyService } from "../gen/crm/parties/v2/party_pb";
import { PartyService as PartyV3 } from "../gen/crm/parties/v3/party_pb";
import { AccountService } from "../gen/crm/accounts/v2/account_pb";
import { ContactPointService } from "../gen/crm/contact_points/v2/contact_point_pb";

// One generated binary Connect client for the single Rust Worker.
// Cloudflare Access supplies identity. Browser NEVER supplies tenant/actor.
// The v3/v2 candidate services remain HTTP 410 in contract-only production;
// inclusion here does NOT activate them before the full cutover.
export function createCloudflareCrmClient(baseUrl: string) {
  const transport = createConnectTransport({ baseUrl, useBinaryFormat: true });
  return {
    parties: createClient(PartyService, transport), // existing published read RPC
    partyRecords: createClient(PartyV3, transport),
    accounts: createClient(AccountService, transport),
    contactPoints: createClient(ContactPointService, transport),
  };
}
