import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { openBuild } from "../nesting-chains/frontends.js";
import {
  type Expectation,
  type Frontend,
  observe,
} from "../nesting-chains/outcome.js";
import { overriddenInputs } from "./expectations-0119.js";
import { sources0119 } from "./sources.js";

const [binary, wasm, distribution, output] = process.argv.slice(2);
assert.ok(
  binary && wasm && distribution && output,
  "binary wasm distribution output",
);
const build = await openBuild(binary, wasm, distribution);
const baseline: Record<string, Expectation> = {};
for (const input of [...sources0119(), ...overriddenInputs()]) {
  const cells = {} as Record<Frontend, { v1: unknown; v2: unknown }>;
  for (const frontend of ["ts", "rust"] as const) {
    const [v1, v2] = await Promise.all(
      ([1, 2] as const).map(
        async (irVersion) =>
          (
            await observe(build[frontend].compile, build[frontend].check, {
              source: input.source,
              config: input.config,
              irVersion,
            })
          ).outcome,
      ),
    );
    cells[frontend] = { v1, v2 };
  }
  baseline[input.name] = cells as unknown as Expectation;
}
await build.close();
await writeFile(output, `${JSON.stringify(baseline, null, 2)}\n`, {
  flag: "wx",
});
