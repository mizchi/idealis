import assert from "node:assert/strict";
import { createServer, type ServerResponse } from "node:http";
import test, { type TestContext } from "node:test";
import {
  observationPairScenarios,
  reachableScenario,
  replayEvents,
  type PublicEvent,
} from "../../examples/model-patterns/replay.js";
import { taskGate } from "./helpers/task-gate.js";

type Command = { kind: "accept" | "reply" | "dispatch" | "receive" };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/delayed-notice.model";

async function delivery(t: TestContext, dropNotice = false) {
  const entered = taskGate();
  const allowReply = taskGate();
  const state = {
    accepted: false,
    replied: false,
    dispatched: false,
    checked: false,
    beforeResponse: false,
  };
  const events: PublicEvent[] = [];
  let submit: Promise<Response> | undefined;
  let stream: ServerResponse | undefined;
  const server = createServer(async (request, response) => {
    if (request.url === "/work" && request.method === "POST") {
      state.accepted = true;
      entered.open();
      await allowReply.promise;
      response.writeHead(202).end();
    } else if (request.url === "/stream") {
      stream = response;
      response.writeHead(200, { "Content-Type": "application/x-ndjson" });
      response.flushHeaders();
    } else if (
      request.url === "/dispatch" &&
      request.method === "POST" &&
      stream
    ) {
      state.dispatched = true;
      if (!dropNotice)
        stream.write(
          `${JSON.stringify({ event: "notice", observation: { checked: true } })}\n`,
        );
      stream.end(`${JSON.stringify({ end: true })}\n`);
      response.writeHead(204).end();
    } else response.writeHead(404).end();
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  t.after(async () => {
    allowReply.open();
    await new Promise<void>((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
      server.closeAllConnections();
    });
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  const origin = `http://127.0.0.1:${address.port}`;
  const subscription = await fetch(`${origin}/stream`);
  assert.equal(subscription.status, 200);
  return {
    async apply({ kind }: Command) {
      switch (kind) {
        case "accept":
          submit = fetch(`${origin}/work`, { method: "POST" });
          assert.equal(
            await Promise.race([entered.promise, submit]),
            undefined,
          );
          break;
        case "reply":
          assert.ok(submit);
          allowReply.open();
          assert.equal((await submit).status, 202);
          state.replied = true;
          break;
        case "dispatch":
          assert.equal(
            (await fetch(`${origin}/dispatch`, { method: "POST" })).status,
            204,
          );
          break;
        case "receive": {
          const lines = (await subscription.text()).trim().split("\n");
          assert.deepEqual(JSON.parse(lines.pop()!), { end: true });
          for (const line of lines)
            events.push(JSON.parse(line) as PublicEvent);
          state.checked = true;
          state.beforeResponse = !state.replied;
          break;
        }
      }
    },
    async observe() {
      return { ...state };
    },
    async observeEvents() {
      return [...events];
    },
    project: (value: Readonly<Record<string, unknown>>) => value,
  };
}

test("a successful HTTP response and a streamed notification have separate observation points", async (t) => {
  const pending = await reachableScenario<Command>(
    checker,
    model,
    "RepliedWhilePending",
  );
  assert.deepEqual(pending.commands, [{ kind: "accept" }, { kind: "reply" }]);
  const pendingVerdict = await replayEvents(
    checker,
    { ...pending, events: [] },
    await delivery(t),
  );
  assert.equal(pendingVerdict.verdict, "matched");

  const scenarios = await observationPairScenarios<Command>(checker, model, {
    at: "Completed",
    observe: ["checked"],
    distinguish: ["beforeResponse"],
    comparison: "history",
    silent: ["accept", "reply", "dispatch"],
    emit: [{ action: "receive", event: "notice" }],
  });
  assert.deepEqual(
    scenarios
      .map(
        ({ commands }) =>
          commands.findIndex(({ kind }) => kind === "reply") <
          commands.findIndex(({ kind }) => kind === "receive"),
      )
      .sort(),
    [false, true],
  );
  for (const scenario of scenarios) {
    const verdict = await replayEvents(checker, scenario, await delivery(t));
    assert.equal(verdict.verdict, "matched");
  }
  const repliedFirst = scenarios.find(
    ({ commands }) =>
      commands.findIndex((command) => command.kind === "reply") <
      commands.findIndex((command) => command.kind === "receive"),
  );
  assert.ok(repliedFirst);
  const dropped = await replayEvents(
    checker,
    repliedFirst,
    await delivery(t, true),
  );
  assert.equal(dropped.verdict, "mismatch");
  assert.deepEqual(dropped.difference.command, { kind: "receive" });
  assert.deepEqual(dropped.difference.expected, {
    state: {
      accepted: true,
      replied: true,
      dispatched: true,
      checked: true,
      beforeResponse: false,
    },
    events: [{ event: "notice", observation: { checked: true } }],
  });
  assert.deepEqual(dropped.difference.observed, {
    state: {
      accepted: true,
      replied: true,
      dispatched: true,
      checked: true,
      beforeResponse: false,
    },
    events: [],
  });
});
