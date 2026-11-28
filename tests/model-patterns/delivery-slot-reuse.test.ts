import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import { readPinnedDelivery } from "../../examples/model-patterns/subscriptions/delivery-stream.js";

type Slot = "Primary" | "Hedge";
type Ticket = "First" | "Second" | "Third";
type Epoch = "Initial" | "AfterFirst" | "AfterSecond";
type Command =
  | { kind: "connect"; slot: Slot; ticket: Ticket }
  | { kind: "detach"; slot: Slot }
  | { kind: "restart" }
  | { kind: "finish"; ticket: Ticket };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/subscriptions/delivery-slot-reuse.ideal";
const id = "notice-1";
const finish = (ticket: Ticket): Command => ({ kind: "finish", ticket });

test(
  "an old connection cannot clean up or publish for a reused slot",
  { timeout: 30_000 },
  async (t) => {
    const repeated = await reachableScenario<Command>(
      checker,
      model,
      "SuccessiveRestarts",
    );
    const sameEpoch = await reachableScenario<Command>(
      checker,
      model,
      "SameEpochReplacement",
    );
    for (const scenario of [
      {
        name: "two restarts with late completions on both sides of publication",
        prefix: repeated,
        tail: ["First", "Third", "Second"],
        fault: null,
        published: { Primary: "Third", Hedge: null },
      },
      {
        name: "same epoch replacement leaves another slot independent",
        prefix: sameEpoch,
        tail: ["First", "Third", "Second"],
        fault: null,
        published: { Primary: "Second", Hedge: "Third" },
      },
      {
        name: "unconditional cleanup of the old slot",
        prefix: repeated,
        tail: ["First"],
        fault: "cleanup-by-slot",
        published: null,
      },
      {
        name: "publication keyed only by slot and epoch",
        prefix: sameEpoch,
        tail: ["First"],
        fault: "publish-by-epoch",
        published: null,
      },
    ] as const) {
      await t.test(scenario.name, async (t) => {
        const server = createServer();
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

        type Connection = {
          ticket: Ticket;
          slot: Slot;
          epoch: Epoch;
          reading: Promise<Response>;
          writer: ServerResponse;
        };
        let epoch: Epoch = "Initial";
        const pending = new Map<Ticket, Connection>();
        const current = new Map<Slot, Connection>();
        const published = new Map<Slot, Ticket>();
        const issued: Ticket[] = [];
        const toSlots = <V>(
          entries: ReadonlyMap<Slot, V>,
          encode: (value: V) => unknown,
        ) =>
          Object.fromEntries(
            (["Primary", "Hedge"] as const).map((slot) => {
              const entry = entries.get(slot);
              return [slot, entry === undefined ? null : encode(entry)];
            }),
          );

        const result = await recordReplay(
          checker,
          {
            ...scenario.prefix,
            commands: [
              ...scenario.prefix.commands,
              ...scenario.tail.map(finish),
            ],
          },
          {
            async apply(command) {
              switch (command.kind) {
                case "connect": {
                  const arrived = once(server, "request");
                  const reading = fetch(
                    `http://127.0.0.1:${address.port}/delivery?slot=${command.slot}&revision=1`,
                    { signal: AbortSignal.timeout(10_000) },
                  );
                  void reading.catch(() => {});
                  const [request, writer] = await Promise.race([
                    arrived,
                    reading.then(() => {
                      throw new Error(
                        "HTTP request completed before its connection was captured",
                      );
                    }),
                  ]);
                  assert.equal(
                    request.url,
                    `/delivery?slot=${command.slot}&revision=1`,
                  );
                  const connection: Connection = {
                    ticket: command.ticket,
                    slot: command.slot,
                    epoch,
                    reading,
                    writer,
                  };
                  issued.push(command.ticket);
                  pending.set(command.ticket, connection);
                  current.set(command.slot, connection);
                  break;
                }
                case "detach":
                  current.delete(command.slot);
                  break;
                case "restart":
                  epoch = epoch === "Initial" ? "AfterFirst" : "AfterSecond";
                  current.clear();
                  published.clear();
                  break;
                case "finish": {
                  const connection = pending.get(command.ticket);
                  assert.ok(connection);
                  connection.writer.writeHead(200, {
                    "Content-Type": "application/x-ndjson",
                    "X-Delivery-Revision": "1",
                  });
                  connection.writer.end(
                    ["notice", "end"]
                      .map((kind) => JSON.stringify({ kind, id }) + "\n")
                      .join(""),
                  );
                  assert.equal(
                    await readPinnedDelivery(await connection.reading, id, 1),
                    "Delivered",
                  );
                  pending.delete(command.ticket);

                  const ownsSlot = current.get(connection.slot) === connection;
                  if (ownsSlot || scenario.fault === "cleanup-by-slot")
                    current.delete(connection.slot);
                  if (
                    ownsSlot ||
                    (scenario.fault === "publish-by-epoch" &&
                      connection.epoch === epoch)
                  ) {
                    published.set(connection.slot, command.ticket);
                  }
                  break;
                }
              }
            },
            async observe() {
              return {
                epoch,
                issued: [...issued].sort(),
                pending: Object.fromEntries(
                  (["First", "Second", "Third"] as const).map((ticket) => {
                    const connection = pending.get(ticket);
                    return [
                      ticket,
                      connection
                        ? { slot: connection.slot, epoch: connection.epoch }
                        : null,
                    ];
                  }),
                ),
                current: toSlots(current, (connection) => connection.ticket),
                published: toSlots(published, (ticket) => ticket),
              };
            },
            project: ({ epoch, issued, pending, current, published }) => ({
              epoch,
              issued,
              pending,
              current,
              published,
            }),
          },
        );
        if (scenario.fault) {
          assert.equal(result.verdict, "mismatch");
          assert.equal(
            result.difference.step,
            scenario.prefix.commands.length + 1,
          );
          assert.deepEqual(result.difference.command, finish("First"));
          const expected = result.difference.expected as {
            current: object;
            published: object;
          };
          const observed = result.difference.observed as typeof expected;
          if (scenario.fault === "cleanup-by-slot") {
            assert.deepEqual(expected.current, {
              Primary: "Third",
              Hedge: null,
            });
            assert.deepEqual(observed.current, { Primary: null, Hedge: null });
            assert.deepEqual(observed.published, expected.published);
          } else {
            assert.deepEqual(expected.published, {
              Primary: null,
              Hedge: null,
            });
            assert.deepEqual(observed.published, {
              Primary: "First",
              Hedge: null,
            });
            assert.deepEqual(observed.current, expected.current);
          }
        } else {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), {
            epoch: scenario.prefix === repeated ? "AfterSecond" : "Initial",
            issued: ["First", "Second", "Third"],
            pending: { First: null, Second: null, Third: null },
            current: { Primary: null, Hedge: null },
            published: scenario.published,
          });
        }
      });
    }
  },
);
