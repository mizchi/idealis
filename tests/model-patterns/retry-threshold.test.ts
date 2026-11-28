import assert from "node:assert/strict";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import {
  observationPairScenarios,
  replayEvents,
  type PublicEvent,
} from "../../examples/model-patterns/replay.js";

type Command =
  | { kind: "start" | "finish" }
  | { kind: "retry"; cause: "Transient" | "Fatal" };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/retry-threshold.model";

async function retryService(
  t: TestContext,
  fault?: "extra" | "missing" | "payload",
) {
  let phase: "Ready" | "Pending" | "Done" = "Ready";
  let remaining = 2;
  let lastCause: "Transient" | "Fatal" | null = null;
  const events: PublicEvent[] = [];
  const emit = (event: string) =>
    events.push({
      event,
      observation: {
        phase,
        publicExhaustion:
          remaining === 0
            ? [
                [
                  fault === "payload" && event === "exhausted"
                    ? "Ready"
                    : phase,
                  "Exhausted",
                ],
              ]
            : [],
      },
    });
  const server = createServer((request, reply) => {
    const url = new URL(request.url ?? "/", "http://localhost");
    switch (url.pathname) {
      case "/start":
        phase = "Pending";
        emit("started");
        break;
      case "/retry":
        lastCause =
          url.searchParams.get("cause") === "Fatal" ? "Fatal" : "Transient";
        remaining = lastCause === "Fatal" ? 0 : remaining - 1;
        if ((remaining === 0 && fault !== "missing") || fault === "extra")
          emit("exhausted");
        break;
      case "/finish":
        phase = "Done";
        emit("finished");
        break;
      case "/state":
        reply.end(
          JSON.stringify({
            phase,
            lastCause,
            attempts:
              remaining === 2
                ? "NoneYet"
                : remaining === 1
                  ? "Once"
                  : "Exhausted",
          }),
        );
        return;
      case "/events":
        reply.end(JSON.stringify(events));
        return;
      default:
        reply.writeHead(404).end();
        return;
    }
    reply.writeHead(204).end();
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  t.after(async () => {
    await new Promise<void>((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
      server.closeAllConnections();
    });
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  const origin = `http://127.0.0.1:${address.port}`;
  async function read(path: string): Promise<unknown> {
    const response = await fetch(`${origin}/${path}`);
    assert.equal(response.status, 200);
    return response.json();
  }
  return {
    adapter: {
      async apply(command: Command) {
        const url = new URL(`/${command.kind}`, origin);
        if (command.kind === "retry")
          url.searchParams.set("cause", command.cause);
        assert.equal((await fetch(url, { method: "POST" })).status, 204);
      },
      observe: () => read("state"),
      observeEvents: async () => (await read("events")) as PublicEvent[],
      project: ({
        phase,
        attempts,
        lastCause,
      }: Readonly<Record<string, unknown>>) => ({
        phase,
        attempts,
        lastCause,
      }),
    },
  };
}

const emissions = [
  { action: "start", event: "started" },
  {
    action: "retry",
    event: "exhausted",
    when: "some after(exhaustion) and no before(exhaustion)",
  },
  { action: "finish", event: "finished" },
] as const;

test("model-derived events detect extra, missing and wrong HTTP notifications", async (t) => {
  for (const [at, distinguish, faults] of [
    ["Finished", "attempts", ["extra"]],
    ["ExhaustionReached", "lastCause", ["missing", "payload"]],
  ] as const) {
    const scenarios = await observationPairScenarios<Command>(checker, model, {
      at,
      observe: ["phase", "publicExhaustion"],
      distinguish: [distinguish],
      comparison: "history",
      emit: emissions,
    });
    const lengths = scenarios
      .map((scenario) => scenario.commands.length)
      .sort();
    assert.deepEqual(lengths, at === "Finished" ? [2, 3] : [3, 4]);
    for (const scenario of scenarios) {
      const runtime = await retryService(t);
      assert.equal(
        (await replayEvents(checker, scenario, runtime.adapter)).verdict,
        "matched",
      );
    }
    const retried = scenarios.find(
      (scenario) => scenario.commands.length === lengths[1],
    );
    assert.ok(retried);
    for (const fault of faults) {
      const runtime = await retryService(t, fault);
      const result: Awaited<ReturnType<typeof replayEvents<Command>>> =
        await replayEvents<Command>(checker, retried, runtime.adapter);
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, fault === "extra" ? 2 : 3);
      type Comparison = {
        state: unknown;
        events: PublicEvent[];
      };
      const expected = result.difference.expected as Comparison;
      const observed = result.difference.observed as Comparison;
      assert.deepEqual(observed.state, expected.state);
      assert.notDeepEqual(observed.events, expected.events);
      if (fault === "payload") {
        assert.deepEqual(expected.events.at(-1), {
          event: "exhausted",
          observation: {
            phase: "Pending",
            publicExhaustion: [["Pending", "Exhausted"]],
          },
        });
        assert.deepEqual(observed.events.at(-1), {
          event: "exhausted",
          observation: {
            phase: "Pending",
            publicExhaustion: [["Ready", "Exhausted"]],
          },
        });
      }
    }
  }
});
