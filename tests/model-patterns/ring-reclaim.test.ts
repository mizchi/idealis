import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = {
  kind:
    | "timeout"
    | "confirmStop"
    | "reclaim"
    | "tryAssign"
    | "resume"
    | "finishOriginal"
    | "lateRelease";
};
type Fault = "none" | "reclaim-on-timeout" | "untagged-release";
type Status = "Copying" | "TimedOut" | "Stopped" | "Finished";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/worker-deque/ring-reclaim.model";

function openRing(t: TestContext, fault: Fault) {
  // Owner, header word, data word. The original reader starts with the slot pinned.
  const words = new Int32Array(
    new SharedArrayBuffer(3 * Int32Array.BYTES_PER_ELEMENT),
  );
  words.set([1, 11, 12]);
  const createWorker = (role: "original" | "replacement") => {
    const worker = new Worker(
      `
      const { parentPort, workerData } = require("node:worker_threads");
      const { words, role } = workerData;
      const header = role === "original" ? Atomics.load(words, 1) : 0;
      let data = 0;
      parentPort.on("message", (kind) => {
        if (kind === "tryAssign") {
          if (Atomics.compareExchange(words, 0, 0, 2) === 0) {
            Atomics.store(words, 1, 21);
            Atomics.store(words, 2, 22);
          }
          parentPort.postMessage(null);
        } else if (kind === "resume") {
          data = Atomics.load(words, 2);
          parentPort.postMessage([header, data]);
        } else if (kind === "finishOriginal") {
          Atomics.compareExchange(words, 0, 1, 0);
          parentPort.postMessage([header, data]);
        }
      });
      parentPort.postMessage(header);
    `,
      { eval: true, workerData: { words, role } },
    );
    t.after(() => worker.terminate());
    return worker;
  };
  const original = createWorker("original");
  const originalReady = once(original, "message");
  const replacement = createWorker("replacement");
  const ready = Promise.all([originalReady, once(replacement, "message")]);
  let status: Status = "Copying";
  let readBack: "Original" | "Replacement" | null = null;
  const releaseOriginal = () => Atomics.compareExchange(words, 0, 1, 0);

  return {
    async apply({ kind }: Command) {
      assert.deepEqual(await ready, [[11], [0]]);
      if (kind === "timeout") {
        status = "TimedOut";
        if (fault === "reclaim-on-timeout") releaseOriginal();
      } else if (kind === "confirmStop") {
        const exit = once(original, "exit");
        const code = await original.terminate();
        assert.deepEqual(await exit, [code]);
        assert.notEqual(code, 0);
        status = "Stopped";
      } else if (kind === "reclaim") {
        assert.equal(status, "Stopped");
        releaseOriginal();
      } else if (kind === "lateRelease") {
        if (fault === "untagged-release") Atomics.store(words, 0, 0);
        else releaseOriginal();
      } else {
        const worker = kind === "tryAssign" ? replacement : original;
        const result = once(worker, "message");
        worker.postMessage(kind);
        const [reply] = await result;
        if (kind === "resume" || kind === "finishOriginal") {
          assert.ok(Array.isArray(reply));
          assert.equal(reply[0], 11);
          assert.ok(reply[1] === 12 || reply[1] === 22);
          readBack = reply[1] === 12 ? "Original" : "Replacement";
          status = kind === "resume" ? "Copying" : "Finished";
        }
      }
    },
    async observe() {
      const owner = [null, "Original", "Replacement"][Atomics.load(words, 0)];
      const payload =
        Atomics.load(words, 2) === 12 ? "Original" : "Replacement";
      return { owner, payload, status, readBack };
    },
  };
}

test(
  "a timed-out reader keeps its slot until it finishes or exits",
  { timeout: 30_000 },
  async (t) => {
    const timeout = await reachableScenario<Command>(
      checker,
      model,
      "TimedOutButStillOwned",
    );
    const stopped = await reachableScenario<Command>(
      checker,
      model,
      "StoppedButStillOwned",
    );
    assert.deepEqual(timeout.commands, [{ kind: "timeout" }]);
    assert.deepEqual(stopped.commands, [
      { kind: "timeout" },
      { kind: "confirmStop" },
    ]);
    const resumes: readonly Command[] = [
      { kind: "tryAssign" },
      { kind: "resume" },
      { kind: "finishOriginal" },
      { kind: "tryAssign" },
    ];
    const reclaims: readonly Command[] = [
      { kind: "reclaim" },
      { kind: "tryAssign" },
      { kind: "lateRelease" },
    ];
    for (const [base, tail, fault] of [
      [timeout, resumes, "none"],
      [timeout, resumes, "reclaim-on-timeout"],
      [stopped, reclaims, "none"],
      [stopped, reclaims, "untagged-release"],
    ] as const) {
      await t.test(
        `${base === timeout ? "resume" : "stop"} / ${fault}`,
        async (t) => {
          const ring = openRing(t, fault);
          const result = await recordReplay(
            checker,
            { ...base, commands: [...base.commands, ...tail] },
            {
              ...ring,
              project: ({ owner, payload, status, readBack }) => ({
                owner,
                payload,
                status,
                readBack,
              }),
            },
          );
          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(await ring.observe(), {
              owner: "Replacement",
              payload: "Replacement",
              status: base === timeout ? "Finished" : "Stopped",
              readBack: base === timeout ? "Original" : null,
            });
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.equal(
              result.difference.step,
              fault === "reclaim-on-timeout" ? 1 : 5,
            );
            if (fault === "reclaim-on-timeout") {
              for (const command of resumes.slice(0, 3))
                await ring.apply(command);
              assert.deepEqual(await ring.observe(), {
                owner: "Replacement",
                payload: "Replacement",
                status: "Finished",
                readBack: "Replacement",
              });
            } else {
              assert.deepEqual(await ring.observe(), {
                owner: null,
                payload: "Replacement",
                status: "Stopped",
                readBack: null,
              });
            }
          }
        },
      );
    }
  },
);
