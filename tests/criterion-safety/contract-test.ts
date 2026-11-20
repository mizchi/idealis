import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { cases } from "../../evals/experiments/single-model/cases.js";
import {
  prepare,
  runTool,
  verify,
} from "../../evals/experiments/single-model/runner.js";

test("comparison uses the requested binary and never falls back", async () => {
  const root = await mkdtemp(join(tmpdir(), "idealis-binary-"));
  const input = cases("small")[0];
  assert.ok(input);
  const directory = join(root, "missing");
  await prepare(directory, input);
  for (const route of ["native", "wasm"] as const) {
    const result = await runTool(
      route,
      directory,
      input,
      "",
      "",
      route === "native" ? 1 : undefined,
      join(root, "absent"),
    );
    assert.notEqual(result.exit, 0);
    assert.throws(() => verify(result, input, "complete"));
  }
});
