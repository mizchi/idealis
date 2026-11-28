import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import {
  type DeliveryResult,
  type DeliverySettlement,
  readDelivery,
  settleDelivery,
} from "../../examples/model-patterns/subscriptions/delivery-stream.js";

type Attempt = "Primary" | "Hedge";
type Command =
  | { readonly kind: "connect"; readonly attempt: Attempt }
  | {
      readonly kind: "finish";
      readonly attempt: Attempt;
      readonly evidence: DeliveryResult;
    };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/subscriptions/delivery-race.ideal";
const id = "order-42";
const frame = (kind: "notice" | "end") => JSON.stringify({ kind, id }) + "\n";

test(
  "overlapping HTTP responses preserve the first complete answer and surface conflicting evidence",
  { timeout: 30_000 },
  async (t) => {
    const scenarios = [
      {
        question: "PrimaryWinsOpenRace",
        winner: "Primary",
        late: "Hedge",
        first: "Delivered",
      },
      {
        question: "HedgeWinsOpenRace",
        winner: "Hedge",
        late: "Primary",
        first: "Delivered",
      },
      {
        question: "MissingWinsOpenRace",
        winner: "Hedge",
        late: "Primary",
        first: "Missing",
      },
    ] as const;
    for (const { question, winner, late, first } of scenarios) {
      const prefix = await reachableScenario<Command>(checker, model, question);
      assert.deepEqual(
        prefix.commands.map(({ kind }) => kind),
        ["connect", "connect", "finish"],
      );
      for (const evidence of ["Pending", "Missing", "Delivered"] as const) {
        await t.test(
          `${winner} first with ${first}, then ${evidence}`,
          async (t) => {
            const writers = new Map<string, ServerResponse>();
            const server = createServer((request, response) => {
              const params = new URL(request.url ?? "/", "http://localhost")
                .searchParams;
              const attempt = params.get("attempt");
              if (!attempt || params.get("id") !== id || writers.has(attempt)) {
                response.writeHead(400).end();
                return;
              }
              writers.set(attempt, response);
              response.writeHead(200, {
                "Content-Type": "application/x-ndjson",
              });
              response.flushHeaders();
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

            const responses = new Map<Attempt, Response>();
            const readout: Record<Attempt, DeliveryResult | null> = {
              Primary: null,
              Hedge: null,
            };
            let settlement: DeliverySettlement<Attempt> = {
              outcome: "Pending",
            };
            const commands: readonly Command[] = [
              ...prefix.commands,
              { kind: "finish", attempt: late, evidence },
            ];
            const result = await recordReplay(
              checker,
              { ...prefix, commands },
              {
                async apply(command) {
                  const { attempt } = command;
                  if (command.kind === "connect") {
                    const response = await fetch(
                      `http://127.0.0.1:${address.port}/?attempt=${attempt}&id=${id}`,
                    );
                    assert.equal(response.status, 200);
                    responses.set(attempt, response);
                    return;
                  }
                  const writer = writers.get(attempt);
                  const response = responses.get(attempt);
                  assert.ok(writer && response);
                  const pending = readDelivery(response, id);
                  switch (command.evidence) {
                    case "Pending":
                      writer.destroy();
                      break;
                    case "Missing":
                      writer.end(frame("end"));
                      break;
                    case "Delivered":
                      writer.end(frame("notice") + frame("end"));
                      break;
                  }
                  const observed = await pending;
                  responses.delete(attempt);
                  readout[attempt] = observed;
                  settlement = settleDelivery(settlement, attempt, observed);
                },
                async observe() {
                  return {
                    open: [...responses.keys()].sort(),
                    readout: { ...readout },
                    outcome: settlement.outcome,
                    winner:
                      settlement.outcome === "Pending"
                        ? null
                        : settlement.winner,
                    conflict:
                      settlement.outcome === "Pending"
                        ? false
                        : settlement.conflict,
                  };
                },
                project: ({ open, readout, outcome, winner, conflict }) => ({
                  open: Array.isArray(open) ? [...open].sort() : open,
                  readout,
                  outcome,
                  winner,
                  conflict,
                }),
              },
            );
            if (result.verdict === "mismatch")
              assert.fail(JSON.stringify(result.difference));
            assert.deepEqual(result.recording.observations.at(-1), {
              open: [],
              readout: { [winner]: first, [late]: evidence },
              outcome: first,
              winner,
              conflict: evidence !== "Pending" && evidence !== first,
            });
          },
        );
      }
    }
  },
);
