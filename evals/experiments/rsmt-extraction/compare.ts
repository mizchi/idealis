import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import {
  createNative,
  createZ3,
  nativePath,
  wasmPath,
} from "../smt-subset/clients.js";
import { controls, generated, handCases } from "../smt-subset/corpus.js";
import { maxWork } from "../smt-subset/paths.js";
import { digest } from "../smt-subset/provenance.js";
import { createWasm } from "../smt-subset/wasm.js";
import type { Reply, Request } from "../smt-subset/wire.js";
import { assertOracle } from "../workspace-oracle/node.js";

interface Record {
  readonly request: Request;
  readonly response: Reply;
}

const phase = process.argv[2];
const directory = process.argv[3];
const parent = process.argv[4];
assert.ok((phase === "baseline" || phase === "check") && directory);
await mkdir(directory);
const inputs = [
  ...handCases(),
  ...generated(0x30a1, 64),
  ...generated(0x30c3, 160),
];
const requests: readonly Request[] = [
  ...[...inputs.map((input) => input.source), ...controls].map(
    (source): Request => ({ kind: "solve", source, maxWork }),
  ),
  { kind: "solve", source: "(assert true)", maxWork: 0 },
  { kind: "prepare", source: "(declare-const x Int)(assert (= x 7))" },
  { kind: "check", handle: 1, maxWork },
  { kind: "release", handle: 1 },
  { kind: "check", handle: 1, maxWork },
];
const baseline: readonly Record[] =
  phase === "check"
    ? JSON.parse(await readFile(join(parent ?? "", "records.json"), "utf8"))
    : [];
if (phase === "check") {
  assert.ok(parent);
  assertOracle(
    baseline.map((record) => record.request),
    requests,
  );
}
const native = createNative();
const wasm =
  phase === "check" ? await createWasm(await readFile(wasmPath)) : undefined;
const z3 = phase === "check" ? await createZ3() : undefined;
const records: Record[] = [];
let witnesses = 0;
try {
  for (const [index, request] of requests.entries()) {
    const response = await native.request(request);
    records.push({ request, response });
    if (phase !== "check") continue;
    assertOracle(response, baseline[index]?.response, `native ${index}`);
    assertOracle(wasm?.request(request), response, `Wasm ${index}`);
    const input =
      request.kind === "solve"
        ? inputs.find((input) => input.source === request.source)
        : undefined;
    if (
      input &&
      response.kind === "solution" &&
      response.result.status === "sat"
    ) {
      await z3?.validate(input, response.result);
      witnesses++;
    }
  }
  await writeFile(
    join(directory, "records.json"),
    JSON.stringify(records, null, 2),
    { flag: "wx" },
  );
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      {
        status: "complete",
        phase,
        parent,
        requests: records.length,
        witnesses,
        knownSeedsReusedForRegressionOnly: true,
        binaries: await Promise.all(
          [nativePath, wasmPath].map(async (path) => ({
            path,
            sha256: digest(await readFile(path)),
          })),
        ),
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  console.log(JSON.stringify({ phase, requests: records.length, witnesses }));
} finally {
  await Promise.all([native.close(), z3?.close()]);
}
