import assert from "node:assert/strict";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { routes } from "../../evals/experiments/readme-comparison/protocol.js";
import { cases, fault } from "../../evals/experiments/single-model/cases.js";
import {
  prepare,
  runTool,
  verify,
} from "../../evals/experiments/single-model/runner.js";

const java = process.env.IDEALIS_TLC_JAVA;
const jar = process.env.IDEALIS_TLC_JAR;
assert.ok(java && jar);
const root = await mkdtemp(join(tmpdir(), "idealis-readme-"));
await mkdir(".artifacts/readme-comparison/controls", { recursive: true });
for (const input of cases("small"))
  test(input.name, async () => {
    for (const [expected, selected] of [
      ["complete", input],
      ["invariant-violation", fault(input)],
    ] as const) {
      for (const route of routes) {
        const directory = join(root, `${input.name}-${expected}-${route}`);
        await prepare(directory, selected);
        const actual = await runTool(
          route === "native-1" || route === "native-8" ? "native" : route,
          directory,
          selected,
          java,
          jar,
          route === "native-1" || route === "native-8"
            ? Number(route.slice(7))
            : undefined,
        );
        await writeFile(
          `.artifacts/readme-comparison/controls/${input.name}-${expected}-${route}.json`,
          JSON.stringify({ input: selected, expected, actual }),
        );
        verify(actual, selected, expected);
      }
    }
  });
