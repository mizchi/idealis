import assert from "node:assert/strict";
import { once } from "node:events";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  deadlockScenario,
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const names = ["Left", "Right"] as const;
const versions = ["Original", "LeftWrite", "RightWrite"] as const;
type Command = {
  kind: "read" | "requestUpgrade" | "tryGrant" | "finish";
  worker: (typeof names)[number];
};
type Local = { basis: number | null; result: "Applied" | "Stale" | null };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/worker-locks/upgrade.model";

function startLock(t: TestContext, retainRead: boolean, skipRecheck: boolean) {
  const shared = new Int32Array(
    new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT),
  );
  const open = (index: number) => {
    const worker = new Worker(
      `
      const { parentPort, workerData: { index, shared, retainRead, skipRecheck } } = require("node:worker_threads");
      const bit = 1 << index;
      const queuedBit = bit << 2;
      const ownerBit = bit << 4;
      const readerMask = 0b000011;
      const ownerMask = 0b110000;
      const lockMask = 0b111111;
      // The low six bits encode the lock; the upper bits encode the version.
      let basis = null;
      let result = null;
      parentPort.on("message", (kind) => {
        for (;;) {
          const word = Atomics.load(shared, 0);
          const version = word >> 6;
          let next = word;
          let observedVersion = basis;
          let decision = result;
          switch (kind) {
            case "read":
              if (word & ownerMask) throw new Error("writer owns the lock");
              next |= bit;
              observedVersion = version;
              break;
            case "requestUpgrade":
              if (!(word & bit)) throw new Error("reader does not own the lock");
              next |= queuedBit;
              if (!retainRead) next &= ~bit;
              break;
            case "tryGrant":
              if (!(word & queuedBit)) throw new Error("upgrade was not requested");
              if (!(word & (ownerMask | (readerMask & ~bit)))) {
                next = (word & ~(queuedBit | bit)) | ownerBit;
              }
              break;
            case "finish":
              if (!(word & ownerBit)) throw new Error("worker does not own the writer lock");
              next &= ~ownerBit;
              if (skipRecheck || basis === version) {
                next = (next & lockMask) | ((index + 1) << 6);
                decision = "Applied";
              } else {
                decision = "Stale";
              }
              break;
          }
          if (Atomics.compareExchange(shared, 0, word, next) === word) {
            basis = observedVersion;
            result = decision;
            parentPort.postMessage({ basis, result });
            break;
          }
        }
      });
      `,
      { eval: true, workerData: { shared, index, retainRead, skipRecheck } },
    );
    t.after(() => worker.terminate());
    return worker;
  };
  const workers = { Left: open(0), Right: open(1) };
  const local: Record<Command["worker"], Local> = {
    Left: { basis: null, result: null },
    Right: { basis: null, result: null },
  };
  const decodeBasis = (value: number | null) =>
    value === null ? null : versions[value];
  return {
    async apply(command: Command) {
      const worker = workers[command.worker];
      const response = once(worker, "message");
      worker.postMessage(command.kind);
      const [state] = await response;
      local[command.worker] = state;
    },
    async observe() {
      const word = Atomics.load(shared, 0);
      return {
        readers: names.filter((_, index) => word & (1 << index)),
        queued: names.filter((_, index) => word & (4 << index)),
        owner: names.find((_, index) => word & (16 << index)) ?? null,
        current: versions[word >> 6],
        basis: {
          Left: decodeBasis(local.Left.basis),
          Right: decodeBasis(local.Right.basis),
        },
        outcome: { Left: local.Left.result, Right: local.Right.result },
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "lock promotion drops the read lock and validates the original snapshot before writing",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "BothReadSameRevision",
    );
    assert.equal(base.commands.length, 2);
    const commands: Command[] = [
      ...base.commands,
      { kind: "requestUpgrade", worker: "Left" },
      { kind: "requestUpgrade", worker: "Right" },
      { kind: "tryGrant", worker: "Left" },
      { kind: "finish", worker: "Left" },
      { kind: "tryGrant", worker: "Right" },
      { kind: "finish", worker: "Right" },
    ];
    for (const skipRecheck of [false, true]) {
      await t.test(
        skipRecheck
          ? "unchecked promotion overwrites the first write"
          : "stale promotion leaves the first write intact",
        async (t) => {
          const result = await recordReplay(
            checker,
            { ...base, commands },
            startLock(t, false, skipRecheck),
          );
          const expected = {
            readers: [],
            queued: [],
            owner: null,
            current: "LeftWrite",
            basis: { Left: "Original", Right: "Original" },
            outcome: { Left: "Applied", Right: "Stale" },
          };
          if (skipRecheck) {
            assert.equal(result.verdict, "mismatch");
            assert.deepEqual(result.difference, {
              step: 8,
              command: commands.at(-1),
              expected,
              observed: {
                ...expected,
                current: "RightWrite",
                outcome: { Left: "Applied", Right: "Applied" },
              },
            });
          } else {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(result.recording.observations.at(-1), expected);
          }
        },
      );
    }
  },
);

test(
  "retaining shared ownership when requesting promotion leaves both workers waiting",
  { timeout: 30_000 },
  async (t) => {
    const directory = await mkdtemp(join(tmpdir(), "idealis-lock-upgrade-"));
    t.after(() => rm(directory, { recursive: true, force: true }));
    const retained = join(directory, "retained.model");
    const source = await readFile(model, "utf8");
    const fault = source.replace(
      "queued = queued + worker readers = readers - worker",
      "queued = queued + worker",
    );
    assert.notEqual(fault, source);
    await writeFile(retained, fault);
    const stuck = await deadlockScenario<Command>(
      checker,
      retained,
      "EveryoneFinished",
      { ignoreStutter: true },
    );
    assert.equal(stuck.commands.length, 4);
    const result = await recordReplay(
      checker,
      {
        ...stuck,
        commands: [
          ...stuck.commands,
          { kind: "tryGrant", worker: "Left" },
          { kind: "tryGrant", worker: "Right" },
        ],
      },
      startLock(t, true, false),
    );
    assert.equal(result.verdict, "matched");
    assert.deepEqual(
      result.recording.observations.slice(-3),
      Array.from({ length: 3 }, () => ({
        readers: ["Left", "Right"],
        queued: ["Left", "Right"],
        owner: null,
        current: "Original",
        basis: { Left: "Original", Right: "Original" },
        outcome: { Left: null, Right: null },
      })),
    );
  },
);
