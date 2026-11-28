import { execFile } from "node:child_process";
import { isDeepStrictEqual, promisify } from "node:util";

export interface Scenario<Command> {
  readonly model: string;
  readonly config: Readonly<Record<string, unknown>>;
  readonly initial: Readonly<Record<string, unknown>>;
  readonly commands: readonly Command[];
  readonly derive?: readonly string[];
  readonly events?: readonly PredictedEvent[];
}

export interface PublicEvent {
  readonly event: string;
  readonly observation: Readonly<Record<string, unknown>>;
}

interface PredictedEvent extends PublicEvent {
  readonly step: number;
}

export interface ObservationPairQuery {
  readonly at: string;
  readonly observe: readonly string[];
  readonly distinguish: readonly string[];
  readonly comparison?: "snapshot" | "history";
  readonly silent?: readonly string[];
  readonly emit?: readonly {
    readonly action: string;
    readonly event: string;
    readonly when?: string;
  }[];
}

/** Convert the two traces from one model into separate scenarios. */
export async function observationPairScenarios<Command>(
  checkerPath: string,
  model: string,
  query: ObservationPairQuery,
): Promise<[Scenario<Command>, Scenario<Command>]> {
  const output = await checkerRequest(
    checkerPath,
    ["observation-pair", model, "-"],
    query,
    "model rejected observation query",
  );
  const result = JSON.parse(output) as {
    verdict: string;
    pair:
      | [Omit<Scenario<Command>, "model">, Omit<Scenario<Command>, "model">]
      | null;
  };
  if (result.verdict !== "reachable" || result.pair === null) {
    throw new Error(`no observation pair for ${query.at}: ${result.verdict}`);
  }
  const [left, right] = result.pair;
  return [
    { ...left, model },
    { ...right, model },
  ];
}

function checkerRequest(
  checkerPath: string,
  args: string[],
  request: unknown,
  failure: string,
): Promise<string> {
  const input = JSON.stringify(request);
  return new Promise((resolve, reject) => {
    const process = execFile(checkerPath, args, (error, stdout, stderr) => {
      if (error)
        reject(
          new Error(`${failure}: ${stderr.trim() || error.message}`, {
            cause: error,
          }),
        );
      else resolve(stdout);
    });
    process.stdin?.on("error", reject);
    process.stdin?.end(input);
  });
}

export interface ObservationAdapter<Command> {
  apply(command: Command): Promise<void>;
  observe(): Promise<unknown>;
  project: ObservationProjection;
}

export interface EventReplayAdapter<
  Command,
> extends ObservationAdapter<Command> {
  observeEvents(): Promise<readonly PublicEvent[]>;
}

export type ObservationProjection = (
  state: Readonly<Record<string, unknown>>,
  derived?: Readonly<Record<string, unknown>>,
) => unknown;

interface ObservationDifference<Command> {
  readonly step: number;
  readonly command: Command | null;
  readonly expected: unknown;
  readonly observed: unknown;
}

class ObservationMismatch<Command> extends Error {
  constructor(readonly difference: ObservationDifference<Command>) {
    super(`observation mismatch: ${JSON.stringify(difference)}`);
  }
}

export interface ReplayRecording<Command> {
  readonly scenario: Scenario<Command>;
  readonly observations: readonly unknown[];
}

type ReplayVerdict<Command> =
  | { readonly verdict: "matched" }
  | {
      readonly verdict: "mismatch";
      readonly difference: ObservationDifference<Command>;
    };

type RecordedReplay<Command> = ReplayVerdict<Command> & {
  readonly recording: ReplayRecording<Command>;
};

async function replayVerdict<Command>(
  run: () => Promise<void>,
): Promise<ReplayVerdict<Command>> {
  try {
    await run();
    return { verdict: "matched" };
  } catch (error) {
    if (!(error instanceof ObservationMismatch)) throw error;
    return { verdict: "mismatch", difference: error.difference };
  }
}

/** Turn a complete reachability witness into a scenario without driving the environment. */
export async function reachableScenario<Command>(
  checkerPath: string,
  model: string,
  question: string,
  options: { readonly checker?: "rprob" } = {},
): Promise<Scenario<Command>> {
  const { stdout } = await promisify(execFile)(checkerPath, [
    "witness",
    model,
    question,
    ...(options.checker === undefined ? [] : ["--checker", options.checker]),
  ]);
  return scenarioFromWitness(model, question, stdout);
}

