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
  readDelivery,
} from "../../examples/model-patterns/subscriptions/delivery-stream.js";

type Delivery = "First" | "Second";
type Command =
  | { kind: "connect"; delivery: Delivery }
  | { kind: "interrupt" }
  | { kind: "finish"; marker: Delivery; hasNotice: boolean };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/delivery-reconnect.model";
const line = (kind: "notice" | "end", id: string) =>
  JSON.stringify({ kind, id }) + "\n";

test(
  "a delivery stays pending across an interrupted stream and a foreign end frame",
  { timeout: 30_000 },
  async (t) => {
    const cut = await reachableScenario<Command>(checker, model, "Interrupted");
    const target = cut.commands.find(
      (command) => command.kind === "connect",
    )?.delivery;
    assert.ok(target);
    const other: Delivery = target === "First" ? "Second" : "First";
    const commands: Command[] = [
      ...cut.commands,
      { kind: "connect", delivery: target },
      { kind: "finish", marker: other, hasNotice: true },
      { kind: "connect", delivery: target },
      { kind: "finish", marker: target, hasNotice: true },
      { kind: "connect", delivery: other },
      { kind: "finish", marker: other, hasNotice: false },
    ];

    for (const mode of [
      "abort",
      "eof-without-end",
      "eof-mid-end",
      "foreign-last-end",
    ] as const) {
      await t.test(mode, async (t) => {
        let writer: ServerResponse | undefined;
        let requested: string | null = null;
        const server = createServer((request, response) => {
          requested = new URL(
            request.url ?? "/",
            "http://localhost",
          ).searchParams.get("id");
          writer = response;
          response.writeHead(200, { "Content-Type": "application/x-ndjson" });
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

        const outcome: Record<Delivery, DeliveryResult> = {
          First: "Pending",
          Second: "Pending",
        };
        let connected: Delivery | null = null;
        let response: Response | undefined;
        const result = await recordReplay(
          checker,
          { ...cut, commands },
          {
            async apply(command) {
              if (command.kind === "connect") {
                response = await fetch(
                  `http://127.0.0.1:${address.port}/?id=${command.delivery}`,
                );
                assert.equal(requested, command.delivery);
                connected = command.delivery;
                return;
              }
              assert.ok(writer && connected && response);
              const reading = readDelivery(response, connected);
              if (command.kind === "interrupt") {
                if (mode === "eof-without-end")
                  writer.end(line("notice", connected));
                else if (mode === "eof-mid-end")
                  writer.end(
                    line("notice", connected) +
                      line("end", connected).slice(0, -3),
                  );
                else writer.destroy();
              } else {
                const notice = line(
                  "notice",
                  command.hasNotice ? connected : target,
                );
                const marker =
                  mode === "foreign-last-end" && connected === other
                    ? target
                    : command.marker;
                writer.end(notice + line("end", marker));
              }
              outcome[connected] = await reading;
              connected = null;
            },
            async observe() {
              return { connected, outcome: { ...outcome } };
            },
            project: ({ connected, outcome }) => ({ connected, outcome }),
          },
        );

        if (mode === "foreign-last-end") {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, commands.length);
          assert.deepEqual(result.difference.expected, {
            connected: null,
            outcome: { [target]: "Delivered", [other]: "Missing" },
          });
          assert.deepEqual(result.difference.observed, {
            connected: null,
            outcome: { [target]: "Delivered", [other]: "Pending" },
          });
          return;
        }
        assert.equal(result.verdict, "matched");
        for (const step of [cut.commands.length, cut.commands.length + 2]) {
          assert.deepEqual(result.recording.observations[step], {
            connected: null,
            outcome: { First: "Pending", Second: "Pending" },
          });
        }
        assert.deepEqual(result.recording.observations.at(-1), {
          connected: null,
          outcome: { [target]: "Delivered", [other]: "Missing" },
        });
      });
    }
  },
);
