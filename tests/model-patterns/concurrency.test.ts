import assert from "node:assert/strict";
import { once } from "node:events";
import test from "node:test";
import { Worker } from "node:worker_threads";
import {
  deadlockScenario,
  reachableScenario,
  replay,
  type Scenario,
} from "../../examples/model-patterns/replay.js";

const checkerPath = "crates/idealis-check/target/debug/idealis-check";
const patterns = "examples/model-patterns";

type QueueCommand =
  | { kind: "enqueue"; message: "A" | "B" }
  | { kind: "dequeue" };

test("a mailbox keeps order and duplicates", async () => {
  const scenario: Scenario<QueueCommand> = {
    model: `${patterns}/fifo/model.project.json`,
    config: {},
    initial: { queue: { $sequence: [] }, delivered: null },
    commands: [
      { kind: "enqueue", message: "B" },
      { kind: "enqueue", message: "A" },
      { kind: "enqueue", message: "A" },
      { kind: "dequeue" },
      { kind: "dequeue" },
      { kind: "dequeue" },
    ],
  };
  for (const lifo of [false, true]) {
    const queue: string[] = [];
    let delivered: string | null = null;
    const run = replay(checkerPath, scenario, {
      async apply(command) {
        if (command.kind === "enqueue") queue.push(command.message);
        else delivered = (lifo ? queue.pop() : queue.shift()) ?? null;
      },
      async observe() {
        return { queue: { $sequence: [...queue] }, delivered };
      },
      project(state) {
        return { queue: state.queue, delivered: state.delivered };
      },
    });
    if (lifo)
      await assert.rejects(
        run,
        /observation mismatch: .*"step":4.*"delivered":"B".*"delivered":"A"/,
      );
    else await run;
  }
});

type Request = "Old" | "New";
type SearchCommand = {
  kind: "start" | "resolve";
  request: Request;
};

test("an older promise completing late leaves the latest search result visible", async () => {
  const scenario: Scenario<SearchCommand> = {
    model: `${patterns}/latest-request/search.ideal`,
    config: {},
    initial: { latest: null, pending: [], shown: null },
    commands: [
      { kind: "start", request: "Old" },
      { kind: "start", request: "New" },
      { kind: "resolve", request: "New" },
      { kind: "resolve", request: "Old" },
    ],
  };
  for (const allowStale of [false, true]) {
    let generation = 0;
    let shown: Request | null = null;
    const pending = new Map<
      Request,
      { resolve: () => void; consumed: Promise<void> }
    >();
    const run = replay(checkerPath, scenario, {
      async apply(command) {
        const { request } = command;
        if (command.kind === "start") {
          const started = ++generation;
          let resolve!: () => void;
          const consumed = new Promise<void>((done) => {
            resolve = done;
          }).then(() => {
            if (allowStale || started === generation) shown = request;
          });
          pending.set(request, { resolve, consumed });
        } else {
          const response = pending.get(request);
          assert.ok(response);
          response.resolve();
          await response.consumed;
          pending.delete(request);
        }
      },
      async observe() {
        return { shown, pending: [...pending.keys()].sort() };
      },
      project(state) {
        return {
          shown: state.shown,
          pending: [...(state.pending as string[])].sort(),
        };
      },
    });
    if (allowStale)
      await assert.rejects(
        run,
        /observation mismatch: .*"step":4.*"shown":"New".*"shown":"Old"/,
      );
    else await run;
  }
});

type WorkerCommand = { kind: "load" | "commit"; worker: "Left" | "Right" };

