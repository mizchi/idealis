import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { transitionScenario } from "../../examples/model-patterns/replay.js";
import {
  repeatedTemporalScenario,
  recordReplay,
  type Scenario,
} from "../../packages/idealis/src/patterns.js";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/semaphore/weighted-fifo.model";
const clients = ["Owner", "Heavy", "Light"] as const;
const permits = ["Left", "Right"] as const;
type Client = (typeof clients)[number] | "LightA" | "LightB";
type Permit = (typeof permits)[number];
type Command = {
  readonly kind: "enqueue" | "grant" | "release";
  readonly client: Client;
};
const run = promisify(execFile);

function semaphore(
  policy: "fifo" | "anyFit" | "firstFit",
  observedClients: readonly Client[] = clients,
  initial: ReadonlyMap<Client, Permit[]> = new Map([["Owner", ["Left"]]]),
) {
  const queue: Client[] = [];
  const allocated = new Map(initial);
  return {
    async apply({ kind, client }: Command) {
      switch (kind) {
        case "enqueue":
          queue.push(client);
          break;
        case "release":
          assert.ok(allocated.delete(client));
          break;
        case "grant": {
          const index = queue.indexOf(client);
          assert.ok(index >= 0);
          if (policy === "fifo" && index !== 0) return;
          const inUse = new Set([...allocated.values()].flat());
          const available = permits.filter((permit) => !inUse.has(permit));
          const need = client === "Heavy" ? 2 : 1;
          if (
            policy === "firstFit" &&
            queue.findIndex(
              (queued) => available.length >= (queued === "Heavy" ? 2 : 1),
            ) !== index
          )
            return;
          assert.ok(available.length >= need);
          allocated.set(client, available.slice(0, need));
          queue.splice(index, 1);
          break;
        }
      }
    },
    async observe() {
      return {
        held: Object.fromEntries(
          observedClients.map((client) => [
            client,
            [...(allocated.get(client) ?? [])],
          ]),
        ),
        waiting: { $sequence: [...queue] },
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test("granting any fitting waiter can keep bypassing the head indefinitely", async (t) => {
  const dir = await mkdtemp(join(tmpdir(), "idealis-weighted-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const bypass = join(dir, "bypass.model");
  const source = await readFile(model, "utf8");
  const changed = source.replace(
    "client = at(waiting, 0),",
    "client in elements(waiting),",
  );
  assert.notEqual(changed, source);
  await writeFile(bypass, changed);

  for (const [path, question] of [
    [model, "HeadGetsCapacity"],
    [bypass, "HeadGetsCapacityUnderStrongGrant"],
  ] as const) {
    await run(checker, [
      path,
      "--checker",
      "explicit",
      "--question",
      question,
      "--expect",
      "holds",
    ]);
  }

  const target = "HeadGetsCapacity";
  const prefix = await repeatedTemporalScenario<Command>(
    checker,
    bypass,
    target,
    0,
  );
  const repeated = await repeatedTemporalScenario<Command>(
    checker,
    bypass,
    target,
    2,
  );
  const cycleLength = (repeated.commands.length - prefix.commands.length) / 2;
  assert.ok(Number.isInteger(cycleLength) && cycleLength > 0);
  const firstEnd = prefix.commands.length + cycleLength;
  assert.deepEqual(
    repeated.commands.slice(prefix.commands.length, firstEnd),
    repeated.commands.slice(firstEnd),
  );

  const overtaking = await recordReplay(checker, repeated, semaphore("anyFit"));
  assert.equal(overtaking.verdict, "matched");
  const observations = overtaking.recording.observations;
  const entry = observations[prefix.commands.length];
  assert.deepEqual(observations[firstEnd], entry);
  assert.deepEqual(observations.at(-1), entry);
  for (const observation of observations.slice(prefix.commands.length)) {
    assert.ok(observation !== null && typeof observation === "object");
    const state = observation as {
      held: { Heavy: Permit[] };
      waiting: { $sequence: Client[] };
    };
    assert.deepEqual(state.held.Heavy, []);
    assert.equal(state.waiting.$sequence[0], "Heavy");
  }

  const fifo = await recordReplay(checker, repeated, semaphore("fifo"));
  assert.equal(fifo.verdict, "mismatch");
  assert.deepEqual(fifo.difference.command, { kind: "grant", client: "Light" });
  const successful = await transitionScenario<Command>(
    checker,
    model,
    "grant",
    "client = Heavy",
  );
  const complete = await recordReplay(checker, successful, semaphore("fifo"));
  assert.equal(complete.verdict, "matched");
  assert.deepEqual(complete.recording.observations.at(-1), {
    held: { Owner: [], Heavy: ["Left", "Right"], Light: [] },
    waiting: { $sequence: [] },
  });
});

test(
  "first-fit can keep a heavy waiter disabled even under strong grant fairness",
  { timeout: 60_000 },
  async () => {
    const firstFit =
      "examples/model-patterns/semaphore/weighted-first-fit.model";
    const { stdout } = await run(checker, [
      "witness",
      firstFit,
      "HeavyEventuallyAcquires",
      "--max-states",
      "20000",
    ]);
    const report = JSON.parse(stdout) as {
      complete: boolean;
      verdict: string;
      trace?: Omit<Scenario<Command | null>, "model"> & { loopStart: number };
    };
    assert.equal(report.verdict, "violated");
    assert.equal(report.complete, true);
    const trace = report.trace;
    assert.ok(trace);
    const prefix = trace.commands
      .slice(0, trace.loopStart)
      .filter((command) => command !== null);
    const loop = trace.commands
      .slice(trace.loopStart)
      .filter((command) => command !== null);
    for (const client of ["LightA", "LightB"]) {
      assert.ok(
        loop.some(
          (command) => command.kind === "grant" && command.client === client,
        ),
      );
      assert.ok(
        loop.some(
          (command) => command.kind === "release" && command.client === client,
        ),
      );
    }
    const replay = await recordReplay(
      checker,
      {
        model: firstFit,
        config: trace.config,
        initial: trace.initial,
        commands: [...prefix, ...loop, ...loop],
      },
      semaphore(
        "firstFit",
        ["Heavy", "LightA", "LightB"],
        new Map([
          ["LightA", ["Left"]],
          ["LightB", ["Right"]],
        ]),
      ),
    );
    assert.equal(replay.verdict, "matched");
    const { observations } = replay.recording;
    const entry = observations[prefix.length];
    assert.deepEqual(observations[prefix.length + loop.length], entry);
    assert.deepEqual(observations.at(-1), entry);
    for (const sample of observations.slice(prefix.length)) {
      const snapshot = sample as {
        held: Record<string, Permit[]>;
        waiting: { $sequence: Client[] };
      };
      assert.ok(snapshot.waiting.$sequence.includes("Heavy"));
      assert.deepEqual(snapshot.held.Heavy, []);
      assert.ok(Object.values(snapshot.held).flat().length >= 1);
    }
  },
);