/** Replay a shortest path whose last operation satisfies a transition predicate. */
export async function transitionScenario<Command>(
  checkerPath: string,
  model: string,
  action: string,
  predicate: string,
): Promise<Scenario<Command>> {
  const { stdout } = await promisify(execFile)(checkerPath, [
    "witness-step",
    model,
    action,
    predicate,
  ]);
  return scenarioFromWitness(model, action, stdout);
}

/** Replay a stall before completion, optionally ignoring operations that leave state unchanged. */
export async function deadlockScenario<Command>(
  checkerPath: string,
  model: string,
  unless: string,
  options: { readonly ignoreStutter?: boolean } = {},
): Promise<Scenario<Command>> {
  const args = ["deadlock", model, "--unless", unless];
  if (options.ignoreStutter) args.push("--ignore-stutter");
  try {
    await promisify(execFile)(checkerPath, args);
  } catch (error) {
    if (
      error instanceof Error &&
      "code" in error &&
      error.code === 1 &&
      "stdout" in error &&
      typeof error.stdout === "string"
    ) {
      return scenarioFromWitness(
        model,
        `deadlock before ${unless}`,
        error.stdout,
      );
    }
    throw error;
  }
  throw new Error(`no reachable deadlock before ${unless}`);
}

function scenarioFromWitness<Command>(
  model: string,
  target: string,
  stdout: string,
): Scenario<Command> {
  const result = JSON.parse(stdout) as {
    verdict: string;
    complete: boolean;
    trace: Omit<Scenario<Command>, "model"> | null;
  };
  if (
    result.verdict !== "reachable" ||
    !result.complete ||
    result.trace === null
  ) {
    throw new Error(
      `no complete reachability witness for ${target}: ${result.verdict}`,
    );
  }
  const { config, initial, commands } = result.trace;
  return { model, config, initial, commands };
}

/** Unroll an explicit temporal counterexample. Zero repetitions returns its prefix. */
export async function repeatedTemporalScenario<Command>(
  checkerPath: string,
  model: string,
  question: string,
  repetitions: number,
): Promise<Scenario<Command>> {
  if (!Number.isSafeInteger(repetitions) || repetitions < 0) {
    throw new Error(
      "temporal replay needs a nonnegative integer repetition count",
    );
  }
  const { stdout } = await promisify(execFile)(checkerPath, [
    "witness",
    model,
    question,
  ]);
  const result = JSON.parse(stdout) as {
    verdict: string;
    complete: boolean;
    trace:
      | (Omit<Scenario<Command | null>, "model"> & { loopStart: number })
      | null;
  };
  if (
    result.verdict !== "violated" ||
    !result.complete ||
    result.trace === null
  ) {
    throw new Error(
      `no complete temporal witness for ${question}: ${result.verdict}`,
    );
  }
  const { config, initial, commands, loopStart } = result.trace;
  if (
    !Number.isInteger(loopStart) ||
    loopStart < 0 ||
    loopStart >= commands.length
  ) {
    throw new Error("invalid loopStart in temporal witness");
  }
  const prefix = commands
    .slice(0, loopStart)
    .filter((command) => command !== null);
  const cycle = commands.slice(loopStart).filter((command) => command !== null);
  if (repetitions > 0 && cycle.length === 0) {
    throw new Error("temporal witness loop has no executable commands");
  }
  for (let repeat = 0; repeat < repetitions; repeat++) {
    for (const command of cycle) prefix.push(command);
  }
  return { model, config, initial, commands: prefix };
}