test(
  "two workers loading before either commit cannot lose an increment",
  { timeout: 30_000 },
  async () => {
    const scenario: Scenario<WorkerCommand> = {
      model: `${patterns}/worker-counter/counter.ideal`,
      config: {},
      initial: { value: 0, loaded: { Left: null, Right: null }, finished: [] },
      commands: [
        { kind: "load", worker: "Left" },
        { kind: "load", worker: "Right" },
        { kind: "commit", worker: "Left" },
        { kind: "commit", worker: "Right" },
      ],
    };
    for (const staleStore of [false, true]) {
      const shared = new Int32Array(new SharedArrayBuffer(4));
      const worker = () =>
        new Worker(
          `
      const { parentPort, workerData } = require("node:worker_threads");
      const shared = workerData.shared;
      let loaded;
      parentPort.on("message", kind => {
        if (kind === "load") loaded = Atomics.load(shared, 0);
        else if (workerData.staleStore) Atomics.store(shared, 0, loaded + 1);
        else Atomics.add(shared, 0, 1);
        parentPort.postMessage(kind === "load" ? loaded : null);
      });
    `,
          { eval: true, workerData: { shared, staleStore } },
        );
      const workers = { Left: worker(), Right: worker() };
      const loaded: Record<WorkerCommand["worker"], number | null> = {
        Left: null,
        Right: null,
      };
      try {
        const run = replay(checkerPath, scenario, {
          async apply(command) {
            const target = workers[command.worker];
            const acknowledged = once(target, "message");
            target.postMessage(command.kind);
            const [value] = await acknowledged;
            assert.ok(value === null || typeof value === "number");
            loaded[command.worker] = value;
          },
          async observe() {
            return { value: Atomics.load(shared, 0), loaded: { ...loaded } };
          },
          project(state) {
            return { value: state.value, loaded: state.loaded };
          },
        });
        if (staleStore)
          await assert.rejects(
            run,
            /observation mismatch: .*"step":4.*"expected":\{"value":2.*"observed":\{"value":1/,
          );
        else await run;
      } finally {
        await Promise.all(
          Object.values(workers).map((running) => running.terminate()),
        );
      }
    }
  },
);

type Completion = { kind: "cancel" | "complete" };

test("cancellation and completion preserve the first terminal outcome", async () => {
  for (const commands of [
    [{ kind: "cancel" }, { kind: "complete" }],
    [{ kind: "complete" }, { kind: "cancel" }],
  ] satisfies Completion[][]) {
    const scenario: Scenario<Completion> = {
      model: `${patterns}/cancellation/request.ideal`,
      config: {},
      initial: { pending: true, outcome: null },
      commands,
    };
    for (const acceptLate of [false, true]) {
      const controller = new AbortController();
      let outcome: "Cancelled" | "Done" | null = null;
      let pending = true;
      let resolve!: () => void;
      const completed = new Promise<void>((done) => {
        resolve = done;
      }).then(() => {
        pending = false;
        if (acceptLate || !controller.signal.aborted) outcome = "Done";
      });
      controller.signal.addEventListener("abort", () => {
        if (outcome === null) outcome = "Cancelled";
      });
      const run = replay(checkerPath, scenario, {
        async apply(command) {
          if (command.kind === "cancel") controller.abort();
          else {
            resolve();
            await completed;
          }
        },
        async observe() {
          return { outcome, pending };
        },
        project(state) {
          return { outcome: state.outcome, pending: state.pending };
        },
      });
      if (acceptLate && commands[0]?.kind === "cancel")
        await assert.rejects(
          run,
          /observation mismatch: .*"step":2.*"outcome":"Cancelled".*"outcome":"Done"/,
        );
      else await run;
    }
  }
});

test(
  "workers with reverse lock order can reach a circular wait",
  { timeout: 30_000 },
  async () => {
    type Side = "Left" | "Right";
    type Lock = "Alpha" | "Beta";
    type Command = { kind: "acquire" | "release"; worker: Side };
    interface Status {
      pending: Lock | null;
      blockedBy: Side | null;
      finished: boolean;
    }
    const lockNames = ["Alpha", "Beta"] as const;
    const workerNames = ["Left", "Right"] as const;
    for (const reversed of [false, true]) {
      const shared = new Int32Array(new SharedArrayBuffer(8));
      const config: { first: Record<Side, Lock> } = {
        first: { Left: "Alpha", Right: reversed ? "Beta" : "Alpha" },
      };
      const model = `${patterns}/worker-locks/${reversed ? "reversed" : "ordered"}.project.json`;
      const scenario = reversed
        ? await deadlockScenario<Command>(checkerPath, model, "BothFinished")
        : await reachableScenario<Command>(checkerPath, model, "BothFinished");
      assert.deepEqual(scenario.config, config);
      const make = (side: Side) =>
        new Worker(
          `
        const { strict: assert } = require("node:assert");
        const { parentPort, workerData } = require("node:worker_threads");
        const { shared, order, id } = workerData;
        let cursor = 0;
        let finished = false;
        parentPort.on("message", (kind) => {
          let blockedBy = 0;
          if (kind === "acquire") {
            assert(cursor < order.length);
            blockedBy = Atomics.compareExchange(shared, order[cursor], 0, id);
            if (!blockedBy) cursor += 1;
          } else if (kind === "release") {
            assert(cursor === order.length && !finished);
            for (const lock of order) assert.equal(Atomics.exchange(shared, lock, 0), id);
            finished = true;
          }
          parentPort.postMessage({
            pending: ["Alpha", "Beta"][order[cursor]] ?? null,
            blockedBy: ["Left", "Right"][blockedBy - 1] ?? null,
            finished,
          });
        });
        `,
          {
            eval: true,
            workerData: {
              shared,
              order: config.first[side] === "Alpha" ? [0, 1] : [1, 0],
              id: workerNames.indexOf(side) + 1,
            },
          },
        );
      const workers = { Left: make("Left"), Right: make("Right") };
      async function request(
        side: Side,
        kind: Command["kind"] | "status",
      ): Promise<Status> {
        const worker = workers[side];
        const acknowledged = once(worker, "message");
        worker.postMessage(kind);
        const [status] = await acknowledged;
        return status as Status;
      }
      try {
        const current = {
          Left: await request("Left", "status"),
          Right: await request("Right", "status"),
        };
        await replay(checkerPath, scenario, {
          async apply(command) {
            current[command.worker] = await request(
              command.worker,
              command.kind,
            );
            assert.equal(current[command.worker].blockedBy, null);
          },
          async observe() {
            return {
              owner: Object.fromEntries(
                lockNames.map((lock, index) => [
                  lock,
                  workerNames[Atomics.load(shared, index) - 1] ?? null,
                ]),
              ),
              pending: {
                Left: current.Left.pending,
                Right: current.Right.pending,
              },
              finished: workerNames.filter((side) => current[side].finished),
            };
          },
          project(state) {
            return {
              owner: state.owner,
              pending: state.pending,
              finished: [...(state.finished as Side[])].sort(),
            };
          },
        });
        if (reversed) {
          const [left, right] = await Promise.all([
            request("Left", "acquire"),
            request("Right", "acquire"),
          ]);
          assert.deepEqual(left, {
            pending: "Beta",
            blockedBy: "Right",
            finished: false,
          });
          assert.deepEqual(right, {
            pending: "Alpha",
            blockedBy: "Left",
            finished: false,
          });
          assert.deepEqual([...shared], [1, 2]);
        }
      } finally {
        await Promise.all(
          Object.values(workers).map((worker) => worker.terminate()),
        );
      }
    }
  },
);
