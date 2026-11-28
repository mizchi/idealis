import assert from "node:assert/strict";
import test, { type TestContext } from "node:test";
import { chromium, type Browser, type Page } from "@playwright/test";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

type Client = "Owner" | "First" | "Second";
type Waiter = Exclude<Client, "Owner">;
type Fault = "ignore-pending-signal" | "release-on-granted-abort";
type Command =
  | { kind: "enqueue" | "cancel"; client: Waiter }
  | { kind: "release"; client: Client };
type LockHarness = {
  start(): void;
  entered(): Promise<void>;
  cancel(): Promise<void>;
  release(): Promise<void>;
  state(): { aborted: boolean; completed: boolean; rejected: boolean };
};

declare global {
  var modelLockHarness: LockHarness;
}

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/browser/web-locks-cancellation.ideal";
const clients = ["Owner", "First", "Second"] as const;
const resource = "document";

function installHarness({
  name,
  fault,
}: {
  name: string;
  fault: Fault | undefined;
}) {
  const controller = new AbortController();
  const reason = new DOMException("request withdrawn", "AbortError");
  let release!: () => void;
  let entered!: () => void;
  let granted = false;
  let completed = false;
  let rejected = false;
  let request: Promise<void> | undefined;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const announced = new Promise<void>((resolve) => {
    entered = resolve;
  });
  globalThis.modelLockHarness = {
    start() {
      const options =
        fault === "ignore-pending-signal" ? {} : { signal: controller.signal };
      request = navigator.locks
        .request(name, options, async (lock) => {
          if (!lock || lock.mode !== "exclusive")
            throw new Error("expected an exclusive lock");
          granted = true;
          if (fault === "release-on-granted-abort") {
            controller.signal.addEventListener("abort", release, {
              once: true,
            });
          }
          entered();
          await gate;
        })
        .then(
          () => {
            completed = true;
          },
          (error: unknown) => {
            if (error !== reason) throw error;
            rejected = true;
          },
        );
      void request.catch(() => undefined);
    },
    entered: () => announced,
    async cancel() {
      controller.abort(reason);
      if (
        granted
          ? fault === "release-on-granted-abort"
          : fault !== "ignore-pending-signal"
      ) {
        await request;
      }
      await navigator.locks.query();
    },
    async release() {
      release();
      await request;
    },
    state: () => ({ aborted: controller.signal.aborted, completed, rejected }),
  };
}

