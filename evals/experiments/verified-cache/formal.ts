import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import {
  advanceCachePolicy,
  type CachePolicyAction,
  type CachePolicyState,
} from "../../../packages/idealis/src/check-cache-policy.js";
import { valueKey } from "../../../packages/idealis/src/trace.js";

const directory = process.argv[2];
assert.ok(directory);
await mkdir(dirname(directory), { recursive: true });
await mkdir(directory);
const project = "formal/cache";
const digest = (value: string | Uint8Array) =>
  createHash("sha256").update(value).digest("hex");
const files: { path: string; sha256: string }[] = [];
const save = async (path: string, content: string) => {
  await writeFile(join(directory, path), content, { flag: "wx" });
  files.push({ path, sha256: digest(content) });
};
const command = async (
  name: string,
  executable: string,
  args: readonly string[],
  expectedFailure = false,
  input?: string,
) => {
  const output = spawnSync(executable, [...args], {
    cwd: project,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    ...(input === undefined ? {} : { input }),
  });
  await save(`${name}.stdout`, output.stdout ?? "");
  await save(`${name}.stderr`, output.stderr ?? "");
  if (output.error) throw output.error;
  if (expectedFailure) {
    assert.equal(output.signal, null);
    assert.ok(typeof output.status === "number" && output.status > 0);
    assert.match(
      output.stdout + output.stderr,
      /unsolved goals|Tactic.*failed|Application type mismatch/i,
    );
  } else assert.equal(output.status, 0, output.stdout + output.stderr);
  return output.stdout;
};

