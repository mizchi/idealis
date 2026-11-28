import assert from "node:assert/strict";
import test from "node:test";
import { replay, type Scenario } from "../../examples/model-patterns/replay.js";

const readers = ["Left", "Middle", "Right"] as const;
const epochs = ["Old", "New"] as const;
type Reader = (typeof readers)[number];
type Generation = (typeof epochs)[number];
type Command =
  | { readonly kind: "read"; readonly reader: Reader }
  | { readonly kind: "invalidate" }
  | { readonly kind: "resolve"; readonly completed: Generation };

const scenario: Scenario<Command> = {
  model: "examples/model-patterns/cache-refresh/cache.model",
  config: {},
  initial: {
    generation: "Old",
    cache: null,
    inflight: [],
    waiting: { Left: null, Middle: null, Right: null },
    visible: { Left: null, Middle: null, Right: null },
    starts: { Old: null, New: null },
  },
  commands: [
    { kind: "read", reader: "Left" },
    { kind: "read", reader: "Middle" },
    { kind: "invalidate" },
    { kind: "read", reader: "Right" },
    { kind: "resolve", completed: "New" },
    { kind: "resolve", completed: "Old" },
    { kind: "read", reader: "Left" },
  ],
};

test("cache readers share a promise and late completion cannot replace the new generation", async () => {
  for (const fault of ["none", "split", "late"] as const) {
    let generation: Generation = "Old";
    let cache: Generation | null = null;
    const waiting = new Map<Reader, Generation>();
    const visible = new Map<Reader, Generation>();
    const issued = { Old: 0, New: 0 };
    type Load = {
      promise: Promise<Generation>;
      resolve: (answer: Generation) => void;
    };
    const jobs = new Map<Generation, Load[]>();

    function load(epoch: Generation): Load {
      let active = jobs.get(epoch);
      if (!active) {
        active = [];
        jobs.set(epoch, active);
      }
      const existing = active[0];
      if (existing && fault !== "split") return existing;
      let resolve!: Load["resolve"];
      const promise = new Promise<Generation>((finish) => {
        resolve = finish;
      });
      const job = { promise, resolve };
      active.push(job);
      issued[epoch] += 1;
      void job.promise.then((answer) => {
        if (answer === generation || fault === "late") cache = answer;
      });
      return job;
    }

    function slots(values: ReadonlyMap<Reader, Generation>) {
      return Object.fromEntries(
        readers.map((reader) => [reader, values.get(reader) ?? null]),
      );
    }
    const run = replay("crates/rcheck/target/debug/rcheck", scenario, {
      async apply(command) {
        if (command.kind === "invalidate") {
          generation = "New";
          cache = null;
        } else if (command.kind === "read") {
          const reader = command.reader;
          if (cache !== null) {
            visible.set(reader, cache);
            return;
          }
          visible.delete(reader);
          waiting.set(reader, generation);
          void load(generation).promise.then((answer) => {
            waiting.delete(reader);
            if (answer === generation) visible.set(reader, answer);
          });
        } else {
          const active = jobs.get(command.completed);
          assert.ok(active);
          jobs.delete(command.completed);
          for (const job of active) job.resolve(command.completed);
          await Promise.all(active.map((job) => job.promise));
        }
      },
      async observe() {
        return {
          generation,
          cache,
          inflight: [...jobs.keys()].sort(),
          waiting: slots(waiting),
          visible: slots(visible),
          starts: Object.fromEntries(
            epochs.map((epoch) => [
              epoch,
              issued[epoch] === 0
                ? null
                : issued[epoch] === 1
                  ? "Once"
                  : "Repeated",
            ]),
          ),
        };
      },
      project(state) {
        return {
          ...state,
          inflight: [...(state.inflight as Generation[])].sort(),
        };
      },
    });
    if (fault === "split") {
      await assert.rejects(
        run,
        /"step":2.*"observed":.*"starts":\{"Old":"Repeated"/,
      );
    } else if (fault === "late") {
      await assert.rejects(
        run,
        /"step":6.*"observed":\{"generation":"New","cache":"Old"/,
      );
    } else {
      await run;
      assert.deepEqual(issued, { Old: 1, New: 1 });
      assert.equal(visible.get("Left"), "New");
    }
  }
});
