export type DeliveryResult = "Pending" | "Delivered" | "Missing";

interface DeliveryFrame {
  readonly kind: "notice" | "end";
  readonly id: string;
}

function parseFrame(line: string): DeliveryFrame {
  const frame: unknown = JSON.parse(line);
  if (
    typeof frame !== "object" ||
    frame === null ||
    !("id" in frame) ||
    typeof frame.id !== "string" ||
    !("kind" in frame) ||
    (frame.kind !== "notice" && frame.kind !== "end")
  ) {
    throw new Error("invalid delivery frame");
  }
  return { id: frame.id, kind: frame.kind };
}

/** Read a finite NDJSON response whose final frame closes exactly one delivery. */
export async function readDelivery(
  response: Response,
  id: string,
): Promise<DeliveryResult> {
  if (response.status !== 200)
    throw new Error(`delivery HTTP status ${response.status}`);
  if (response.bodyUsed || response.body?.locked)
    throw new Error("delivery response body is unavailable");

  let body: string;
  try {
    body = await response.text();
  } catch (error) {
    if (
      error instanceof TypeError ||
      (error instanceof DOMException && error.name === "AbortError")
    )
      return "Pending";
    throw error;
  }

  if (!body.endsWith("\n")) return "Pending";
  const frames = body.trim().split("\n").filter(Boolean).map(parseFrame);
  const end = frames.pop();
  if (frames.some((frame) => frame.kind === "end"))
    throw new Error("delivery end frame must be last");
  if (end?.kind !== "end" || end.id !== id) return "Pending";
  return frames.some((frame) => frame.id === id) ? "Delivered" : "Missing";
}

export type DeliverySettlement<Attempt extends string> =
  | { readonly outcome: "Pending" }
  | {
      readonly outcome: Exclude<DeliveryResult, "Pending">;
      readonly winner: Attempt;
      readonly conflict: boolean;
    };

/** Keep the first complete answer and surface contradictory complete evidence. */
export function settleDelivery<Attempt extends string>(
  current: DeliverySettlement<Attempt>,
  attempt: Attempt,
  outcome: DeliveryResult,
): DeliverySettlement<Attempt> {
  if (outcome === "Pending" || outcome === current.outcome) return current;
  if (current.outcome === "Pending")
    return { outcome, winner: attempt, conflict: false };
  return { ...current, conflict: true };
}

type CompleteDelivery = Exclude<DeliveryResult, "Pending">;
type VersionedReading =
  | { readonly outcome: "Pending" }
  | { readonly outcome: CompleteDelivery; readonly revision: number };

export type VersionedSettlement<Attempt extends string> = {
  readonly revision: number;
  readonly evidence: Exclude<
    DeliverySettlement<Attempt>,
    { outcome: "Pending" }
  >;
} | null;

/** The revision header identifies the snapshot used for the entire response. */
export async function readVersionedDelivery(
  response: Response,
  id: string,
): Promise<VersionedReading> {
  const outcome = await readDelivery(response, id);
  if (outcome === "Pending") return { outcome };
  const header = response.headers.get("x-delivery-revision");
  const revision = Number(header);
  if (
    header === null ||
    !/^(0|[1-9]\d*)$/.test(header) ||
    !Number.isSafeInteger(revision)
  )
    throw new Error("invalid delivery revision");
  return { outcome, revision };
}

/** Pin the first eligible version and compare only evidence from that snapshot. */
export function settleVersionedDelivery<Attempt extends string>(
  current: VersionedSettlement<Attempt>,
  minimum: number,
  attempt: Attempt,
  reading: VersionedReading,
): VersionedSettlement<Attempt> {
  if (
    reading.outcome === "Pending" ||
    reading.revision < minimum ||
    (current !== null && current.revision !== reading.revision)
  )
    return current;
  const evidence = settleDelivery(
    current?.evidence ?? { outcome: "Pending" },
    attempt,
    reading.outcome,
  );
  if (evidence.outcome === "Pending") return current;
  return { revision: reading.revision, evidence };
}

export type PinnedDeliveryResult =
  | "Retry"
  | "Expired"
  | "WrongRevision"
  | CompleteDelivery;

/** Read an exact-version request, where 410 means that the version was pruned. */
export async function readPinnedDelivery(
  response: Response,
  id: string,
  pinned: number,
): Promise<PinnedDeliveryResult> {
  if (response.status === 410 || response.status === 503) {
    await response.body?.cancel();
    return response.status === 410 ? "Expired" : "Retry";
  }
  const reading = await readVersionedDelivery(response, id);
  if (reading.outcome === "Pending") return "Retry";
  return reading.revision === pinned ? reading.outcome : "WrongRevision";
}