try {
  const paths = [
    "formal/cache/lean-toolchain",
    "formal/cache/lakefile.toml",
    "formal/cache/lake-manifest.json",
    "formal/cache/Cache.lean",
    "formal/cache/Main.lean",
    "formal/cache/ReuseMain.lean",
    "crates/rtlc/src/expansions.rs",
    "crates/rtlc/src/safety.rs",
    "crates/rtlc/src/safety_values.rs",
    "crates/rtlc/Cargo.lock",
    "packages/idealis/src/safety-expansions.ts",
    "packages/idealis/src/check-cache-policy.ts",
    "packages/idealis/src/check-cache.ts",
    "packages/idealis/src/check-cache-wire.ts",
    "packages/idealis/src/check-cache-node.ts",
    "packages/idealis/src/check-cache-http.ts",
    "packages/idealis/src/contracts.ts",
    "packages/idealis/src/backend.ts",
    "packages/idealis/src/cached-scheduling.ts",
    "evals/experiments/verified-cache/formal.ts",
  ];
  const sources = await Promise.all(
    paths.map(async (path) => ({ path, sha256: digest(await readFile(path)) })),
  );
  for (const entry of sources) {
    await mkdir(join(directory, "sources", dirname(entry.path)), {
      recursive: true,
    });
    await save(join("sources", entry.path), await readFile(entry.path, "utf8"));
  }
  const sourceVersion = digest(valueKey(sources));
  await save(
    "manifest.json",
    JSON.stringify(
      {
        plan: process.argv[3] ? "cached-expansions" : "verified-cache",
        sourceVersion,
        sources,
        toolchain: "leanprover/lean4:v4.35.0",
        assumptions: [
          "sound checker and complete dependency identity",
          "linearizable CAS and retained fencing tombstones",
          "TS safe integers; Lean mathematical naturals",
        ],
        scope:
          "policy proof and finite TS/Lean conformance; not a whole-TS proof",
      },
      null,
      2,
    ),
  );
  const source = await readFile(join(project, "Cache.lean"), "utf8");
  assert.doesNotMatch(source, /\b(sorry|admit|axiom|native_decide)\b/);
  const theorems = [...source.matchAll(/^theorem (\w+)/gm)].map(
    (match) => match[1],
  );
  assert.ok(theorems.length >= 17);
  await command("build", "lake", ["build"]);
  const audit = `import Cache\n${theorems.map((name) => `#print axioms Cache.${name}`).join("\n")}\n`;
  await save("audit.lean", audit);
  await writeFile(join(project, ".lake/VerifiedCacheAudit.lean"), audit);
  const auditOutput = await command("audit", "lake", [
    "env",
    "lean",
    ".lake/VerifiedCacheAudit.lean",
  ]);
  const axioms = new Set<string>();
  for (const match of auditOutput.matchAll(
    /depends on axioms:\s*\[([^\]]*)\]/g,
  ))
    for (const name of (match[1] ?? "")
      .split(",")
      .map((value) => value.trim())
      .filter(Boolean))
      axioms.add(name);
  for (const name of theorems) assert.ok(auditOutput.includes(`Cache.${name}`));
  for (const name of axioms)
    assert.ok(
      ["propext", "Classical.choice", "Quot.sound"].includes(name),
      name,
    );
  await command("kernel-recheck", "lake", ["env", "leanchecker", "Cache"]);
  const mutants = [
    { name: "fence", before: "action.fence == cell.fence", after: "true" },
    {
      name: "identity",
      before: "action.receiptIdentity != cell.identity",
      after: "false",
    },
    { name: "complete", before: "!action.complete", after: "false" },
    {
      name: "disabled",
      before: "probe.enabled &&",
      after: "true &&",
    },
    {
      name: "next",
      before: "probe.sameNext && probe.complete",
      after: "true && probe.complete",
    },
    {
      name: "partial",
      before: "probe.sameNext && probe.complete",
      after: "probe.sameNext && true",
    },
    { name: "memo", before: "if enabled source then", after: "if true then" },
  ];
  for (const mutant of mutants) {
    assert.equal(source.split(mutant.before).length, 2);
    const broken = source.replace(mutant.before, mutant.after);
    await save(`mutant-${mutant.name}.lean`, broken);
    const path = `.lake/VerifiedCacheMutant-${mutant.name}.lean`;
    await writeFile(join(project, path), broken);
    const diagnostic = await command(
      `mutant-${mutant.name}`,
      "lake",
      ["env", "lean", path],
      true,
    );
    const expected: Record<string, string> = {
      fence: "stale_fence_publish",
      identity: "step_valid",
      complete: "incomplete_not_published",
      disabled: "disabled_expansion_not_reused",
      next: "changed_next_not_reused",
      partial: "partial_expansion_not_reused",
      memo: "memo_next_exact",
    };
    assert.ok(
      [
        ...diagnostic.matchAll(
          /:(\d+):\d+: error: (?:unsolved goals|Tactic.*failed|Application type mismatch)/g,
        ),
      ].some((match) => {
        const before = broken.split("\n").slice(0, Number(match[1])).join("\n");
        return (
          [...before.matchAll(/^theorem (\w+)/gm)].at(-1)?.[1] ===
          expected[mutant.name]
        );
      }),
      mutant.name,
    );
  }
  const actions: CachePolicyAction[] = [];
  const add = (
    op: string,
    identity: string,
    owner = "",
    fence = 0,
    now = 0,
    duration = 1,
    receiptIdentity = "",
    complete = false,
  ) =>
    actions.push({
      op,
      identity,
      owner,
      fence,
      now,
      duration,
      receiptIdentity,
      complete,
    });
  for (const identity of ["a", "b"]) {
    add("evict", identity);
    for (const now of [0, 1, 2, 3]) {
      add("probe", identity, "", 0, now);
      for (const owner of ["w1", "w2"]) {
        for (const duration of [0, 1, 2])
          add("acquire", identity, owner, 0, now, duration);
        for (const fence of [0, 1, 2, 3, 4]) {
          for (const duration of [0, 1, 2])
            add("renew", identity, owner, fence, now, duration);
          for (const receipt of ["a", "b"])
            for (const complete of [false, true])
              add("publish", identity, owner, fence, now, 1, receipt, complete);
          if (now === 0) add("release", identity, owner, fence);
        }
      }
    }
  }
  const initial: CachePolicyState = {
    identity: "a",
    fence: 0,
    phase: "idle",
    owner: "",
    expiresAt: 0,
    storedIdentity: "",
    complete: false,
  };
  const states = [initial];
  const seen = new Set([valueKey(initial)]);
  const probes: { state: CachePolicyState; action: CachePolicyAction }[] = [];
  const actual = [];
  const codes = new Set<string>();
  for (let index = 0; index < states.length; index++) {
    const state = states[index];
    assert.ok(state);
    for (const action of actions) {
      const reply = advanceCachePolicy(state, action);
      probes.push({ state, action });
      actual.push(reply);
      codes.add(reply.code);
      if (reply.state.fence > 3) continue;
      const key = valueKey(reply.state);
      if (!seen.has(key)) {
        seen.add(key);
        states.push(reply.state);
      }
    }
  }
  for (const code of [
    "hit",
    "miss",
    "busy",
    "lease",
    "renewed",
    "published",
    "uncacheable",
    "stale",
    "mismatch",
    "released",
    "evicted",
    "invalid",
  ])
    assert.ok(codes.has(code), code);
  await save("probes.json", JSON.stringify(probes));
  const expected = await command(
    "oracle",
    resolve(project, ".lake/build/bin/cache-oracle"),
    [],
    false,
    JSON.stringify(probes),
  );
  assert.deepEqual(JSON.parse(expected), actual);
  const reuseFile = process.argv[3];
  const reuse = reuseFile
    ? JSON.parse(await readFile(reuseFile, "utf8"))
    : [...Array(32).keys()].map((bits) => ({
        probe: {
          enabled: !!(bits & 16),
          sameBuild: !!(bits & 1),
          sameNext: !!(bits & 2),
          sameState: !!(bits & 4),
          complete: !!(bits & 8),
        },
        reused: bits === 31,
      }));
  assert.equal(reuse.length, 32);
  for (const entry of reuse) {
    assert.deepEqual(Object.keys(entry.probe).sort(), [
      "complete",
      "enabled",
      "sameBuild",
      "sameNext",
      "sameState",
    ]);
    for (const value of Object.values(entry.probe))
      assert.equal(typeof value, "boolean");
    assert.equal(typeof entry.reused, "boolean");
  }
  assert.equal(
    new Set(reuse.map((entry: { probe: unknown }) => valueKey(entry.probe)))
      .size,
    32,
  );
  await save("reuse-probes.json", JSON.stringify(reuse));
  const reuseExpected = JSON.parse(
    await command(
      "reuse-oracle",
      resolve(project, ".lake/build/bin/reuse-oracle"),
      [],
      false,
      JSON.stringify(reuse.map((entry: { probe: unknown }) => entry.probe)),
    ),
  );
  assert.deepEqual(
    reuseExpected,
    reuse.map((entry: { reused: boolean }) => entry.reused),
  );
  for (const entry of sources)
    assert.equal(digest(await readFile(entry.path)), entry.sha256, entry.path);
  await save(
    "result.json",
    JSON.stringify(
      {
        status: "complete",
        sourceVersion,
        theorems,
        axioms: [...axioms].sort(),
        negativeProofControls: mutants.map((entry) => entry.name),
        expansionReuseCases: reuse.length,
        expansionOracle: reuseFile
          ? "real Rust/Wasm observations"
          : "boolean policy table",
        states: states.length,
        actions: actions.length,
        transitions: probes.length,
        fencesThrough: 3,
        arithmeticDomain: "Lean Nat; finite conformance values 0..4",
        files,
      },
      null,
      2,
    ),
  );
  console.log(
    JSON.stringify({
      theorems: theorems.length,
      states: states.length,
      transitions: probes.length,
      negativeProofControls: mutants.length,
    }),
  );
} catch (error) {
  await save("error.json", JSON.stringify({ message: String(error) }, null, 2));
  throw error;
}
