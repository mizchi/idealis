import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import {
  createServer,
  type RequestListener,
  type ServerResponse,
} from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
  repeatedTemporalScenario,
} from "../../examples/model-patterns/replay.js";

type Command = {
  kind:
    | "charge"
    | "reserve"
    | "rejectInventory"
    | "refund"
    | "loseReply"
    | "confirmRefund";
};
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/compensation/saga.ideal";

async function listen(
  t: TestContext,
  handler: RequestListener,
): Promise<string> {
  const server = createServer(handler);
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(async () => {
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  return `http://127.0.0.1:${address.port}`;
}

test(
  "refund replay preserves one refund after a lost HTTP response",
  { timeout: 30_000 },
  async (t) => {
    const lost = await reachableScenario<Command>(
      checker,
      model,
      "RefundCommittedAndReplyLost",
    );
    const reserved = await reachableScenario<Command>(
      checker,
      model,
      "OrderReserved",
    );
    assert.deepEqual(
      lost.commands.map(({ kind }) => kind),
      ["charge", "rejectInventory", "refund", "loseReply"],
    );

    const dir = await mkdtemp(join(tmpdir(), "idealis-refund-"));
    t.after(() => rm(dir, { recursive: true, force: true }));
    const weak = join(dir, "weak.model");
    const source = await readFile(model, "utf8");
    await writeFile(
      weak,
      source.replace("strong confirmRefund", "weak confirmRefund"),
    );
    const repeated = await repeatedTemporalScenario<Command>(
      checker,
      weak,
      "RefundEventuallyConfirmed",
      3,
    );
    const refundRequests = repeated.commands.filter(
      ({ kind }) => kind === "refund",
    ).length;
    assert.ok(refundRequests >= 3);
    assert.ok(
      repeated.commands.filter(({ kind }) => kind === "loseReply").length >= 3,
    );

    for (const [name, base] of [
      ["reserved", reserved],
      ["retried", lost],
      ["duplicate-refund", lost],
      ["confirm-on-loss", lost],
      ["repeated-loss", repeated],
    ] as const) {
      await t.test(name, async (t) => {
        const received = new EventEmitter();
        const ledger = new DatabaseSync(":memory:");
        t.after(() => ledger.close());
        ledger.exec(`CREATE TABLE charges (id TEXT PRIMARY KEY);
        CREATE TABLE refunds (id TEXT NOT NULL);
        CREATE TABLE receipts (id TEXT PRIMARY KEY);
        CREATE TABLE attempts (id TEXT NOT NULL)`);
        const payment = await listen(t, (request, response) => {
          const key = request.headers["idempotency-key"];
          if (request.method !== "POST" || typeof key !== "string") {
            response.writeHead(400).end();
            return;
          }
          if (request.url === "/charge") {
            ledger.prepare("INSERT INTO charges VALUES (?)").run(key);
            response.writeHead(204).end();
            return;
          }
          assert.equal(request.url, "/refund");
          ledger.exec("BEGIN");
          try {
            ledger.prepare("INSERT INTO attempts VALUES (?)").run(key);
            const receipt = ledger
              .prepare("INSERT OR IGNORE INTO receipts VALUES (?)")
              .run(key);
            if (Number(receipt.changes) > 0 || name === "duplicate-refund") {
              ledger.prepare("INSERT INTO refunds VALUES (?)").run(key);
            }
            ledger.exec("COMMIT");
          } catch (error) {
            ledger.exec("ROLLBACK");
            response.writeHead(500).end();
            received.emit("error", error);
            return;
          }
          received.emit("refund", response);
        });
        const inventory = await listen(t, (request, response) => {
          assert.equal(request.method, "POST");
          assert.equal(request.url, "/reserve");
          response.writeHead(name === "reserved" ? 204 : 409).end();
        });
        let stage = "New";
        let pending: {
          serve: ServerResponse;
          consume: Promise<{ status: number } | { error: unknown }>;
        } | null = null;
        function post(origin: string, path: string, key: string) {
          return fetch(origin + path, {
            method: "POST",
            headers: { "idempotency-key": key },
            signal: AbortSignal.timeout(5_000),
          });
        }
        const result = await recordReplay(
          checker,
          name === "reserved" || name === "repeated-loss"
            ? base
            : {
                ...base,
                commands: [
                  ...base.commands,
                  { kind: "refund" },
                  { kind: "confirmRefund" },
                ],
              },
          {
            async apply({ kind }) {
              switch (kind) {
                case "charge": {
                  const response = await post(
                    payment,
                    "/charge",
                    "charge-order",
                  );
                  assert.equal(response.status, 204);
                  stage = "Charged";
                  break;
                }
                case "reserve":
                case "rejectInventory": {
                  const response = await post(inventory, "/reserve", "order");
                  assert.ok(response.status === 204 || response.status === 409);
                  stage = response.status === 204 ? "Reserved" : "Refunding";
                  break;
                }
                case "refund": {
                  const written = once(received, "refund", {
                    signal: AbortSignal.timeout(5_000),
                  });
                  const consume = post(payment, "/refund", "refund-order").then(
                    (response) => ({ status: response.status }),
                    (error: unknown) => ({ error }),
                  );
                  const [serve] = (await written) as [ServerResponse];
                  pending = { serve, consume };
                  break;
                }
                case "loseReply": {
                  assert.ok(pending);
                  pending.serve.destroy();
                  const received = await pending.consume;
                  assert.ok("error" in received);
                  assert.ok(received.error instanceof TypeError);
                  assert.equal(
                    (received.error.cause as { code?: string })?.code,
                    "UND_ERR_SOCKET",
                  );
                  pending = null;
                  if (name === "confirm-on-loss") stage = "Cancelled";
                  break;
                }
                case "confirmRefund": {
                  assert.ok(pending);
                  pending.serve.writeHead(204).end();
                  assert.deepEqual(await pending.consume, { status: 204 });
                  pending = null;
                  stage = "Cancelled";
                  break;
                }
              }
            },
            async observe() {
              const n = Number(
                ledger.prepare("SELECT COUNT(*) AS n FROM refunds").get()?.n,
              );
              return {
                stage,
                replyPending: pending !== null,
                effect:
                  n === 0 ? "NoRefund" : n === 1 ? "Refunded" : "Repeated",
              };
            },
            project: ({ stage, replyPending, effect }) => ({
              stage,
              replyPending,
              effect,
            }),
          },
        );

        assert.equal(
          ledger.prepare("SELECT COUNT(*) AS n FROM charges").get()?.n,
          1,
        );
        assert.equal(
          ledger.prepare("SELECT COUNT(*) AS n FROM attempts").get()?.n,
          name === "repeated-loss"
            ? refundRequests
            : name === "reserved"
              ? 0
              : name === "confirm-on-loss"
                ? 1
                : 2,
        );
        if (name === "duplicate-refund") {
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(result.difference, {
            step: 5,
            command: { kind: "refund" },
            expected: {
              stage: "Refunding",
              replyPending: true,
              effect: "Refunded",
            },
            observed: {
              stage: "Refunding",
              replyPending: true,
              effect: "Repeated",
            },
          });
        } else if (name === "confirm-on-loss") {
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(result.difference, {
            step: 4,
            command: { kind: "loseReply" },
            expected: {
              stage: "Refunding",
              replyPending: false,
              effect: "Refunded",
            },
            observed: {
              stage: "Cancelled",
              replyPending: false,
              effect: "Refunded",
            },
          });
        } else if (name === "repeated-loss") {
          assert.equal(result.verdict, "matched");
          const last = result.recording.observations.at(-1);
          assert.ok(last && typeof last === "object");
          assert.equal(Reflect.get(last, "stage"), "Refunding");
          assert.equal(Reflect.get(last, "effect"), "Refunded");
          assert.equal(
            ledger.prepare("SELECT COUNT(*) AS n FROM refunds").get()?.n,
            1,
          );
        } else {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), {
            stage: name === "reserved" ? "Reserved" : "Cancelled",
            replyPending: false,
            effect: name === "reserved" ? "NoRefund" : "Refunded",
          });
        }
      });
    }
  },
);
