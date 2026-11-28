import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import {
  readVersionedDelivery,
  settleVersionedDelivery,
  type VersionedSettlement,
} from "../../examples/model-patterns/subscriptions/delivery-stream.js";

type Attempt = "Primary" | "Hedge";
type Snapshot = { revision: number; outcome: "Delivered" | "Missing" };
type Command =
  | { kind: "connect"; attempt: Attempt }
  | {
      kind: "finish";
      attempt: Attempt;
      revision: number;
      outcome: Snapshot["outcome"];
    };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/delivery-revision.model";
const id = "shipment-1";
const frame = (kind: "notice" | "end") => JSON.stringify({ id, kind }) + "\n";

test(
  "a replicated delivery ignores stale evidence and compares answers for one revision",
  { timeout: 30_000 },
  async (t) => {
    const stale = await reachableScenario<Command>(
      checker,
      model,
      "StaleMissingWhileHedgeOpen",
    );
    const fresh = await reachableScenario<Command>(
      checker,
      model,
      "DeliveredAtFloorWhilePrimaryOpen",
    );
    const minimum = Number(fresh.initial.minimum);
    const cases = [
      {
        name: "stale first",
        prefix: stale,
        late: "Hedge",
        revision: minimum,
        outcome: "Delivered",
        conflict: false,
      },
      {
        name: "stale last",
        prefix: fresh,
        late: "Primary",
        revision: minimum - 1,
        outcome: "Missing",
        conflict: false,
      },
      {
        name: "same revision contradicts",
        prefix: fresh,
        late: "Primary",
        revision: minimum,
        outcome: "Missing",
        conflict: true,
      },
      {
        name: "same revision agrees",
        prefix: fresh,
        late: "Primary",
        revision: minimum,
        outcome: "Delivered",
        conflict: false,
      },
      {
        name: "other eligible revision",
        prefix: fresh,
        late: "Primary",
        revision: minimum + 1,
        outcome: "Missing",
        conflict: false,
      },
    ] as const;

    for (const { name, prefix, late, revision, outcome, conflict } of cases) {
      await t.test(name, async (t) => {
        const writers = new Map<string, ServerResponse>();
        const server = createServer((request, response) => {
          const query = new URL(request.url ?? "/", "http://localhost")
            .searchParams;
          const attempt = query.get("attempt");
          if (
            !attempt ||
            writers.has(attempt) ||
            query.get("id") !== id ||
            query.get("minimum") !== String(minimum)
          ) {
            response.writeHead(400).end();
            return;
          }
          writers.set(attempt, response);
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

        const responses = new Map<Attempt, Promise<Response>>();
        const readout: Record<Attempt, Snapshot | null> = {
          Primary: null,
          Hedge: null,
        };
        let accepted: VersionedSettlement<Attempt> = null;
        const commands: readonly Command[] = [
          ...prefix.commands,
          { kind: "finish", attempt: late, revision, outcome },
        ];

        const result = await recordReplay(
          checker,
          { ...prefix, commands },
          {
            async apply(command) {
              const { attempt } = command;
              if (command.kind === "connect") {
                const entered = once(server, "request");
                const request = fetch(
                  `http://127.0.0.1:${address.port}/?id=${id}&attempt=${attempt}&minimum=${minimum}`,
                  { signal: AbortSignal.timeout(10_000) },
                );
                responses.set(attempt, request);
                await Promise.race([entered, request]);
                assert.ok(writers.has(attempt));
                return;
              }
              const writer = writers.get(attempt);
              const response = responses.get(attempt);
              assert.ok(writer && response);
              writer.writeHead(200, {
                "Content-Type": "application/x-ndjson",
                "X-Delivery-Revision": String(command.revision),
              });
              writer.end(
                (command.outcome === "Delivered" ? frame("notice") : "") +
                  frame("end"),
              );
              const reading = await readVersionedDelivery(await response, id);
              assert.ok(
                reading.outcome !== "Pending",
                "response did not close",
              );
              readout[attempt] = {
                revision: reading.revision,
                outcome: reading.outcome,
              };
              accepted = settleVersionedDelivery(
                accepted,
                minimum,
                attempt,
                reading,
              );
              responses.delete(attempt);
            },
            async observe() {
              return {
                open: [...responses.keys()].sort(),
                readout: { ...readout },
                accepted:
                  accepted === null
                    ? null
                    : {
                        revision: accepted.revision,
                        outcome: accepted.evidence.outcome,
                      },
                winner: accepted?.evidence.winner ?? null,
                conflict: accepted?.evidence.conflict ?? false,
              };
            },
            project: ({ open, readout, accepted, winner, conflict }) => ({
              open: Array.isArray(open) ? [...open].sort() : open,
              readout,
              accepted,
              winner,
              conflict,
            }),
          },
        );

        if (result.verdict === "mismatch")
          assert.fail(JSON.stringify(result.difference));
        assert.deepEqual(result.recording.observations.at(-1), {
          open: [],
          readout: {
            Primary:
              name === "stale first"
                ? { revision: minimum - 1, outcome: "Missing" }
                : { revision, outcome },
            Hedge: { revision: minimum, outcome: "Delivered" },
          },
          accepted: { revision: minimum, outcome: "Delivered" },
          winner: "Hedge",
          conflict,
        });
      });
    }
  },
);
