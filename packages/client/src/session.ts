export type SessionState =
  | Readonly<{ status: "unknown" }>
  | Readonly<{ status: "unauthenticated" }>
  | Readonly<{
      status: "authenticated";
      bearerToken: string;
      tenantId: string;
      actorLabel?: string;
      tenantLabel?: string;
      expiresAtUnixMillis?: number;
    }>
  | Readonly<{ status: "expired" }>
  | Readonly<{ status: "revoked" }>;

export interface SessionProvider {
  getSnapshot(): SessionState;
  subscribe(listener: () => void): () => void;
}

export class MutableSessionStore implements SessionProvider {
  private state: SessionState;
  private readonly listeners = new Set<() => void>();

  public constructor(initialState: SessionState = { status: "unknown" }) {
    this.state = initialState;
  }

  public getSnapshot(): SessionState {
    return this.state;
  }

  public setState(nextState: SessionState): void {
    if (Object.is(this.state, nextState)) {
      return;
    }
    this.state = nextState;
    for (const listener of this.listeners) {
      listener();
    }
  }

  public clearProtectedState(reason: "logout" | "expired" | "revoked"): void {
    const nextState: SessionState =
      reason === "logout" ? { status: "unauthenticated" } : { status: reason };
    this.setState(nextState);
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
}

export function requireAuthenticatedSession(
  state: SessionState,
): Extract<SessionState, { status: "authenticated" }> {
  if (state.status !== "authenticated") {
    throw new SessionUnavailableError(state.status);
  }
  if (state.expiresAtUnixMillis !== undefined && state.expiresAtUnixMillis <= Date.now()) {
    throw new SessionUnavailableError("expired");
  }
  return state;
}

export class SessionUnavailableError extends Error {
  public readonly sessionStatus: Exclude<SessionState["status"], "authenticated">;

  public constructor(
    sessionStatus: Exclude<SessionState["status"], "authenticated">,
  ) {
    super(`A governed request requires an authenticated session; current state is ${sessionStatus}.`);
    this.name = "SessionUnavailableError";
    this.sessionStatus = sessionStatus;
  }
}
