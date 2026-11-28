import assert from "node:assert/strict";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import { MessageChannel } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Store = "east" | "west";
type Request = "First" | "Second";
type Command = {
  kind: `${Store}_commit` | `${Store}Delivery_send`;
  request: Request;
};

const checker = "crates/idealis-check/target/debug/idealis-check";
const model =
  "examples/model-patterns/offline-merge/two-stores/model.project.json";

test("two storage instances keep same-ID receipts separate across delivery", async (t) => {
  const scenario = await reachableScenario<Command>(
    checker,
    model,
    "IndependentReceipts",
  );
  assert.equal(scenario.commands.length, 5);
  for (const crossed of [false, true]) {
    await t.test(
      crossed ? "delivery reads the wrong store" : "independent stores",
      async (t) => {
        const root = await mkdtemp(join(tmpdir(), "idealis-two-stores-"));
        t.after(() => rm(root, { recursive: true, force: true }));
        using database = new DatabaseSync(join(root, "stores.sqlite"));
        database.exec(`
        CREATE TABLE receipt (
          store TEXT NOT NULL,
          request TEXT NOT NULL,
          revision INTEGER NOT NULL,
          PRIMARY KEY (store, request),
          UNIQUE (store, revision)
        );
      `);
        const append = database.prepare(`
        INSERT INTO receipt
        SELECT ?, ?, coalesce(max(revision), 0) + 1 FROM receipt WHERE store = ?
      `);
        const read = database.prepare(
          "SELECT revision FROM receipt WHERE store = ? AND request = ?",
        );
        const head = database.prepare(
          "SELECT coalesce(max(revision), 0) AS revision FROM receipt WHERE store = ?",
        );
        const receipt = (store: Store, request: Request) =>
          Number(read.get(store, request)?.revision) || null;
        const delivered: Record<Store, Record<Request, number | null>> = {
          east: { First: null, Second: null },
          west: { First: null, Second: null },
        };
        const run = await recordReplay(checker, scenario, {
          async apply({ kind, request }) {
            const store = kind.startsWith("east") ? "east" : "west";
            if (kind.endsWith("_commit")) {
              append.run(store, request, store);
            } else {
              delivered[store][request] = receipt(
                crossed && store === "west" ? "east" : store,
                request,
              );
            }
          },
          async observe() {
            return {
              east_head: head.get("east")?.revision,
              west_head: head.get("west")?.revision,
              eastReceipt: {
                First: receipt("east", "First"),
                Second: receipt("east", "Second"),
              },
              westReceipt: {
                First: receipt("west", "First"),
                Second: receipt("west", "Second"),
              },
              eastDelivery_received: { ...delivered.east },
              westDelivery_received: { ...delivered.west },
            };
          },
          project: (state) => state,
        });
        if (!crossed) {
          assert.equal(run.verdict, "matched");
          return;
        }
        assert.equal(run.verdict, "mismatch");
        assert.equal(run.difference.step, 5);
        assert.deepEqual(run.difference.command, {
          kind: "westDelivery_send",
          request: "First",
        });
        const expected = run.difference.expected as {
          westDelivery_received: Record<Request, number | null>;
        };
        const observed = run.difference.observed as typeof expected;
        assert.equal(expected.westDelivery_received.First, 1);
        assert.equal(observed.westDelivery_received.First, 2);
      },
    );
  }
});

test("the standalone delivery harness replays a receipt across a message port", async (t) => {
  type Input = { request: Request; revision: number };
  type Step =
    | ({ kind: "publish" } & Input)
    | { kind: "subscriber_send"; request: Request };
  const harness =
    "examples/model-patterns/offline-merge/two-stores/delivery-harness.project.json";
  const scenario = await reachableScenario<Step>(
    checker,
    harness,
    "subscriber_CanDeliver",
  );
  const publication = scenario.commands[0];
  assert.equal(publication?.kind, "publish");
  for (const stale of [false, true]) {
    await t.test(
      stale ? "stale version delivered" : "published version delivered",
      async (t) => {
        const { port1, port2 } = new MessageChannel();
        t.after(() => {
          port1.close();
          port2.close();
        });
        const input: Record<Request, number | null> = {
          First: null,
          Second: null,
        };
        const received: Record<Request, number | null> = {
          First: null,
          Second: null,
        };
        const result = await recordReplay(checker, scenario, {
          async apply(step) {
            if (step.kind === "publish") {
              const arrival = once(port2, "message");
              port1.postMessage({
                request: step.request,
                revision: step.revision,
              });
              const [message] = (await arrival) as [Input];
              input[message.request] = message.revision;
            } else {
              received[step.request] = stale ? 0 : input[step.request];
            }
          },
          async observe() {
            return {
              input: { ...input },
              subscriber_received: { ...received },
            };
          },
          project: (state) => state,
        });
        if (!stale) {
          assert.equal(result.verdict, "matched");
          return;
        }
        assert.equal(result.verdict, "mismatch");
        assert.equal(result.difference.step, 2);
        assert.equal(result.difference.command?.kind, "subscriber_send");
      },
    );
  }
});
