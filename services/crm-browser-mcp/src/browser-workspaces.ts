/** Explicit session partitioning: ChatGPT provides no trusted conversation ID.
 * Never route missing workspace handles to the old owner-wide Durable Object. */
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function selectWorkspaceId(
  operation: string, provided: string | undefined,
  generate: () => string = () => crypto.randomUUID(),
): string {
  if (provided === undefined && operation !== "open") {
    throw new Error("BROWSER_WORKSPACE_REQUIRED");
  }
  const id = provided ?? generate();
  if (!UUID.test(id)) throw new Error("BROWSER_WORKSPACE_INVALID");
  return id.toLowerCase();
}

export function workspaceObjectName(controllerEmail: string, workspaceId: string): string {
  const controller = controllerEmail.trim().toLowerCase();
  if (!controller || !UUID.test(workspaceId)) throw new Error("BROWSER_WORKSPACE_INVALID");
  // Keep the historical "owner:" prefix so existing controller workspaces
  // remain addressable after the allowlist becomes multi-user.
  return `owner:${controller}:workspace:${workspaceId.toLowerCase()}`;
}
