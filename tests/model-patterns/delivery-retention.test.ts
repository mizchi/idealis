import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import {
  readPinnedDelivery,
  type PinnedDeliveryResult,
} from "../../examples/model-patterns/subscriptions/delivery-stream.js";

type Command = { kind: "outage" | "restore" | "compact" | "probe" | "restart" };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/delivery-retention.model";
const id = "order-42";
const frame = (kind: "notice" | "end") => JSON.stringify({ kind, id }) + "\n";

test(
  "retry preserves the pinned version until the server reports it has expired",
  { timeout: 30_000 },
  async (t) => {
    const temporary = await reachableScenario<Command>(
      checker,
      model,
      "TemporaryFailure",
    );
    const expired: Command[] = [{ kind: "compact" }, { kind: "probe" }];
    const cases = [
      {
        name: "replica recovers",
        commands: [
          ...temporary.commands,
          { kind: "restore" },
          { kind: "probe" },
        ],
        revisions: [1, 1],
        final: { pinned: 1, result: "Missing" },
        fault: null,
      },
      {
        name: "restart after expiry",
        commands: [
          ...temporary.commands,
          ...expired,
          { kind: "restart" },
          { kind: "restore" },
          { kind: "probe" },
        ],
        revisions: [1, 1, 2],
        final: { pinned: 2, result: "Delivered" },
        fault: null,
      },
      {
        name: "expiry reported as temporary",
        commands: [...temporary.commands, ...expired],
        revisions: [1, 1],
        fault: "mask-expiry",
      },
      {
        name: "expiry silently served from head",
        commands: [...temporary.commands, ...expired],
        revisions: [1, 1],
        fault: "serve-head",
      },
    ] as const;

    for (const scenario of cases) {
      const { name, commands, revisions, fault } = scenario;
      await t.test(name, async (t) => {
        let floor = 1;
        let online = true;
        const snapshots = new Map<number, string>([
          [1, frame("end")],
          [2, frame("notice") + frame("end")],
        ]);
        const requested: number[] = [];
        const server = createServer((request, response) => {
          const url = new URL(request.url ?? "/", "http://localhost");
          if (url.pathname === "/head") {
            response.writeHead(200, { "Content-Type": "application/json" });
            response.end(JSON.stringify({ revision: 2 }));
            return;
          }

          assert.equal(url.searchParams.get("id"), id);
          const revision = Number(url.searchParams.get("revision"));
          requested.push(revision);
          if (revision < floor && fault !== "serve-head") {
            response.writeHead(fault === "mask-expiry" ? 503 : 410).end();
            return;
          }
          if (!online && revision >= floor) {
            response.writeHead(503).end();
            return;
          }
          const served = Math.max(floor, revision);
          const body = snapshots.get(served);
          assert.ok(body);
          response.writeHead(200, {
            "Content-Type": "application/x-ndjson",
            "X-Delivery-Revision": String(served),
          });
          response.end(body);
        });
        server.listen(0, "127.0.0.1");
        await once(server, "listening");
        t.after(async () => {
          const closed = new Promise<void>((resolve, reject) =>
            server.close((error) => (error ? reject(error) : resolve())),
          );
          server.closeAllConnections();
          await closed;
        });
        const address = server.address();
        assert.ok(address && typeof address !== "string");
        const origin = `http://127.0.0.1:${address.port}`;
        let pinned = 1;
        let result: "Waiting" | PinnedDeliveryResult = "Waiting";
        const verdict = await recordReplay(
          checker,
          { ...temporary, commands },
          {
            async apply({ kind }) {
              switch (kind) {
                case "outage":
                  online = false;
                  break;
                case "restore":
                  online = true;
                  break;
                case "compact":
                  snapshots.delete(floor);
                  floor = 2;
                  break;
                case "probe":
                  result = await readPinnedDelivery(
                    await fetch(
                      `${origin}/delivery?id=${id}&revision=${pinned}`,
                    ),
                    id,
                    pinned,
                  );
                  break;
                case "restart": {
                  const response = await fetch(`${origin}/head`);
                  assert.equal(response.status, 200);
                  const wire: unknown = await response.json();
                  assert.ok(
                    wire && typeof wire === "object" && "revision" in wire,
                  );
                  assert.ok(
                    typeof wire.revision === "number" &&
                      Number.isSafeInteger(wire.revision),
                  );
                  pinned = wire.revision;
                  result = "Waiting";
                  break;
                }
              }
            },
            async observe() {
              return { pinned, result };
            },
            project: ({ pinned, result }) => ({ pinned, result }),
          },
        );

        assert.deepEqual(requested, revisions);
        if (scenario.fault !== null) {
          assert.equal(verdict.verdict, "mismatch");
          assert.equal(verdict.difference.step, commands.length);
          assert.deepEqual(verdict.difference.command, { kind: "probe" });
          assert.deepEqual(verdict.difference.expected, {
            pinned: 1,
            result: "Expired",
          });
          assert.deepEqual(verdict.difference.observed, {
            pinned: 1,
            result: fault === "serve-head" ? "WrongRevision" : "Retry",
          });
        } else {
          if (verdict.verdict === "mismatch")
            assert.fail(JSON.stringify(verdict.difference));
          assert.deepEqual(
            verdict.recording.observations.at(-1),
            scenario.final,
          );
        }
      });
    }
  },
);