async function openLocks(browser: Browser, t: TestContext, fault?: Fault) {
  const context = await browser.newContext();
  t.after(() => context.close());
  await context.route("https://locks.test/", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: "<title>Web Locks test</title>",
    }),
  );
  const pages = new Map<Client, Page>();
  const identities = new Map<string, Client>();
  for (const client of clients) {
    const page = await context.newPage();
    await page.goto("https://locks.test/");
    const identity = await page.evaluate(
      (tag) =>
        navigator.locks.request(tag, async () => {
          const snapshot = await navigator.locks.query();
          return snapshot.held?.find((lock) => lock.name === tag)?.clientId;
        }),
      `probe-${client}`,
    );
    assert.ok(identity);
    assert.ok(!identities.has(identity));
    identities.set(identity, client);
    pages.set(client, page);
    await page.evaluate(installHarness, {
      name: resource,
      fault: client === "Owner" ? undefined : fault,
    });
  }
  const pane = (client: Client) => {
    const page = pages.get(client);
    assert.ok(page);
    return page;
  };
  async function readLocks() {
    const snapshot = await pane("Owner").evaluate(() =>
      navigator.locks.query(),
    );
    const translate = (entries: LockInfo[] | undefined) =>
      (entries ?? [])
        .filter((lock) => lock.name === resource)
        .map((lock) => {
          const client = identities.get(lock.clientId ?? "");
          assert.ok(client, "the lock belongs to one of the observed clients");
          return client;
        });
    const held = translate(snapshot.held);
    assert.ok(held.length <= 1);
    return {
      holder: held[0] ?? null,
      waiting: { $sequence: translate(snapshot.pending) },
    };
  }

  await pane("Owner").evaluate(async () => {
    globalThis.modelLockHarness.start();
    await globalThis.modelLockHarness.entered();
  });
  return {
    async apply(command: Command) {
      const page = pane(command.client);
      switch (command.kind) {
        case "enqueue":
          await page.evaluate(async () => {
            globalThis.modelLockHarness.start();
            await navigator.locks.query();
          });
          break;
        case "cancel":
          await page.evaluate(() => globalThis.modelLockHarness.cancel());
          break;
        case "release": {
          await page.evaluate(() => globalThis.modelLockHarness.release());
          const { holder } = await readLocks();
          if (holder)
            await pane(holder).evaluate(() =>
              globalThis.modelLockHarness.entered(),
            );
          break;
        }
      }
    },
    async observe() {
      const states = await Promise.all(
        clients.map(async (client) => ({
          client,
          ...(await pane(client).evaluate(() =>
            globalThis.modelLockHarness.state(),
          )),
        })),
      );
      return {
        ...(await readLocks()),
        aborted: states
          .filter((entry) => entry.aborted)
          .map((entry) => entry.client),
        rejected: states
          .filter((entry) => entry.rejected)
          .map((entry) => entry.client),
        completed: states
          .filter((entry) => entry.completed)
          .map((entry) => entry.client),
      };
    },
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test(
  "Web Locks discards pending cancellations and keeps a granted callback in control",
  { timeout: 30_000 },
  async (t) => {
    const browser = await chromium.launch();
    t.after(() => browser.close());
    const base = await reachableScenario<Command>(
      checker,
      model,
      "TwoPending",
      { checker: "rprob" },
    );
    const commands: Command[] = [
      ...base.commands,
      { kind: "cancel", client: "First" },
      { kind: "release", client: "Owner" },
      { kind: "cancel", client: "Second" },
      { kind: "release", client: "Second" },
    ];
    const scenario = { ...base, commands };
    await t.test(
      "the next page acquires and calling abort after grant leaves it held",
      async (t) => {
        const driver = await openLocks(browser, t);
        const result = await recordReplay(checker, scenario, driver);
        assert.equal(result.verdict, "matched");
        assert.deepEqual(result.recording.observations.at(-1), {
          holder: null,
          waiting: { $sequence: [] },
          aborted: ["First", "Second"],
          rejected: ["First"],
          completed: ["Owner", "Second"],
        });
      },
    );
    await t.test(
      "without withdrawal the first request is granted before the second",
      async (t) => {
        const driver = await openLocks(browser, t);
        const result = await recordReplay(
          checker,
          {
            ...base,
            commands: [...base.commands, { kind: "release", client: "Owner" }],
          },
          driver,
        );
        assert.equal(result.verdict, "matched");
        assert.partialDeepStrictEqual(await driver.observe(), {
          holder: "First",
          waiting: { $sequence: ["Second"] },
          completed: ["Owner"],
        });
      },
    );
    for (const control of [
      {
        fault: "ignore-pending-signal",
        step: 3,
        expected: {
          holder: "Owner",
          rejected: ["First"],
          waiting: { $sequence: ["Second"] },
        },
        observed: {
          holder: "Owner",
          rejected: [],
          waiting: { $sequence: ["First", "Second"] },
        },
      },
      {
        fault: "release-on-granted-abort",
        step: 5,
        expected: { holder: "Second", completed: ["Owner"] },
        observed: { holder: null, completed: ["Owner", "Second"] },
      },
    ] as const) {
      await t.test(control.fault, async (t) => {
        const driver = await openLocks(browser, t, control.fault);
        const result = await recordReplay(checker, scenario, driver);
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, control.step);
        assert.partialDeepStrictEqual(
          result.difference.expected,
          control.expected,
        );
        assert.partialDeepStrictEqual(
          result.difference.observed,
          control.observed,
        );
        const itf = JSON.parse(JSON.stringify(toReplayItf(result.recording)));
        assert.deepEqual(
          await replayRecording(checker, fromReplayItf(itf), driver.project),
          {
            verdict: "mismatch",
            difference: result.difference,
          },
        );
      });
    }
  },
);
