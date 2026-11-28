import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test, { type TestContext } from "node:test";
import { promisify } from "node:util";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";
import {
  type Amount,
  type Command,
  type Domain,
  type Nat,
  type Order,
  type Payment,
  Orders,
  commandFromJson,
  decodeNat,
  stateToJson,
} from "../idealis-check/generated/checkout.js";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/checkout/checkout.ideal";
const run = promisify(execFile);

function createCheckout(chargeDuplicate: boolean): Domain {
  const requested = new Map<Order, Amount>();
  const payment = new Map<Order, Payment>();
  const chargeCount = new Map<Order, Nat>();
  for (const order of Orders) {
    payment.set(order, { tag: "Unplaced" });
    chargeCount.set(order, decodeNat(0));
  }
  return {
    get state() {
      return {
        requested: new Map(requested),
        payment: new Map(payment),
        chargeCount: new Map(chargeCount),
      };
    },
    apply(command) {
      const { order } = command;
      const current = payment.get(order);
      switch (command.kind) {
        case "place":
          if (current?.tag !== "Unplaced" || command.amount.units === 0)
            return false;
          requested.set(order, command.amount);
          payment.set(order, { tag: "Pending" });
          return true;
        case "reject":
          if (current?.tag !== "Pending") return false;
          payment.set(order, { tag: "Rejected", value: command.reason });
          return true;
        case "capture":
          if (requested.get(order)?.units !== command.amount.units)
            return false;
          if (current?.tag !== "Pending" && current?.tag !== "Captured")
            return false;
          if (current.tag === "Captured" && !chargeDuplicate) return true;
          payment.set(order, { tag: "Captured", value: command.amount });
          chargeCount.set(order, decodeNat((chargeCount.get(order) ?? 0) + 1));
          return true;
      }
    },
  };
}

