export type Cell = {
  readonly family: string;
  readonly length: number;
  readonly exit: number | null;
  readonly signal: string | null;
  readonly spawnError: string | null;
  readonly stdout: string;
  readonly stderr: string;
};

export type SpawnOutcome = {
  readonly status: number | null;
  readonly signal: string | null;
  readonly error?: Error | undefined;
  readonly stdout?: string | null | undefined;
  readonly stderr?: string | null | undefined;
};

export const cellFrom = (
  family: string,
  length: number,
  result: SpawnOutcome,
): Cell => ({
  family,
  length,
  exit: result.status,
  signal: result.signal,
  spawnError: result.error === undefined ? null : String(result.error),
  stdout: (result.stdout ?? "").trim(),
  stderr: (result.stderr ?? "").trim(),
});
