import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer, type ServerResponse } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
  repeatedTemporalScenario,
} from "../../examples/model-patterns/replay.js";

type Request = "Long" | "Late";
type Outcome = "Pending" | "Delivered" | "Rejected" | "Dropped";
type Command =
  | { readonly kind: "submit" | "finish"; readonly request: Request }
  | { readonly kind: "beginDrain" | "stop" };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/http-drain/drain.model";

test(
  "HTTP shutdown rejects late work and lets existing responses finish",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "DrainingWithLongRequest",
    );
    assert.deepEqual(base.commands, [
      { kind: "submit", request: "Long" },
      { kind: "beginDrain" },
    ]);
    const commands: Command[] = [
      ...base.commands,
      { kind: "submit", request: "Late" },
      { kind: "stop" },
      { kind: "finish", request: "Long" },
      { kind: "stop" },
    ];
    const directory = await mkdtemp(join(tmpdir(), "idealis-http-drain-"));
    t.after(() => rm(directory, { recursive: true, force: true }));
    const unfair = join(directory, "unfair.model");
    await writeFile(
      unfair,
      (await readFile(model, "utf8")).replace(
        "weak finish(*) weak stop",
        "weak stop",
      ),
    );
    const stalled = await repeatedTemporalScenario<Command>(
      checker,
      unfair,
      "DrainStopsWithFairCompletion",
      3,
    );
    assert.ok(
      stalled.commands.filter(({ kind }) => kind === "stop").length >= 3,
    );
    assert.ok(stalled.commands.every(({ kind }) => kind !== "finish"));
    for (const fault of [
      "none",
      "accept-late",
      "stop-early",
      "stalled",
    ] as const) {
      await t.test(fault, async (t) => {
        const received = new EventEmitter();
        const held = new Map<Request, ServerResponse>();
        const replies = new Map<Request, Promise<void>>();
        const results = new Map<Request, Outcome>();
        let accepting = true;
        const server = createServer((request, response) => {
          const key = request.url === "/Long" ? "Long" : "Late";
          if (accepting || fault === "accept-late") held.set(key, response);
          else response.writeHead(503).end("draining");
          received.emit(key);
        });
        async function closeServer() {
          await new Promise<void>((resolve, reject) =>
            server.close((error) => (error ? reject(error) : resolve())),
          );
        }
        server.listen(0, "127.0.0.1");
        await once(server, "listening");
        t.after(async () => {
          server.closeAllConnections();
          if (server.listening) await closeServer();
        });
        const address = server.address();
        assert.ok(address && typeof address !== "string");
        const origin = `http://127.0.0.1:${address.port}`;

        const result = await recordReplay<Command>(
          checker,
          fault === "stalled" ? stalled : { ...base, commands },
          {
            async apply(command) {
              switch (command.kind) {
                case "submit": {
                  const { request } = command;
                  const ready = once(received, request);
                  const reply = fetch(`${origin}/${request}`).then(
                    async (response) => {
                      const body = await response.text();
                      if (response.status === 200 && body === `done:${request}`)
                        results.set(request, "Delivered");
                      else if (response.status === 503 && body === "draining")
                        results.set(request, "Rejected");
                      else
                        assert.fail(
                          `unexpected HTTP response: ${response.status} ${body}`,
                        );
                    },
                    (error: unknown) => {
                      assert.ok(error instanceof TypeError);
                      results.set(request, "Dropped");
                    },
                  );
                  void reply.catch(() => {});
                  replies.set(request, reply);
                  await Promise.race([ready, reply]);
                  if (!held.has(request)) await reply;
                  break;
                }
                case "beginDrain":
                  accepting = false;
                  break;
                case "stop":
                  if (held.size > 0 && fault !== "stop-early") break;
                  if (fault === "stop-early") {
                    for (const response of held.values()) response.destroy();
                    held.clear();
                  }
                  await closeServer();
                  await Promise.all(replies.values());
                  break;
                case "finish": {
                  const { request } = command;
                  const outgoing = held.get(request);
                  assert.ok(outgoing);
                  outgoing.end(`done:${request}`);
                  await replies.get(request);
                  held.delete(request);
                  break;
                }
              }
            },
            async observe() {
              const outcome = (request: Request) =>
                held.has(request) ? "Pending" : (results.get(request) ?? null);
              return {
                phase: !server.listening
                  ? "Halted"
                  : accepting
                    ? "Accepting"
                    : "Draining",
                results: { Long: outcome("Long"), Late: outcome("Late") },
              };
            },
            project: ({ phase, results }) => ({ phase, results }),
          },
        );

        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[4], {
            phase: "Draining",
            results: { Long: "Pending", Late: "Rejected" },
          });
          assert.deepEqual(result.recording.observations.at(-1), {
            phase: "Halted",
            results: { Long: "Delivered", Late: "Rejected" },
          });
        } else if (fault === "stalled") {
          assert.equal(result.verdict, "matched");
          const firstStop = stalled.commands.findIndex(
            ({ kind }) => kind === "stop",
          );
          assert.ok(firstStop > 0);
          const pending = stalled.commands.find(
            (command) => command.kind === "submit",
          );
          assert.ok(pending?.kind === "submit");
          const expected = {
            phase: "Draining",
            results: { Long: null, Late: null, [pending.request]: "Pending" },
          };
          for (const observation of result.recording.observations.slice(
            firstStop + 1,
          )) {
            assert.deepEqual(observation, expected);
          }
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, fault === "accept-late" ? 3 : 4);
          assert.deepEqual(
            result.difference.observed,
            fault === "accept-late"
              ? {
                  phase: "Draining",
                  results: { Long: "Pending", Late: "Pending" },
                }
              : {
                  phase: "Halted",
                  results: { Long: "Dropped", Late: "Rejected" },
                },
          );
        }
      });
    }
  },
);
