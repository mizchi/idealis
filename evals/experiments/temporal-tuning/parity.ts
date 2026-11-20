import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type {
  PortableCommand,
  PortableCore,
  TlaSourceInput,
} from "../../../packages/idealis/src/contracts.js";
import { PortableCoreError } from "../../../packages/idealis/src/portable-value.js";
import { createPortableWasm } from "../../../packages/idealis/src/portable-wasm.js";
import { fingerprint } from "../rust-safety/provenance.js";
import { startNative } from "../usable/native.js";
import { digest } from "../usable/provenance.js";
import { assertOracle } from "../workspace-oracle/node.js";

interface Input extends TlaSourceInput {
  readonly name: string;
}
const baseline = process.argv[2];
const directory = process.argv[3];
const extra = process.env.IDEALIS_PARITY_EXTRA;
const plan = process.env.IDEALIS_PARITY_PLAN ?? "0049-temporal-tuning";
assert.ok(baseline && directory);
await mkdir(directory);
const base = await fingerprint();
const paths = [
  `evals/plans/${plan}.md`,
  "evals/experiments/temporal-tuning/parity.ts",
  "fixtures/temporal-tuning/boundaries.json",
  ...(extra ? [extra] : []),
];
const sources = [
  ...base.sources,
  ...(await Promise.all(
    paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  )),
];
const provenance = JSON.parse(
  await readFile(join(baseline, "manifest.json"), "utf8"),
);
for (const binary of provenance.binaries)
  assert.equal(digest(await readFile(binary.path)), binary.sha256);
const before = await createPortableWasm(
  await readFile(join(baseline, "core.wasm")),
);
const after = await createPortableWasm(
  await readFile(
    "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
  ),
);
const nativeBefore = startNative(join(baseline, "idealis-core"));
const nativeAfter = startNative();
const observe = (core: PortableCore, command: PortableCommand) => {
  try {
    return core.request(command);
  } catch (error) {
    assert.ok(error instanceof PortableCoreError);
    return { kind: "error", code: error.code, message: error.message };
  }
};
const inputs: Input[] = (
  await Promise.all(
    ["finite", "modules", "operators"].map(async (name) =>
      JSON.parse(
        await readFile(
          `crates/rtla-compiler/tests/${name}-support.json`,
          "utf8",
        ),
      ),
    ),
  )
).flat();
inputs.push(
  ...JSON.parse(
    await readFile("fixtures/temporal-tuning/boundaries.json", "utf8"),
  ),
);
if (extra) inputs.push(...JSON.parse(await readFile(extra, "utf8")));
const bounds = {
  maxDepth: 100,
  maxStates: 1000,
  maxWork: 1_000_000,
  maxTemporalNodes: 10000,
};
const limits = [
  bounds,
  ...[1, 17, 99, 511, 10000].map((maxWork) => ({ ...bounds, maxWork })),
  { ...bounds, maxDepth: 0 },
  { ...bounds, maxStates: 1 },
  { ...bounds, maxTemporalNodes: 1 },
];
const records = [];
try {
  for (const input of inputs) {
    const request = {
      source: input.source,
      config: input.config,
      ...(input.modules ? { modules: input.modules } : {}),
    };
    const requests: PortableCommand[] = [
      { version: 1, request: { kind: "compile-tla", ...request } },
      ...limits.map<PortableCommand>((bounds) => ({
        version: 1,
        request: { kind: "check-tla", ...request, bounds },
      })),
    ];
    const responses = [];
    for (const command of requests) {
      const expected = observe(before, command);
      assertOracle(observe(after, command), expected, input.name);
      assertOracle(
        await nativeBefore.request(command),
        expected,
        `${input.name}: before native/Wasm`,
      );
      assertOracle(
        await nativeAfter.request(command),
        expected,
        `${input.name}: after native/Wasm`,
      );
      responses.push({ command, response: expected });
    }
    records.push({ name: input.name, responses });
  }
  for (const source of sources)
    assert.equal(
      digest(await readFile(source.path)),
      source.sha256,
      source.path,
    );
  await writeFile(
    join(directory, "manifest.json"),
    JSON.stringify({ plan, sources, baseline: provenance, limits }, null, 2),
    { flag: "wx" },
  );
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        cases: inputs.length,
        commands: inputs.length * 10,
        records,
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  console.log(
    `${inputs.length} cases × 10 commands: exact before/after native/Wasm responses`,
  );
} finally {
  await nativeBefore.close();
  await nativeAfter.close();
}
