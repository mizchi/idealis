import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = {
  readonly kind:
    | "accept"
    | "beginDrain"
    | "reachDeadline"
    | "expire"
    | "finish"
    | "halt";
};
type Outcome = "Idle" | "Pending" | "Success" | "Aborted";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/http-drain/deadline.model";

test(
  "HTTP drain resolves finish and deadline in either order, releasing the resource once",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "DrainingWithPendingWork",
    );
    assert.deepEqual(
      base.commands.map(({ kind }) => kind),
      ["accept", "beginDrain"],
    );
    const cases = [
      {
        name: "finish first",
        order: ["finish", "expire"],
        outcome: "Success",
        faults: ["expire-overwrites", "expire-early"],
      },
      {
        name: "deadline first",
        order: ["expire", "finish"],
        outcome: "Aborted",
        faults: ["finish-overwrites"],
      },
    ] as const;

    for (const scenario of cases) {
      for (const fault of ["none", ...scenario.faults] as const) {
        await t.test(`${scenario.name} / ${fault}`, async (t) => {
          let response: ServerResponse | undefined;
          let releaseCount = 0;
          let outcome: Outcome = "Idle";
          let observed: Outcome = "Idle";
          let draining = false;
          let due = false;
          let client: Promise<void> | undefined;
          const server = createServer((_request, incoming) => {
            assert.equal(response, undefined);
            response = incoming;
            outcome = "Pending";
            observed = "Pending";
            server.emit("accepted");
          });
          server.listen(0, "127.0.0.1");
          await once(server, "listening");
          async function closeServer() {
            await new Promise<void>((resolve, reject) =>
              server.close((error) => (error ? reject(error) : resolve())),
            );
          }
          t.after(async () => {
            server.closeAllConnections();
            if (server.listening) await closeServer();
          });
          const address = server.address();
          assert.ok(address && typeof address !== "string");
          const url = `http://127.0.0.1:${address.port}`;

          async function settle(
            next: "Success" | "Aborted",
            overwrite: boolean,
          ) {
            if (outcome !== "Pending" && !overwrite) return;
            releaseCount += 1;
            outcome = next;
            if (!response) return;
            if (next === "Success") response.end("finished");
            else response.destroy();
            response = undefined;
            await client;
          }

          const commands: Command[] = [
            ...base.commands,
            { kind: "expire" },
            { kind: "reachDeadline" },
            ...scenario.order.map((kind) => ({ kind })),
            { kind: "halt" },
          ];
          const result = await recordReplay<Command>(
            checker,
            { ...base, commands },
            {
              async apply({ kind }) {
                switch (kind) {
                  case "accept": {
                    const ready = once(server, "accepted");
                    client = fetch(url).then(
                      async (reply) => {
                        assert.equal(reply.status, 200);
                        assert.equal(await reply.text(), "finished");
                        observed = "Success";
                      },
                      (error: unknown) => {
                        assert.ok(error instanceof TypeError);
                        observed = "Aborted";
                      },
                    );
                    void client.catch(() => {});
                    await Promise.race([ready, client]);
                    break;
                  }
                  case "beginDrain":
                    draining = true;
                    break;
                  case "reachDeadline":
                    due = true;
                    break;
                  case "expire":
                    if (due || fault === "expire-early")
                      await settle("Aborted", fault === "expire-overwrites");
                    break;
                  case "finish":
                    await settle("Success", fault === "finish-overwrites");
                    break;
                  case "halt":
                    if (!response) await closeServer();
                    break;
                }
              },
              async observe() {
                return {
                  phase: server.listening
                    ? draining
                      ? "Draining"
                      : "Accepting"
                    : "Halted",
                  outcome,
                  observed,
                  releaseCount,
                  inFlight: response !== undefined,
                };
              },
              project: ({ phase, outcome, releaseCount }) => ({
                phase,
                outcome,
                observed: outcome,
                releaseCount,
                inFlight: outcome === "Pending",
              }),
            },
          );

          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(result.recording.observations[3], {
              phase: "Draining",
              outcome: "Pending",
              observed: "Pending",
              releaseCount: 0,
              inFlight: true,
            });
            assert.deepEqual(result.recording.observations.at(-1), {
              phase: "Halted",
              outcome: scenario.outcome,
              observed: scenario.outcome,
              releaseCount: 1,
              inFlight: false,
            });
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.equal(
              result.difference.step,
              fault === "expire-early" ? 3 : 6,
            );
            assert.deepEqual(result.difference.observed, {
              phase: "Draining",
              outcome: fault === "finish-overwrites" ? "Success" : "Aborted",
              observed: fault === "expire-early" ? "Aborted" : scenario.outcome,
              releaseCount: fault === "expire-early" ? 1 : 2,
              inFlight: false,
            });
          }
        });
      }
    }
  },
);
