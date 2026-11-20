import assert from "node:assert/strict";
import { type SpawnSyncReturns, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import {
  advanceCachePolicy,
  type CachePolicyAction,
  type CachePolicyState,
} from "../../../packages/idealis/src/check-cache-policy.js";

const [java, jar, output] = process.argv.slice(2);
if (!java || !jar || !output)
  throw new Error("Usage: cache-tlc java jar output");
const directory = resolve(output);
await mkdir(directory);
const source = await readFile("formal/cache/CacheProtocol.tla", "utf8");
const digest = (body: string | Uint8Array) =>
  createHash("sha256").update(body).digest("hex");
const cases = [
  { name: "correct", broken: 0, invariant: "Safe", violated: false },
  { name: "reachable-hit", broken: 0, invariant: "NoReady", violated: true },
  { name: "stale-fence", broken: 1, invariant: "Safe", violated: true },
  { name: "wrong-receipt", broken: 2, invariant: "Safe", violated: true },
  { name: "incomplete-result", broken: 3, invariant: "Safe", violated: true },
];
const results = [];
for (const scenario of cases) {
  const cwd = resolve(directory, scenario.name);
  await mkdir(cwd);
  await writeFile(resolve(cwd, "CacheProtocol.tla"), source);
  await writeFile(
    resolve(cwd, "CacheProtocol.cfg"),
    `CONSTANT Broken = ${scenario.broken}\nINIT Init\nNEXT Next\nINVARIANT ${scenario.invariant}\nCHECK_DEADLOCK FALSE\n`,
  );
  const args = [
    "-cp",
    resolve(jar),
    "tlc2.TLC",
    "-workers",
    "1",
    "-seed",
    "1",
    "-dumpTrace",
    "json",
    "trace.json",
    "CacheProtocol",
  ];
  const run: SpawnSyncReturns<string> = spawnSync(resolve(java), args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  if (run.error) throw run.error;
  await writeFile(
    resolve(cwd, "command.json"),
    JSON.stringify({ java: resolve(java), args }),
  );
  await writeFile(resolve(cwd, "tlc.log"), run.stdout + run.stderr);
  assert.equal(
    run.status,
    scenario.violated ? 12 : 0,
    `${scenario.name}: ${run.stdout}${run.stderr}`,
  );
  let replay = null;
  if (scenario.violated) {
    const trace = JSON.parse(
      await readFile(resolve(cwd, "trace.json"), "utf8"),
    );
    const states: [
      number,
      { cell: CachePolicyState; event: CachePolicyAction },
    ][] = trace.counterexample.state;
    states.sort((left, right) => left[0] - right[0]);
    const initial = states[0]?.[1].cell;
    assert.ok(initial);
    let state = initial;
    const replies = [];
    for (const [, frame] of states.slice(1)) {
      if (frame.event.op === "tick") continue;
      const reply = advanceCachePolicy(state, frame.event);
      replies.push(reply);
      if (scenario.broken === 0) assert.deepEqual(reply.state, frame.cell);
      state = reply.state;
    }
    const last = replies.at(-1);
    assert.ok(last);
    assert.equal(
      last.code,
      scenario.broken === 0
        ? "published"
        : scenario.broken === 1
          ? "stale"
          : scenario.broken === 2
            ? "mismatch"
            : "uncacheable",
    );
    replay = { steps: replies.length, lastCode: last.code };
    await writeFile(
      resolve(cwd, "replay.json"),
      JSON.stringify(replies, null, 2),
    );
  }
  const count = run.stdout.match(
    /([\d,]+) states generated, ([\d,]+) distinct states found/,
  );
  results.push({
    ...scenario,
    status: run.status,
    generated: count?.[1],
    distinct: count?.[2],
    replay,
  });
}
const result = {
  sourceDigest: digest(source),
  jarDigest: digest(await readFile(jar)),
  policyDigest: digest(
    await readFile("packages/idealis/src/check-cache-policy.ts"),
  ),
  cases: results,
};
await writeFile(
  resolve(directory, "result.json"),
  JSON.stringify(result, null, 2),
);
console.log(JSON.stringify(result, null, 2));
