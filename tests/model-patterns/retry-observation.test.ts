import assert from "node:assert/strict";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import {
  observationPairScenarios,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command = { kind: "start" | "retry" | "finish" };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/retry-observation.model";

async function notifications(t: TestContext, exposeRetry: boolean) {
  let phase: "Ready" | "Pending" | "Done" = "Ready";
  let notice = false;
  let retried = false;
  const events: string[] = [];
  const server = createServer((request, response) => {
    switch (request.url) {
      case "/start":
        events.push("started");
        phase = "Pending";
        notice = true;
        break;
      case "/retry":
        if (exposeRetry) events.push("retrying");
        retried = true;
        notice = false;
        break;
      case "/finish":
        events.push("finished");
        phase = "Done";
        notice = false;
        break;
      case "/events":
        response.setHeader("Content-Type", "application/json");
        response.end(JSON.stringify(events));
        return;
      case "/state":
        response.setHeader("Content-Type", "application/json");
        response.end(JSON.stringify({ phase, notice, retried }));
        return;
      default:
        response.writeHead(404).end();
        return;
    }
    response.writeHead(204).end();
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
  async function read(route: string): Promise<unknown> {
    const response = await fetch(`${origin}/${route}`);
    assert.equal(response.status, 200);
    return response.json();
  }
  return {
    readEvents: () => read("events"),
    adapter: {
      async apply(command: Command) {
        const response = await fetch(`${origin}/${command.kind}`, {
          method: "POST",
        });
        assert.equal(response.status, 204);
      },
      observe: () => read("state"),
      project: ({
        phase,
        notice,
        retried,
      }: Readonly<Record<string, unknown>>) => ({ phase, notice, retried }),
    },
  };
}

test("an internal retry adds a command without adding an HTTP notification", async (t) => {
  const scenarios = await observationPairScenarios<Command>(checker, model, {
    at: "QuietDone",
    observe: ["phase", "notice"],
    distinguish: ["retried"],
    comparison: "history",
    silent: ["retry"],
  });
  assert.deepEqual(scenarios.map((run) => run.commands.length).sort(), [2, 3]);
  for (const exposeRetry of [false, true]) {
    const events = await Promise.all(
      scenarios.map(async (scenario) => {
        const runtime = await notifications(t, exposeRetry);
        const run = await recordReplay(checker, scenario, runtime.adapter);
        assert.equal(run.verdict, "matched");
        return runtime.readEvents();
      }),
    );
    const expected = scenarios.map((scenario) =>
      exposeRetry && scenario.commands.some(({ kind }) => kind === "retry")
        ? ["started", "retrying", "finished"]
        : ["started", "finished"],
    );
    assert.deepEqual(events, expected);
  }
});
