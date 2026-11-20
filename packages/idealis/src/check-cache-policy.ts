export interface CachePolicyState {
  readonly identity: string;
  readonly fence: number;
  readonly phase: "idle" | "leased" | "ready";
  readonly owner: string;
  readonly expiresAt: number;
  readonly storedIdentity: string;
  readonly complete: boolean;
}

export interface CachePolicyAction {
  readonly op: string;
  readonly identity: string;
  readonly owner: string;
  readonly fence: number;
  readonly now: number;
  readonly duration: number;
  readonly receiptIdentity: string;
  readonly complete: boolean;
}

export const advanceCachePolicy = (
  state: CachePolicyState,
  action: CachePolicyAction,
) => {
  const cleared: CachePolicyState = {
    identity: state.identity,
    fence: state.fence,
    phase: "idle",
    owner: "",
    expiresAt: 0,
    storedIdentity: "",
    complete: false,
  };
  const holds =
    state.phase === "leased" &&
    action.identity === state.identity &&
    action.owner === state.owner &&
    action.fence === state.fence;
  const live = holds && action.now < state.expiresAt;
  if (action.identity !== state.identity) return { state, code: "mismatch" };
  switch (action.op) {
    case "probe":
    case "acquire": {
      if (state.phase === "ready") return { state, code: "hit" };
      if (state.phase === "leased" && action.now < state.expiresAt)
        return { state, code: "busy" };
      if (action.op === "probe") return { state, code: "miss" };
      if (action.duration === 0) return { state, code: "invalid" };
      return {
        state: {
          ...cleared,
          fence: state.fence + 1,
          phase: "leased" as const,
          owner: action.owner,
          expiresAt: action.now + action.duration,
        },
        code: "lease",
      };
    }
    case "renew":
      return live && action.duration > 0
        ? {
            state: {
              ...state,
              expiresAt: Math.max(
                state.expiresAt,
                action.now + action.duration,
              ),
            },
            code: "renewed",
          }
        : { state, code: "stale" };
    case "publish":
      if (!live) return { state, code: "stale" };
      if (action.receiptIdentity !== state.identity)
        return { state, code: "mismatch" };
      if (!action.complete) return { state: cleared, code: "uncacheable" };
      return {
        state: {
          ...state,
          phase: "ready" as const,
          owner: "",
          expiresAt: 0,
          storedIdentity: action.receiptIdentity,
          complete: true,
        },
        code: "published",
      };
    case "release":
      return holds
        ? { state: cleared, code: "released" }
        : { state, code: "stale" };
    case "evict":
      return state.phase === "ready"
        ? { state: cleared, code: "evicted" }
        : { state, code: "miss" };
    default:
      return { state, code: "invalid" };
  }
};
