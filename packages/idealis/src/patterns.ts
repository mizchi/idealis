import {
  browserHistoryAdapter,
  type BrowserHistory,
  type Navigation,
  type Screen,
} from "./patterns/browser/adapter.js";
import {
  type DrainIoBatch,
  type IoBatch,
  ioBatchAdapter,
  type NextTickCallback,
  nextTickProjection,
} from "./patterns/node-event-loop/adapter.js";
import {
  type ObjectCommand,
  type ObjectStore,
  objectStoreAdapter,
} from "./patterns/object-store/adapter.js";
import type { ObservationAdapter, Scenario } from "./patterns/replay.js";

export type { BrowserHistory, Screen, IoBatch, ObjectStore };
export type {
  ObservationAdapter,
  ObservationProjection,
  ReplayRecording,
  Scenario,
} from "./patterns/replay.js";
export {
  recordReplay,
  repeatedTemporalScenario,
  replay,
  replayRecording,
} from "./patterns/replay.js";
export { fromReplayItf, toReplayItf } from "./patterns/replay-itf.js";
export {
  s3ObjectStore,
  type S3ObjectClient,
} from "./patterns/object-store/s3.js";
export {
  nodeFileIoBatch,
  nodeFileNextTickBatch,
} from "./patterns/node-event-loop/file-batch.js";
export {
  browserHistoryAdapter,
  browserScreenProjection,
  type Navigation,
} from "./patterns/browser/adapter.js";
export {
  ioBatchAdapter,
  ioBatchProjection,
  nextTickProjection,
  type DrainIoBatch,
  type IoCallback,
  type NextTickCallback,
} from "./patterns/node-event-loop/adapter.js";
export {
  objectStoreAdapter,
  objectStoreProjection,
  type ObjectCommand,
} from "./patterns/object-store/adapter.js";

export interface ConnectedPattern<Command> {
  readonly scenario: Scenario<Command>;
  readonly adapter: ObservationAdapter<Command>;
}

type HistoryRoute = "Home" | "Catalog" | "Detail";

function bundledModel(path: string): string {
  return fileURLToPath(
    import.meta.resolve(`@mizchi/idealis/patterns/models/${path}`),
  );
}

/** Start on Home, traverse the history, then replace its forward entry. */
export function browserHistoryPattern(
  history: BrowserHistory,
  routes: Readonly<Record<HistoryRoute, Screen>>,
  model = bundledModel("browser/project.rmodel.json"),
): ConnectedPattern<Navigation<HistoryRoute>> {
  return {
    scenario: {
      model,
      config: { start: "Home" },
      initial: { current: "Home", nextRoute: null, atNext: false },
      commands: [
        { kind: "push", destination: "Catalog" },
        { kind: "goBack" },
        { kind: "goForward" },
        { kind: "goBack" },
        { kind: "push", destination: "Detail" },
        { kind: "goBack" },
        { kind: "goForward" },
      ],
    },
    adapter: browserHistoryAdapter(history, routes),
  };
}

/** Observe the order of three callbacks scheduled inside a single I/O callback. */
export function nodeIoBatchPattern(
  batch: IoBatch,
  model = bundledModel("node-event-loop/io-batch.model"),
): ConnectedPattern<DrainIoBatch> {
  return {
    scenario: ioBatchScenario(model, { calls: { $sequence: [] } }),
    adapter: ioBatchAdapter(batch),
  };
}

/** Observe the two queue drains around next ticks registered from I/O and a microtask. */
export function nodeNextTickPattern(
  batch: IoBatch<NextTickCallback>,
  model = bundledModel("node-event-loop/next-tick.model"),
): ConnectedPattern<DrainIoBatch> {
  return {
    scenario: ioBatchScenario(model, {
      callbackAt: { First: null, Second: null, Third: null, Fourth: null },
    }),
    adapter: ioBatchAdapter(batch, nextTickProjection),
  };
}

function ioBatchScenario(
  model: string,
  initial: Scenario<DrainIoBatch>["initial"],
): Scenario<DrainIoBatch> {
  return {
    model,
    config: {},
    initial,
    commands: [{ kind: "drainIoBatch" }],
  };
}

/** Write, overwrite, and delete a caller-supplied, initially absent test key. */
export function objectStorePattern(
  store: ObjectStore,
  testKey: string,
  model = bundledModel("object-store/project.rmodel.json"),
): ConnectedPattern<ObjectCommand> {
  return {
    scenario: {
      model,
      config: {},
      initial: { object: null },
      commands: [
        { kind: "put", body: "V1" },
        { kind: "put", body: "V2" },
        { kind: "remove" },
      ],
    },
    adapter: objectStoreAdapter(store, testKey),
  };
}
import { fileURLToPath } from "node:url";
