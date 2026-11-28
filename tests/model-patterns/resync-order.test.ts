import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Epoch = "Old" | "New";
type Snapshot = { readonly cursor: number; readonly total: number };
type Command =
  | { readonly kind: "append" | "pull" }
  | { readonly kind: "start" | "capture"; readonly epoch: Epoch }
  | {
      readonly kind: "deliver";
      readonly epoch: Epoch;
    };
type Fault = "none" | "ignore-epoch" | "ignore-position";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/lease/resync-order.ideal";

test(
  "resync compares request epoch and capture position against independent HTTP observations",
  { timeout: 30_000 },
  async (t) => {
    const cases = [
      {
        question: "NewAppliedWhileOldHoldsAheadImage",
        fault: "ignore-epoch",
        epoch: "Old",
        expected: { cursor: 1, total: 2 },
        observed: { cursor: 2, total: 4 },
      },
      {
        question: "CurrentCaptureFellBehind",
        fault: "ignore-position",
        epoch: "New",
        expected: { cursor: 2, total: 4 },
        observed: { cursor: 1, total: 2 },
      },
    ] as const;
    for (const scenario of cases) {
      const base = await reachableScenario<Command>(
        checker,
        model,
        scenario.question,
      );
      for (const fault of ["none", scenario.fault] as readonly Fault[]) {
        await t.test(`${scenario.question} / ${fault}`, async (t) => {
          const arrived = new EventEmitter();
          const requests = new Map<string, ServerResponse>();
          const captures = new Map<Epoch, Snapshot>();
          const outstanding = new Map<Epoch, Promise<void>>();
          const deltas: number[] = [];
          const server = createServer((request, response) => {
            const url = new URL(request.url ?? "/", "http://localhost");
            if (url.pathname === "/snapshot") {
              const epoch = url.searchParams.get("epoch") ?? "";
              requests.set(epoch, response);
              arrived.emit(epoch);
              return;
            }
            if (url.pathname === "/append" && request.method === "POST") {
              deltas.push(2);
              response.writeHead(204).end();
              return;
            }
            if (url.pathname === "/next") {
              const cursor = Number(url.searchParams.get("cursor"));
              const amount = deltas[cursor];
              if (amount === undefined) response.writeHead(204).end();
              else response.end(JSON.stringify({ cursor: cursor + 1, amount }));
              return;
            }
            response.writeHead(404).end();
          });
          server.listen(0, "127.0.0.1");
          await once(server, "listening");
          t.after(async () => {
            server.closeAllConnections();
            await new Promise<void>((resolve) => server.close(() => resolve()));
          });
          const address = server.address();
          assert.ok(address && typeof address !== "string");
          const origin = `http://127.0.0.1:${address.port}`;
          let latest: Epoch | undefined;
          let view: Snapshot = { cursor: 0, total: 0 };

          const suffix: Command[] = [
            {
              kind: "deliver",
              epoch: scenario.epoch,
            },
          ];
          if (scenario.epoch === "Old") suffix.push({ kind: "pull" });
          const result = await recordReplay<Command>(
            checker,
            { ...base, commands: [...base.commands, ...suffix] },
            {
              async apply(command) {
                switch (command.kind) {
                  case "append": {
                    const response = await fetch(`${origin}/append`, {
                      method: "POST",
                    });
                    assert.equal(response.status, 204);
                    break;
                  }
                  case "pull": {
                    const response = await fetch(
                      `${origin}/next?cursor=${view.cursor}`,
                    );
                    if (response.status === 204) break;
                    assert.equal(response.status, 200);
                    const delta = (await response.json()) as {
                      cursor: number;
                      amount: number;
                    };
                    view = {
                      cursor: delta.cursor,
                      total: view.total + delta.amount,
                    };
                    break;
                  }
                  case "start": {
                    const { epoch } = command;
                    latest = epoch;
                    const acknowledged = once(arrived, epoch);
                    const request = fetch(
                      `${origin}/snapshot?epoch=${epoch}`,
                    ).then(async (response) => {
                      assert.equal(response.status, 200);
                      const image = (await response.json()) as Snapshot;
                      if (
                        (fault === "ignore-epoch" || latest === epoch) &&
                        (fault === "ignore-position" ||
                          image.cursor >= view.cursor)
                      ) {
                        view = image;
                      }
                    });
                    void request.catch(() => {});
                    outstanding.set(epoch, request);
                    await Promise.race([acknowledged, request]);
                    break;
                  }
                  case "capture":
                    assert.ok(requests.has(command.epoch));
                    captures.set(command.epoch, {
                      cursor: deltas.length,
                      total: deltas.reduce((sum, amount) => sum + amount, 0),
                    });
                    break;
                  case "deliver": {
                    const response = requests.get(command.epoch);
                    const image = captures.get(command.epoch);
                    const request = outstanding.get(command.epoch);
                    assert.ok(response && image && request);
                    response.end(JSON.stringify(image));
                    await request;
                    requests.delete(command.epoch);
                    captures.delete(command.epoch);
                    outstanding.delete(command.epoch);
                    break;
                  }
                }
              },
              async observe() {
                return view;
              },
              project: ({ cursor, replicaTotal }) => ({
                cursor,
                total: replicaTotal,
              }),
            },
          );

          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(
              result.recording.observations[base.commands.length + 1],
              scenario.expected,
            );
            assert.deepEqual(result.recording.observations.at(-1), {
              cursor: 2,
              total: 4,
            });
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.deepEqual(result.difference, {
              step: base.commands.length + 1,
              command: {
                kind: "deliver",
                epoch: scenario.epoch,
              },
              expected: scenario.expected,
              observed: scenario.observed,
            });
          }
        });
      }
    }
  },
);
