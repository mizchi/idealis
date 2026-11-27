import assert from "node:assert/strict";
import test, { type TestContext } from "node:test";
import { chromium, type Browser } from "@playwright/test";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

type Requester = "Writer" | "Later";
type Client = "ReaderA" | "ReaderB" | Requester;
type Availability = "Available" | "Unavailable";
type Command =
  | { kind: "acquire"; client: Requester }
  | { kind: "release"; client: Client }
  | { kind: "tryShared" };
type Harness = {
  start(client: Client): Promise<void>;
  release(client: Client): Promise<void>;
  tryShared(): Promise<void>;
  observe(): Promise<{
    held: Client[];
    waiting: { $sequence: Requester[] };
    started: Requester[];
    probe: Availability | null;
  }>;
};

declare global {
  var sharedLockHarness: Harness;
}

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/browser/web-locks-shared-fifo.model";

async function installLocks(heldOnlyProbe: boolean) {
  const name = "document";
  const jobs = new Map<
    Client,
    { entered: Promise<void>; done: Promise<void>; release: () => void }
  >();
  const held = new Set<Client>();
  const started = new Set<Requester>();
  let probe: Availability | null = null;

  async function start(client: Client) {
    let release!: () => void;
    let announce!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const entered = new Promise<void>((resolve) => {
      announce = resolve;
    });
    const mode = client === "Writer" ? "exclusive" : "shared";
    const done = navigator.locks.request(name, { mode }, async (lock) => {
      if (lock?.mode !== mode) throw new Error("unexpected lock mode");
      held.add(client);
      announce();
      await gate;
      held.delete(client);
    });
    void done.catch(() => undefined);
    jobs.set(client, { entered, done, release });
    if (client === "Writer" || client === "Later") started.add(client);
    await navigator.locks.query();
  }

  await start("ReaderA");
  await start("ReaderB");
  globalThis.sharedLockHarness = {
    start,
    async release(client) {
      const job = jobs.get(client);
      if (!job) throw new Error("no lock request to release");
      job.release();
      await job.done;
      jobs.delete(client);
    },
    async tryShared() {
      if (heldOnlyProbe) {
        const snapshot = await navigator.locks.query();
        probe = snapshot.held?.some(
          (lock) => lock.name === name && lock.mode === "exclusive",
        )
          ? "Unavailable"
          : "Available";
      } else {
        probe = await navigator.locks.request(
          name,
          { mode: "shared", ifAvailable: true },
          (lock): Availability => (lock ? "Available" : "Unavailable"),
        );
      }
    },
    async observe() {
      const snapshot = await navigator.locks.query();
      const waiting = (snapshot.pending ?? [])
        .filter((lock) => lock.name === name)
        .map((lock): Requester =>
          lock.mode === "exclusive" ? "Writer" : "Later",
        );
      for (const [client, job] of jobs) {
        if (!waiting.some((key) => key === client)) await job.entered;
      }
      const nativeHeld = (snapshot.held ?? []).filter(
        (lock) => lock.name === name,
      );
      if (nativeHeld.length !== held.size)
        throw new Error("held callbacks disagree with lock state");
      return {
        held: [...held].sort(),
        waiting: { $sequence: waiting },
        started: [...started].sort(),
        probe,
      };
    },
  };
}

async function openLocks(
  browser: Browser,
  t: TestContext,
  heldOnlyProbe = false,
) {
  const page = await browser.newPage();
  t.after(() => page.close());
  await page.route("https://shared-locks.test/", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: "<title>Shared Web Locks test</title>",
    }),
  );
  await page.goto("https://shared-locks.test/");
  await page.evaluate(installLocks, heldOnlyProbe);
  return {
    async apply(command: Command) {
      await page.evaluate(async (command) => {
        const api = globalThis.sharedLockHarness;
        switch (command.kind) {
          case "acquire":
            await api.start(command.client);
            break;
          case "release":
            await api.release(command.client);
            break;
          case "tryShared":
            await api.tryShared();
            break;
        }
      }, command);
    },
    observe: () => page.evaluate(() => globalThis.sharedLockHarness.observe()),
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test(
  "shared Web Locks stay behind a pending exclusive request",
  { timeout: 20_000 },
  async (t) => {
    const browser = await chromium.launch();
    t.after(() => browser.close());
    const blocked = await reachableScenario<Command>(
      checker,
      model,
      "WriterWaitingForBoth",
      { checker: "rprob" },
    );
    await t.test(
      "the queued writer is granted only after both readers finish",
      async (t) => {
        const driver = await openLocks(browser, t);
        const commands: Command[] = [
          ...blocked.commands,
          { kind: "tryShared" },
          { kind: "acquire", client: "Later" },
          { kind: "release", client: "ReaderA" },
          { kind: "release", client: "ReaderB" },
          { kind: "release", client: "Writer" },
          { kind: "release", client: "Later" },
        ];
        const result = await recordReplay(
          checker,
          { ...blocked, commands },
          driver,
        );
        assert.equal(result.verdict, "matched");
        const observations = result.recording.observations;
        const index = blocked.commands.length;
        assert.partialDeepStrictEqual(observations[index + 1], {
          probe: "Unavailable",
        });
        assert.partialDeepStrictEqual(observations[index + 3], {
          held: ["ReaderB"],
          waiting: { $sequence: ["Writer", "Later"] },
        });
        assert.partialDeepStrictEqual(observations[index + 4], {
          held: ["Writer"],
          waiting: { $sequence: ["Later"] },
        });
        assert.partialDeepStrictEqual(observations[index + 5], {
          held: ["Later"],
          waiting: { $sequence: [] },
        });
      },
    );
    await t.test(
      "readers still share the resource when there is no queued writer",
      async (t) => {
        const live = await reachableScenario<Command>(
          checker,
          model,
          "SharedRequestsOverlap",
          { checker: "rprob" },
        );
        const driver = await openLocks(browser, t);
        const result = await recordReplay(
          checker,
          {
            ...live,
            commands: [...live.commands, { kind: "tryShared" }],
          },
          driver,
        );
        assert.equal(result.verdict, "matched");
        assert.partialDeepStrictEqual(await driver.observe(), {
          held: ["Later", "ReaderA", "ReaderB"],
          probe: "Available",
        });
      },
    );
    await t.test(
      "a probe checking only held locks incorrectly admits a late reader",
      async (t) => {
        const driver = await openLocks(browser, t, true);
        const result = await recordReplay(
          checker,
          {
            ...blocked,
            commands: [...blocked.commands, { kind: "tryShared" }],
          },
          driver,
        );
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, blocked.commands.length + 1);
        assert.partialDeepStrictEqual(result.difference.expected, {
          probe: "Unavailable",
          held: ["ReaderA", "ReaderB"],
          waiting: { $sequence: ["Writer"] },
        });
        assert.partialDeepStrictEqual(result.difference.observed, {
          probe: "Available",
          held: ["ReaderA", "ReaderB"],
          waiting: { $sequence: ["Writer"] },
        });
        const itf = JSON.parse(JSON.stringify(toReplayItf(result.recording)));
        assert.deepEqual(
          await replayRecording(checker, fromReplayItf(itf), driver.project),
          {
            verdict: "mismatch",
            difference: result.difference,
          },
        );
      },
    );
  },
);
