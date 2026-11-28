import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const actors = ["Owner", "Thief"] as const;
const tasks = ["Front", "Back"] as const;
type Actor = (typeof actors)[number];
type Task = (typeof tasks)[number];
type Command = { kind: "snapshot" | "claim"; actor: Actor };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/worker-deque/last-task.ideal";

function setup(t: TestContext, skipCas: boolean) {
  const slots = new Int32Array(
    new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * tasks.length),
  );
  slots.fill(1);
  const start = (actor: Actor) => {
    const worker = new Worker(
      `
      const { parentPort, workerData } = require("node:worker_threads");
      const { slots, preferred, skipCas } = workerData;
      let candidate = -1;
      parentPort.on("message", kind => {
        if (kind === "snapshot") {
          const other = 1 - preferred;
          candidate = Atomics.load(slots, preferred) === 1 ? preferred
            : Atomics.load(slots, other) === 1 ? other : -1;
          parentPort.postMessage(candidate);
          return;
        }
        let claimed = -1;
        if (candidate !== -1) {
          if (skipCas) {
            Atomics.store(slots, candidate, 0);
            claimed = candidate;
          } else if (Atomics.compareExchange(slots, candidate, 1, 0) === 1) {
            claimed = candidate;
          }
        }
        candidate = -1;
        parentPort.postMessage(claimed);
      });
    `,
      {
        eval: true,
        workerData: { slots, preferred: actor === "Owner" ? 1 : 0, skipCas },
      },
    );
    t.after(() => worker.terminate());
    return worker;
  };
  const peers = { Owner: start("Owner"), Thief: start("Thief") };
  const pending = new Map<Actor, Task | null>();
  const claimed: Record<Task, Actor[]> = { Front: [], Back: [] };
  return {
    async apply({ kind, actor }: Command) {
      const worker = peers[actor];
      const response = once(worker, "message");
      worker.postMessage(kind);
      const [index] = await response;
      assert.ok(index === -1 || index === 0 || index === 1);
      const task = tasks[index] ?? null;
      if (kind === "snapshot") pending.set(actor, task);
      else {
        pending.delete(actor);
        if (task !== null) claimed[task].push(actor);
      }
      return task;
    },
    async observe() {
      return {
        queued: tasks
          .filter((_, index) => Atomics.load(slots, index) === 1)
          .sort(),
        pending: [...pending.keys()].sort(),
        candidate: {
          Owner: pending.get("Owner") ?? null,
          Thief: pending.get("Thief") ?? null,
        },
        claimed: {
          Front: [...claimed.Front].sort(),
          Back: [...claimed.Back].sort(),
        },
      };
    },
  };
}

test(
  "owner and thief share the last task without claiming it twice",
  { timeout: 30_000 },
  async (t) => {
    const last = await reachableScenario<Command>(
      checker,
      model,
      "BothReadLastTask",
    );
    const different = await reachableScenario<Command>(
      checker,
      model,
      "ReadOppositeEnds",
    );
    assert.equal(last.commands.length, 4);
    assert.equal(different.commands.length, 2);
    for (const first of actors) {
      for (const [base, skipCas] of [
        [last, false],
        [last, true],
        [different, false],
      ] as const) {
        await t.test(
          `${base === last ? "last task" : "opposite ends"} / ${first} first / ${skipCas ? "unchecked" : "CAS"}`,
          async (t) => {
            const deque = setup(t, skipCas);
            const second = first === "Owner" ? "Thief" : "Owner";
            const commands: Command[] = [
              ...base.commands,
              { kind: "claim", actor: first },
              { kind: "claim", actor: second },
              { kind: "snapshot", actor: first },
              { kind: "claim", actor: first },
            ];
            const result = await recordReplay(
              checker,
              { ...base, commands },
              {
                async apply(command) {
                  await deque.apply(command);
                },
                observe: deque.observe,
                project: ({ queued, pending, candidate, claimed }) => ({
                  queued,
                  pending,
                  candidate,
                  claimed,
                }),
              },
            );
            const beforeClaims = result.recording.observations[
              base.commands.length
            ] as Awaited<ReturnType<typeof deque.observe>>;
            const chosen = beforeClaims.candidate[first];
            assert.ok(chosen);
            assert.deepEqual(beforeClaims.pending, actors);
            if (base === last) {
              assert.deepEqual(beforeClaims.queued, [chosen]);
              assert.equal(beforeClaims.candidate[second], chosen);
            } else {
              assert.equal(beforeClaims.candidate.Owner, "Back");
              assert.equal(beforeClaims.candidate.Thief, "Front");
            }
            if (skipCas) {
              assert.equal(result.verdict, "mismatch");
              assert.equal(result.difference.step, 6);
              const after = await deque.observe();
              assert.deepEqual(after.claimed[chosen], actors);
            } else {
              assert.equal(result.verdict, "matched");
              const after = await deque.observe();
              assert.deepEqual(after.queued, []);
              assert.deepEqual(after.claimed[chosen], [first]);
              assert.equal(
                after.claimed.Front.length + after.claimed.Back.length,
                2,
              );
            }
          },
        );
      }
    }

    await t.test(
      "both CAS operations are released with no ordering between their commands",
      async (t) => {
        const deque = setup(t, false);
        for (const command of last.commands) await deque.apply(command);
        const { candidate, queued } = await deque.observe();
        assert.equal(queued.length, 1);
        assert.equal(candidate.Owner, queued[0]);
        assert.equal(candidate.Thief, queued[0]);
        const outcomes = await Promise.all(
          actors.map((actor) => deque.apply({ kind: "claim", actor })),
        );
        assert.equal(outcomes.filter((task) => task === queued[0]).length, 1);
        assert.equal(outcomes.filter((task) => task === null).length, 1);
        assert.deepEqual((await deque.observe()).queued, []);
      },
    );
  },
);
