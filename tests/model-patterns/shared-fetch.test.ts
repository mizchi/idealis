import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Caller = "First" | "Second";
type Command =
  | { readonly kind: "join" | "cancel"; readonly caller: Caller }
  | { readonly kind: "complete" };
type Result = "Done" | "Cancelled" | "Failed";
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/cancellation/shared-fetch.ideal";

test(
  "shared fetch aborts its HTTP transport only when the last waiter cancels",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "BothCallersAreWaiting",
    );
    const cases = [
      {
        name: "one cancels",
        fault: "abort-first",
        tail: [{ kind: "complete" }, { kind: "cancel", caller: "Second" }],
        expected: {
          phase: "Completed",
          results: { First: "Cancelled", Second: "Done" },
          started: true,
        },
      },
      {
        name: "both cancel",
        fault: "never-abort",
        tail: [{ kind: "cancel", caller: "Second" }, { kind: "complete" }],
        expected: {
          phase: "Aborted",
          results: { First: "Cancelled", Second: "Cancelled" },
          started: true,
        },
      },
    ] as const;
    for (const scenario of cases) {
      for (const fault of ["none", scenario.fault] as const) {
        await t.test(`${scenario.name} / ${fault}`, async (t) => {
          let backingRequests = 0;
          let socket: ServerResponse | undefined;
          let closed: Promise<unknown> | undefined;
          let peerAborted = false;
          let peerCompleted = false;
          const server = createServer((_request, reply) => {
            backingRequests += 1;
            socket = reply;
            closed = once(reply, "close").then(() => {
              peerAborted = !reply.writableFinished;
              peerCompleted = reply.writableFinished;
            });
            server.emit("requested");
          });
          server.listen(0, "127.0.0.1");
          await once(server, "listening");
          t.after(async () => {
            server.closeAllConnections();
            await new Promise<void>((resolve) => server.close(() => resolve()));
          });
          const address = server.address();
          assert.ok(address && typeof address !== "string");
          const url = `http://127.0.0.1:${address.port}`;
          const controller = new AbortController();
          const waiting = new Map<Caller, (result: Result) => void>();
          const callers = new Map<Caller, Promise<void>>();
          const results: Record<Caller, Result | null> = {
            First: null,
            Second: null,
          };
          let underlying: Promise<void> | undefined;
          const settleAll = (value: Result) => {
            for (const settle of waiting.values()) settle(value);
            waiting.clear();
          };
          const commands: Command[] = [
            ...base.commands,
            { kind: "cancel", caller: "First" },
            ...scenario.tail,
          ];
          const result = await recordReplay<Command>(
            checker,
            { ...base, commands },
            {
              async apply(command) {
                switch (command.kind) {
                  case "join": {
                    const caller = command.caller;
                    const outcome = new Promise<Result>((resolve) =>
                      waiting.set(caller, resolve),
                    );
                    callers.set(
                      caller,
                      outcome.then((value) => {
                        results[caller] = value;
                      }),
                    );
                    if (!underlying) {
                      const requested = once(server, "requested");
                      underlying = fetch(url, {
                        signal: controller.signal,
                      }).then(
                        async (response) => {
                          assert.equal(response.status, 200);
                          assert.equal(await response.text(), "shared");
                          settleAll("Done");
                        },
                        (error: unknown) => {
                          assert.ok(
                            error instanceof DOMException &&
                              error.name === "AbortError",
                          );
                          settleAll("Failed");
                        },
                      );
                      void underlying.catch(() => {});
                      await Promise.race([requested, underlying]);
                    }
                    break;
                  }
                  case "cancel": {
                    const settle = waiting.get(command.caller);
                    if (!settle) break;
                    waiting.delete(command.caller);
                    settle("Cancelled");
                    await callers.get(command.caller);
                    if (
                      fault === "abort-first" ||
                      (waiting.size === 0 && fault !== "never-abort")
                    ) {
                      controller.abort();
                      await underlying;
                      await closed;
                    }
                    break;
                  }
                  case "complete":
                    assert.ok(socket);
                    if (peerAborted || peerCompleted) break;
                    socket.end("shared");
                    await underlying;
                    await closed;
                    await Promise.all(callers.values());
                    break;
                }
              },
              async observe() {
                assert.ok(backingRequests <= 1);
                const phase = peerCompleted
                  ? "Completed"
                  : peerAborted
                    ? "Aborted"
                    : backingRequests === 1
                      ? "Running"
                      : "Idle";
                return {
                  phase,
                  results: { ...results },
                  started: backingRequests === 1,
                };
              },
              project: ({ phase, results }) => ({
                phase,
                results,
                started: phase !== "Idle",
              }),
            },
          );
          assert.equal(backingRequests, 1);
          if (fault === "none") {
            assert.equal(result.verdict, "matched");
            assert.deepEqual(
              result.recording.observations.at(-1),
              scenario.expected,
            );
          } else {
            assert.equal(result.verdict, "mismatch");
            assert.equal(
              result.difference.step,
              fault === "abort-first" ? 3 : 4,
            );
            assert.deepEqual(
              result.difference.observed,
              fault === "abort-first"
                ? {
                    phase: "Aborted",
                    results: { First: "Cancelled", Second: "Failed" },
                    started: true,
                  }
                : {
                    phase: "Running",
                    results: { First: "Cancelled", Second: "Cancelled" },
                    started: true,
                  },
            );
          }
        });
      }
    }
  },
);
