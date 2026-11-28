import assert from "node:assert/strict";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import {
  observationPairScenarios,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command =
  | { kind: "publish"; visible: boolean; sent: boolean }
  | { kind: "acknowledge" | "wait" };
type Policy = "Strict" | "Optional";
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/subscriptions/notice-history.ideal";

async function notifications(t: TestContext, policy: Policy) {
  let phase: "Ready" | "Published" | "Done" = "Ready";
  let notice = false;
  const outbox: string[] = [];
  const server = createServer(async (request, response) => {
    if (request.method === "GET") {
      response.setHeader("Content-Type", "application/json");
      response.end(JSON.stringify({ phase, notice }));
      return;
    }
    if (request.url === "/publish") {
      const chunks: Buffer[] = [];
      for await (const chunk of request) chunks.push(chunk);
      const { sent, visible } = JSON.parse(
        Buffer.concat(chunks).toString(),
      ) as Extract<Command, { kind: "publish" }>;
      if (sent) outbox.push("delivery");
      notice = sent && (policy === "Strict" || visible);
      phase = "Published";
    } else if (request.url === "/acknowledge") {
      notice = false;
      phase = "Done";
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
  return {
    outbox,
    adapter: {
      async apply(command: Command) {
        const response = await fetch(`${origin}/${command.kind}`, {
          method: "POST",
          body: JSON.stringify(command),
        });
        assert.equal(response.status, 204);
      },
      async observe() {
        const response = await fetch(origin);
        assert.equal(response.status, 200);
        return response.json();
      },
      project: ({ phase, notice }: Readonly<Record<string, unknown>>) => ({
        phase,
        notice,
      }),
    },
  };
}

test("one transient HTTP notification distinguishes two equal final observations", async (t) => {
  for (const [policy, comparison] of [
    ["Strict", "snapshot"],
    ["Optional", "history"],
  ] as const) {
    const scenarios = await observationPairScenarios<Command>(checker, model, {
      at: `${policy}Done`,
      comparison,
      observe: ["phase", "notice"],
      distinguish: ["delivered"],
    });
    const records = await Promise.all(
      scenarios.map(async (scenario) => {
        assert.equal(scenario.config.policy, policy);
        const runtime = await notifications(t, policy);
        const result = await recordReplay(checker, scenario, runtime.adapter);
        assert.equal(result.verdict, "matched");
        return {
          notices: result.recording.observations,
          delivered: runtime.outbox.length,
        };
      }),
    );
    const [left, right] = records;
    assert.ok(left && right);
    assert.notEqual(left.delivered, right.delivered);
    assert.deepEqual(left.notices.at(-1), right.notices.at(-1));
    if (comparison === "history") assert.deepEqual(left.notices, right.notices);
    else assert.notDeepEqual(left.notices[1], right.notices[1]);
  }
});