/** Check the model before every command, then compare the observed projection. */
export async function replay<Command>(
  checkerPath: string,
  scenario: Scenario<Command>,
  adapter: ObservationAdapter<Command>,
  record?: (observation: unknown) => void,
): Promise<void> {
  async function compare(
    step: number,
    command: Command | null,
    state: Readonly<Record<string, unknown>>,
    derived: Readonly<Record<string, unknown>>,
  ): Promise<void> {
    const expected = adapter.project(state, derived);
    const observed = await adapter.observe();
    record?.(observed);
    if (!isDeepStrictEqual(observed, expected)) {
      throw new ObservationMismatch({ step, command, expected, observed });
    }
  }

  const request = {
    config: scenario.config,
    initial: scenario.initial,
    derive: scenario.derive,
  };
  if (scenario.commands.length === 0) {
    const output = await checkerRequest(
      checkerPath,
      ["validate", scenario.model, "-"],
      request,
      "model rejected initial snapshot",
    );
    const { state, derived } = JSON.parse(output) as {
      state: Record<string, unknown>;
      derived?: Record<string, unknown>;
    };
    if (scenario.derive?.length && !derived)
      throw new Error("model response is missing derived observations");
    await compare(0, null, state, derived ?? {});
    return;
  }
  const prefix: Command[] = [];
  for (const command of scenario.commands) {
    const step = prefix.length + 1;
    const output = await checkerRequest(
      checkerPath,
      ["validate", scenario.model, "-"],
      {
        ...request,
        prefix,
        command,
      },
      `model rejected step ${step}`,
    );
    const { state, derived } = JSON.parse(output) as {
      state: Record<string, unknown>;
      derived?: {
        before: Record<string, unknown>;
        after: Record<string, unknown>;
      };
    };
    if (scenario.derive?.length && !derived)
      throw new Error("model response is missing derived observations");
    if (step === 1)
      await compare(0, null, scenario.initial, derived?.before ?? {});
    await adapter.apply(command);
    await compare(step, command, state, derived?.after ?? {});
    prefix.push(command);
  }
}

/** Snapshot observations through the first difference. Model and runtime errors propagate. */
export async function recordReplay<Command>(
  checkerPath: string,
  scenario: Scenario<Command>,
  adapter: ObservationAdapter<Command>,
): Promise<RecordedReplay<Command>> {
  const observations: unknown[] = [];
  const recording = { scenario: structuredClone(scenario), observations };
  const verdict = await replayVerdict<Command>(() =>
    replay(checkerPath, recording.scenario, adapter, (value) => {
      observations.push(structuredClone(value));
    }),
  );
  return { ...verdict, recording };
}

/** Compare cumulative runtime notifications with a model-generated history trace. */
export async function replayEvents<Command>(
  checkerPath: string,
  scenario: Scenario<Command>,
  adapter: EventReplayAdapter<Command>,
): Promise<ReplayVerdict<Command>> {
  const predicted = scenario.events;
  if (predicted === undefined)
    throw new Error("event replay needs a model event trace");
  let last = 0;
  for (const { step } of predicted) {
    if (
      !Number.isSafeInteger(step) ||
      step <= last ||
      step > scenario.commands.length
    ) {
      throw new Error(`invalid predicted event step ${step}`);
    }
    last = step;
  }
  const events: PublicEvent[] = [];
  let step = 0;
  return replayVerdict<Command>(() =>
    replay(checkerPath, scenario, {
      apply: (command) => adapter.apply(command),
      async observe() {
        return {
          state: await adapter.observe(),
          events: await adapter.observeEvents(),
        };
      },
      project(state, derived) {
        const next = predicted[events.length];
        if (next?.step === step)
          events.push({ event: next.event, observation: next.observation });
        step++;
        return { state: adapter.project(state, derived), events: [...events] };
      },
    }),
  );
}

/** Recover the verdict and first difference without constructing a runtime adapter. */
export function replayRecording<Command>(
  checkerPath: string,
  recording: ReplayRecording<Command>,
  project: ObservationProjection,
): Promise<ReplayVerdict<Command>> {
  return replayVerdict(() =>
    replayObservations(
      checkerPath,
      recording.scenario,
      recording.observations,
      project,
    ),
  );
}

/** Check recorded observations against the model without driving the environment. */
export async function replayObservations<Command>(
  checkerPath: string,
  scenario: Scenario<Command>,
  observations: readonly unknown[],
  project: ObservationProjection,
): Promise<void> {
  let step = 0;
  await replay(checkerPath, scenario, {
    async apply() {},
    async observe() {
      if (step >= observations.length) {
        throw new Error(`missing observation at step ${step}`);
      }
      return observations[step++];
    },
    project,
  });
  if (step < observations.length) {
    throw new Error(`unexpected observation at step ${step}`);
  }
}