async function serveCheckout(t: TestContext, chargeDuplicate = false) {
  const domain = createCheckout(chargeDuplicate);
  let writes = 0;
  const server = createServer(async (request, response) => {
    if (request.method === "GET" && request.url === "/state") {
      response.writeHead(200, { "Content-Type": "application/json" });
      response.end(JSON.stringify(stateToJson(domain.state)));
      return;
    }
    if (request.method !== "POST" || request.url !== "/commands") {
      response.writeHead(404).end();
      return;
    }
    writes++;
    try {
      request.setEncoding("utf8");
      let body = "";
      for await (const chunk of request) body += chunk;
      const command = commandFromJson(JSON.parse(body));
      response.writeHead(domain.apply(command) ? 204 : 409).end();
    } catch {
      response.writeHead(400).end();
    }
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
  const origin = `http://127.0.0.1:${address.port}`;
  return {
    async apply(command: Command) {
      assert.equal(
        (
          await fetch(`${origin}/commands`, {
            method: "POST",
            body: JSON.stringify(command),
          })
        ).status,
        204,
      );
    },
    async observe() {
      const response = await fetch(`${origin}/state`);
      assert.equal(response.status, 200);
      return response.json();
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
    writes: () => writes,
  };
}

test(
  "one checkout model supplies the glossary, generated types and HTTP replay",
  { timeout: 30_000 },
  async (t) => {
    const [
      { stdout: glossary },
      { stdout: generated },
      checkedIn,
      capture,
      rejection,
      { stdout: liveness },
    ] = await Promise.all([
      run(checker, ["generate", model, "glossary"]),
      run(checker, ["generate", model, "typescript"]),
      readFile("tests/idealis-check/generated/checkout.ts", "utf8"),
      reachableScenario<Command>(checker, model, "CanCapture", {
        checker: "rprob",
      }),
      reachableScenario<Command>(checker, model, "CanReject", {
        checker: "rprob",
      }),
      run(checker, [
        model,
        "--checker",
        "explicit",
        "--question",
        "OrderDecides",
      ]),
    ]);
    assert.deepEqual(JSON.parse(glossary), {
      Order: "顧客が金額を指定して決済を依頼する注文。",
    });
    assert.equal(generated, checkedIn);
    const [
      {
        answers: [progress],
      },
    ] = JSON.parse(liveness);
    assert.equal(progress.verdict, "holds");
    assert.equal(progress.coverage.kind, "complete");

    const callback = capture.commands.at(-1);
    assert.ok(callback?.kind === "capture");
    const duplicate = { ...capture, commands: [...capture.commands, callback] };
    const saved = await mkdtemp(join(tmpdir(), "idealis-checkout-itf-"));
    t.after(() => rm(saved, { recursive: true, force: true }));
    const traceFile = join(saved, "checkout.itf.json");
    const recordingFile = join(saved, "observations.json");
    const nativeReplay = () =>
      run(
        resolve(checker),
        ["replay-itf", resolve(model), "checkout.itf.json"],
        {
          cwd: saved,
        },
      );
    const project = (state: Readonly<Record<string, unknown>>) => state;
    for (const scenario of [rejection, duplicate]) {
      const result = await recordReplay(
        checker,
        scenario,
        await serveCheckout(t),
      );
      assert.equal(result.verdict, "matched");
      const itf = toReplayItf(result.recording);
      const { config, initial, commands } = result.recording.scenario;
      await writeFile(
        recordingFile,
        JSON.stringify({
          config,
          initial,
          commands,
          observations: result.recording.observations,
        }),
      );
      const { stdout } = await run(checker, [
        "record-itf",
        model,
        recordingFile,
      ]);
      assert.deepEqual(JSON.parse(stdout), itf);
      await writeFile(traceFile, stdout);
      assert.deepEqual(JSON.parse((await nativeReplay()).stdout), {
        verdict: "matched",
        steps: commands.length,
      });
      assert.equal(
        (
          await replayRecording(
            checker,
            fromReplayItf(JSON.parse(await readFile(traceFile, "utf8"))),
            project,
          )
        ).verdict,
        "matched",
      );
      const truncated = { ...itf, states: itf.states.slice(0, -1) };
      await assert.rejects(
        replayRecording(checker, fromReplayItf(truncated), project),
        /missing observation/,
      );
      const reordered = { ...itf, states: [...itf.states].reverse() };
      assert.throws(
        () => fromReplayItf(reordered),
        /inconsistent ITF replay frame 0/,
      );
    }

    const faulty = await recordReplay(
      checker,
      { ...duplicate, commands: [...duplicate.commands, callback] },
      await serveCheckout(t, true),
    );
    assert.equal(faulty.verdict, "mismatch");
    assert.equal(faulty.difference.step, 3);
    assert.deepEqual(faulty.difference.command, callback);
    const expected = {
      requested: { [callback.order]: { units: 2 } },
      payment: { [callback.order]: { tag: "Captured", value: { units: 2 } } },
      chargeCount: { [callback.order]: 1 },
    };
    assert.deepEqual(faulty.difference.expected, expected);
    assert.deepEqual(faulty.difference.observed, {
      ...expected,
      chargeCount: { [callback.order]: 2 },
    });
    const failureTrace = toReplayItf(faulty.recording);
    const initial = failureTrace.states[0]?.observation;
    assert.deepEqual(initial?.requested, { [callback.order]: { "#tup": [] } });
    assert.deepEqual(failureTrace.states.at(-1)?.observation.chargeCount, {
      [callback.order]: { "#bigint": "2" },
    });
    await writeFile(traceFile, JSON.stringify(failureTrace));
    await assert.rejects(nativeReplay(), (error: unknown) => {
      assert.ok(error instanceof Error && "code" in error && "stdout" in error);
      assert.equal(error.code, 1);
      assert.equal(typeof error.stdout, "string");
      assert.deepEqual(JSON.parse(error.stdout as string), {
        verdict: "mismatch",
        difference: faulty.difference,
      });
      return true;
    });
    assert.deepEqual(
      await replayRecording(
        checker,
        fromReplayItf(JSON.parse(await readFile(traceFile, "utf8"))),
        project,
      ),
      { verdict: "mismatch", difference: faulty.difference },
    );

    const server = await serveCheckout(t);
    const wrongAmount = { ...callback, amount: { units: decodeNat(1) } };
    await assert.rejects(
      recordReplay(
        checker,
        {
          ...capture,
          commands: [...capture.commands.slice(0, -1), wrongAmount],
        },
        server,
      ),
      /model rejected step 2/,
    );
    assert.equal(server.writes(), 1);
  },
);
