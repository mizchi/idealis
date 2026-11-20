import assert from "node:assert/strict";
import { test } from "node:test";
import { loadNativeBinding } from "../../evals/experiments/roracle-native/client.js";
import { fixtures } from "../../evals/experiments/roracle-native/fixtures.js";
import { decodeOracleReply } from "../../evals/experiments/workspace-oracle/protocol.js";
import { registerNativeOracleContract } from "./contract.js";

registerNativeOracleContract(test);

test("the decoder returns the validated report from JSON.parse", (t) => {
  const fixture = fixtures[0];
  assert.ok(fixture);
  const json = loadNativeBinding().dispatchJson(JSON.stringify(fixture.input));
  const parse = t.mock.method(JSON, "parse");
  const reply = decodeOracleReply(json);
  assert.strictEqual(reply.report, parse.mock.calls[0]?.result.report);
});
