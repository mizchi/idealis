import assert from "node:assert/strict";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";
import { taskGate } from "./helpers/task-gate.js";

type Wire = "SuccessXml" | "ErrorXml" | "LostBody";
type Report = "Committed" | "Rejected" | "Unknown";
type Command =
  | { kind: "sendHeaders" | "readBody" }
  | { kind: "finish"; outcome: Wire; committed: boolean };
type Fault = "trust-headers" | "ignore-error-body" | "reject-disconnect";
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/object-store/multipart-response.model";
const success =
  "<CompleteMultipartUploadResult><ETag>example</ETag></CompleteMultipartUploadResult>";
const failure = "<Error><Code>InternalError</Code></Error>";

async function completion(
  t: TestContext,
  fault?: Fault,
): Promise<ObservationAdapter<Command>> {
  const release = taskGate();
  const sent = taskGate();
  let final: Extract<Command, { kind: "finish" }> | undefined;
  let object = "old object";
  let response: Response | undefined;
  let result: Report | null = null;
  const server = createServer(async (request, reply) => {
    if (request.method === "GET") {
      reply.end(object);
      return;
    }
    reply.writeHead(200, { "Content-Type": "application/xml" });
    reply.flushHeaders();
    reply.write(" \n");
    await release.promise;
    if (final === undefined) {
      reply.end();
      return;
    }
    if (final.committed) object = "assembled object";
    if (final.outcome === "LostBody") {
      reply.write("<CompleteMultipartUploadResult>", () => {
        reply.destroy();
        sent.open();
      });
    } else {
      reply.end(final.outcome === "SuccessXml" ? success : failure, sent.open);
    }
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  t.after(async () => {
    release.open();
    await new Promise<void>((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
      server.closeAllConnections();
    });
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  const origin = `http://127.0.0.1:${address.port}`;
  return {
    async apply(command) {
      switch (command.kind) {
        case "sendHeaders":
          response = await fetch(origin, { method: "POST" });
          assert.equal(response.status, 200);
          if (fault === "trust-headers") result = "Committed";
          break;
        case "finish":
          final = command;
          release.open();
          await sent.promise;
          break;
        case "readBody": {
          assert.ok(response);
          let xml: string;
          try {
            xml = (await response.text()).trim();
          } catch (error) {
            assert.ok(error instanceof TypeError);
            result = fault === "reject-disconnect" ? "Rejected" : "Unknown";
            return;
          }
          assert.ok(xml === success || xml === failure);
          result =
            xml === success || fault === "ignore-error-body"
              ? "Committed"
              : "Rejected";
          break;
        }
      }
    },
    async observe() {
      const read = await fetch(origin);
      assert.equal(read.status, 200);
      return {
        status: response?.status ?? null,
        result,
        object: await read.text(),
      };
    },
    project: ({ headers, result, published }) => ({
      status: headers ? 200 : null,
      result,
      object: published ? "assembled object" : "old object",
    }),
  };
}

test("a multipart HTTP 200 still requires a complete application result", async (t) => {
  for (const [question, expected, object] of [
    ["SuccessObserved", "Committed", "assembled object"],
    ["ErrorObserved", "Rejected", "old object"],
    ["CommittedButUnconfirmed", "Unknown", "assembled object"],
    ["NotCommittedAndUnconfirmed", "Unknown", "old object"],
  ] as const) {
    const scenario = await reachableScenario<Command>(checker, model, question);
    const matched = await recordReplay(checker, scenario, await completion(t));
    assert.equal(matched.verdict, "matched");
    assert.deepEqual(matched.recording.observations[1], {
      status: 200,
      result: null,
      object: "old object",
    });
    assert.deepEqual(matched.recording.observations.at(-1), {
      status: 200,
      result: expected,
      object,
    });
    const faults: readonly [Fault, number, Report][] =
      expected === "Rejected"
        ? [
            ["trust-headers", 1, "Committed"],
            ["ignore-error-body", 3, "Committed"],
          ]
        : expected === "Unknown"
          ? [["reject-disconnect", 3, "Rejected"]]
          : [];
    for (const [fault, step, result] of faults) {
      const faulty = await recordReplay(
        checker,
        scenario,
        await completion(t, fault),
      );
      assert.equal(faulty.verdict, "mismatch");
      assert.equal(faulty.difference.step, step);
      assert.deepEqual(faulty.difference.observed, {
        status: 200,
        result,
        object,
      });
    }
  }
});
