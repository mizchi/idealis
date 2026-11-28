/// <reference lib="es2024.sharedmemory" />
import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

type Limit = "Poll" | "Finite" | "Forever";
type Count = "Zero" | "One";
type Wait =
  | "Idle"
  | "Waiting"
  | "Mismatch"
  | "PollTimeout"
  | "AsyncTimeout"
  | "Notified";
type Command =
  | { kind: "publish" | "expire" }
  | { kind: "wait"; timeout: Limit }
  | { kind: "notify"; count: Count };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/wait-notify/wait-async-timeout.ideal";
const duration = { Poll: 0, Finite: 10, Forever: Infinity };

function openWait(t: TestContext, checkTimeoutFirst = false) {
  const shared = new Int32Array(
    new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT),
  );
  const worker = new Worker(
    `
const { parentPort, workerData: shared } = require("node:worker_threads");
parentPort.on("message", (count) => {
  if (count === "publish") {
    Atomics.store(shared, 0, 1);
    parentPort.postMessage("published");
  } else {
    parentPort.postMessage(Atomics.notify(shared, 0, count));
  }
});
`,
    { eval: true, workerData: shared },
  );
  t.after(async () => {
    await worker.terminate();
  });
  let result: Wait = "Idle";
  let limit: Limit | null = null;
  let wakeCount: Count | null = null;
  let pending: Promise<string> | null = null;

  async function askWorker(input: number | "publish"): Promise<unknown> {
    const answer = once(worker, "message", {
      signal: AbortSignal.timeout(5_000),
    });
    worker.postMessage(input);
    const [reply] = await answer;
    return reply;
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "publish":
          assert.equal(await askWorker("publish"), "published");
          break;
        case "wait": {
          limit = command.timeout;
          const report =
            checkTimeoutFirst && limit === "Poll"
              ? { async: false as const, value: "timed-out" }
              : Atomics.waitAsync(shared, 0, 0, duration[limit]);
          if (report.async) {
            pending = report.value;
            result = "Waiting";
          } else {
            assert.ok(
              report.value === "not-equal" || report.value === "timed-out",
            );
            result = report.value === "not-equal" ? "Mismatch" : "PollTimeout";
          }
          break;
        }
        case "expire":
          assert.ok(pending);
          assert.equal(await pending, "timed-out");
          pending = null;
          result = "AsyncTimeout";
          break;
        case "notify": {
          const count = await askWorker(command.count === "One" ? 1 : 0);
          assert.ok(count === 0 || count === 1);
          wakeCount = count === 1 ? "One" : "Zero";
          if (count === 1) {
            assert.ok(pending);
            assert.equal(await pending, "ok");
            pending = null;
            result = "Notified";
          }
          break;
        }
      }
    },
    async observe() {
      return {
        changed: Atomics.load(shared, 0) !== 0,
        limit,
        result,
        wakeCount,
      };
    },
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test(
  "waitAsync compares before polling and removes finite waiters at expiry",
  { timeout: 20_000 },
  async (t) => {
    const changed = await reachableScenario<Command>(
      checker,
      model,
      "AlreadyChanged",
      { checker: "rprob" },
    );
    const finite = await reachableScenario<Command>(
      checker,
      model,
      "FiniteWaitRegistered",
      { checker: "rprob" },
    );
    const infinite = await reachableScenario<Command>(
      checker,
      model,
      "InfiniteWaitRegistered",
      { checker: "rprob" },
    );
    const cases = [
      {
        name: "zero timeout still distinguishes an already changed value",
        prefix: changed,
        suffix: [
          { kind: "wait", timeout: "Poll" },
          { kind: "notify", count: "One" },
        ],
        expected: {
          changed: true,
          limit: "Poll",
          result: "Mismatch",
          wakeCount: "Zero",
        },
      },
      {
        name: "zero timeout on an equal value does not register a waiter",
        prefix: { ...changed, commands: [] },
        suffix: [
          { kind: "wait", timeout: "Poll" },
          { kind: "notify", count: "One" },
        ],
        expected: {
          changed: false,
          limit: "Poll",
          result: "PollTimeout",
          wakeCount: "Zero",
        },
      },
      {
        name: "a finite timeout registers, resolves asynchronously and leaves no waiter",
        prefix: finite,
        suffix: [{ kind: "expire" }, { kind: "notify", count: "One" }],
        expected: {
          changed: false,
          limit: "Finite",
          result: "AsyncTimeout",
          wakeCount: "Zero",
        },
      },
      {
        name: "zero-count notification leaves the indefinite wait registered",
        prefix: infinite,
        suffix: [
          { kind: "notify", count: "Zero" },
          { kind: "notify", count: "One" },
        ],
        expected: {
          changed: false,
          limit: "Forever",
          result: "Notified",
          wakeCount: "One",
        },
      },
    ] as const;
    for (const item of cases) {
      await t.test(item.name, async (t) => {
        const driver = openWait(t);
        const commands: Command[] = [...item.prefix.commands, ...item.suffix];
        const replay = await recordReplay(
          checker,
          { ...item.prefix, commands },
          driver,
        );
        assert.equal(replay.verdict, "matched");
        assert.deepEqual(await driver.observe(), item.expected);
      });
    }
    await t.test(
      "checking the deadline first loses an already changed value",
      async (t) => {
        const driver = openWait(t, true);
        const replay = await recordReplay(
          checker,
          {
            ...changed,
            commands: [...changed.commands, { kind: "wait", timeout: "Poll" }],
          },
          driver,
        );
        assert.equal(replay.verdict, "mismatch");
        assert.equal(replay.difference.step, changed.commands.length + 1);
        assert.partialDeepStrictEqual(replay.difference.expected, {
          changed: true,
          result: "Mismatch",
        });
        assert.partialDeepStrictEqual(replay.difference.observed, {
          changed: true,
          result: "PollTimeout",
        });
        const trace = JSON.parse(JSON.stringify(toReplayItf(replay.recording)));
        assert.deepEqual(
          await replayRecording(checker, fromReplayItf(trace), driver.project),
          {
            verdict: "mismatch",
            difference: replay.difference,
          },
        );
      },
    );
  },
);
