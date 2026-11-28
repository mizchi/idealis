import assert from "node:assert/strict";
import { once } from "node:events";
import test from "node:test";
import { Worker } from "node:worker_threads";
import { replay, type Scenario } from "../../examples/model-patterns/replay.js";

const sides = ["Left", "Right"] as const;
type Side = (typeof sides)[number];
type Command =
  | { readonly kind: "request" | "acquire"; readonly worker: Side }
  | { readonly kind: "release" };
type Status = { readonly waiting: number | null; readonly blocked: boolean };

test(
  "a worker can repeatedly overtake in competition but cannot pass a FIFO ticket",
  { timeout: 30_000 },
  async () => {
    const prefix: Command[] = [
      { kind: "request", worker: "Left" },
      { kind: "acquire", worker: "Left" },
      { kind: "request", worker: "Right" },
      { kind: "release" },
      { kind: "request", worker: "Left" },
    ];
    for (const fifo of [false, true]) {
      const shared = new Int32Array(
        new SharedArrayBuffer(3 * Int32Array.BYTES_PER_ELEMENT),
      );
      const makeWorker = (id: number) =>
        new Worker(
          `
        const { strict: assert } = require("node:assert");
        const { parentPort, workerData } = require("node:worker_threads");
        const { id, fifo, shared } = workerData;
        let ticket = null;
        parentPort.on("message", (kind) => {
          let blocked = false;
          if (kind === "request") {
            assert(ticket === null);
            ticket = Atomics.add(shared, 1, 1);
          } else if (kind === "acquire") {
            assert(ticket !== null);
            blocked = (fifo && ticket !== Atomics.load(shared, 2))
              || Atomics.compareExchange(shared, 0, 0, id) !== 0;
            if (!blocked) ticket = null;
          } else if (kind === "release") {
            assert.equal(Atomics.compareExchange(shared, 0, id, 0), id);
            if (fifo) Atomics.add(shared, 2, 1);
          }
          parentPort.postMessage({ waiting: ticket, blocked });
        });
        `,
          { eval: true, workerData: { id, fifo, shared } },
        );
      const workers = { Left: makeWorker(1), Right: makeWorker(2) };
      const statuses: Record<Side, Status> = {
        Left: { waiting: null, blocked: false },
        Right: { waiting: null, blocked: false },
      };
      async function send(side: Side, kind: Command["kind"]): Promise<Status> {
        const target = workers[side];
        const received = once(target, "message", {
          signal: AbortSignal.timeout(5_000),
        });
        target.postMessage(kind);
        const [value] = await received;
        return value as Status;
      }
      function owner(): Side | null {
        return sides[Atomics.load(shared, 0) - 1] ?? null;
      }
      const commands: Command[] = fifo
        ? [
            ...prefix,
            { kind: "acquire", worker: "Right" },
            { kind: "release" },
            { kind: "acquire", worker: "Left" },
          ]
        : [
            ...prefix,
            { kind: "acquire", worker: "Left" },
            { kind: "release" },
            { kind: "request", worker: "Left" },
            { kind: "acquire", worker: "Left" },
            { kind: "release" },
          ];
      const scenario: Scenario<Command> = {
        model: `examples/model-patterns/worker-fairness/${fifo ? "fifo" : "competitive"}.model`,
        config: {},
        initial: fifo
          ? { queue: { $sequence: [] }, owner: null }
          : { waiting: [], owner: null },
        commands,
      };
      try {
        await replay("crates/rcheck/target/debug/rcheck", scenario, {
          async apply(command) {
            const side = command.kind === "release" ? owner() : command.worker;
            assert.ok(side);
            statuses[side] = await send(side, command.kind);
            assert.equal(statuses[side].blocked, false);
            if (
              fifo &&
              command.kind === "request" &&
              side === "Left" &&
              statuses.Right.waiting !== null
            ) {
              const probe = await send("Left", "acquire");
              assert.equal(probe.blocked, true);
              assert.equal(probe.waiting, statuses.Left.waiting);
              assert.equal(owner(), null);
            }
          },
          async observe() {
            const waiting = sides.filter(
              (side) => statuses[side].waiting !== null,
            );
            if (fifo)
              waiting.sort(
                (left, right) =>
                  Number(statuses[left].waiting) -
                  Number(statuses[right].waiting),
              );
            return { waiting, owner: owner() };
          },
          project(state) {
            const waiting = fifo
              ? (state.queue as { $sequence: string[] }).$sequence
              : [...(state.waiting as string[])].sort();
            return { waiting, owner: state.owner };
          },
        });
        assert.equal(statuses.Right.waiting === null, fifo);
      } finally {
        await Promise.all(
          Object.values(workers).map((worker) => worker.terminate()),
        );
      }
    }
  },
);
