import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { cases } from "../evals/experiments/collections-temporal/cases.js";
import { config, names } from "../evals/experiments/quint-generated/cases.js";
import { startNative } from "../evals/experiments/usable/native.js";
import type {
  SafetyBounds,
  SafetyExpansionCache,
} from "../packages/idealis/src/contracts.js";
import { createPortableWasm } from "../packages/idealis/src/portable-wasm.js";
import {
  checkSafetyIncremental,
  createSafetyExpansionCache,
  decodeSafetyExpansionCache,
  mergeSafetyExpansions,
} from "../packages/idealis/src/safety-expansions.js";
import { checkSafety } from "../packages/idealis/src/safety.js";
import { compileTlaSource } from "../packages/idealis/src/tla-source.js";
import { isRecord } from "../packages/idealis/src/trace.js";

const bytes = await readFile(
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm",
);
const buildDigest = createHash("sha256").update(bytes).digest("hex");
const core = await createPortableWasm(bytes);
const bounds: SafetyBounds = {
  maxDepth: 64,
  maxStates: 10000,
  maxWork: 1000000,
};
const temporalBounds = { ...bounds, maxTemporalNodes: 50000 };
const source =
  "---- MODULE Expansion ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x<3 THEN x+1 ELSE 0\nSafe == x<4\n====";
const compile = (text = source) =>
  compileTlaSource(core, {
    source: text,
    config: "INIT Init\nNEXT Next\nINVARIANT Safe",
  });
const check = (
  program = compile(),
  cache: SafetyExpansionCache = createSafetyExpansionCache(),
  limits: SafetyBounds = bounds,
  maxEntries = 100,
) =>
  checkSafetyIncremental(core, program, limits, {
    enabled: true,
    cache,
    buildDigest,
    maxEntries,
  });
const sameResult = (
  left: ReturnType<typeof checkSafety>,
  right: ReturnType<typeof checkSafety>,
) => assert.deepEqual({ ...left, work: 0 }, { ...right, work: 0 });

export const registerExpansionContract = (
  test: (name: string, run: () => void | Promise<void>) => unknown,
): void => {
  test("options cannot replace the positional unsafe program or bounds", () => {
    const unsafe = compile(source.replace("Safe == x<4", "Safe == x<2"));
    const options = {
      enabled: true,
      cache: createSafetyExpansionCache(),
      buildDigest,
      maxEntries: 100,
      program: compile(),
      bounds: { ...bounds, maxDepth: 0 },
    };
    const result = checkSafetyIncremental(core, unsafe, bounds, options);
    assert.equal(result.result.status, "invariant-violation");
    sameResult(result.result, checkSafety(core, unsafe, bounds));
  });
  test("disabled checks leave opaque cache objects untouched", () => {
    const cache = createSafetyExpansionCache();
    Reflect.set(cache, "entries", cache);
    for (const options of [{}, { enabled: false }]) {
      const result = checkSafetyIncremental(core, compile(), bounds, {
        ...options,
        cache,
        buildDigest: "",
        maxEntries: 0,
      });
      assert.deepEqual(result.result, checkSafety(core, compile(), bounds));
      assert.equal(result.cache, cache);
      const raw = core.request({
        version: 1,
        request: {
          kind: "check-safety-incremental",
          program: compile(),
          bounds,
          ...options,
          cache,
          buildDigest: "",
          maxEntries: 0,
        },
      });
      assert.equal(raw.kind, "incremental-safety");
      if (raw.kind !== "incremental-safety")
        throw new Error("Expected incremental result");
      assert.equal(raw.cache, cache);
      assert.deepEqual(raw.result, result.result);
      assert.deepEqual(result.stats, {
        hits: 0,
        misses: 0,
        writes: 0,
        invalidated: false,
      });
    }
  });
  test("snapshot decoding rejects sparse, noncanonical and misleading fields", () => {
    const snapshot = check().cache;
    const key = Object.keys(snapshot.entries)[0];
    assert.ok(key);
    for (const damage of [
      (cache: SafetyExpansionCache) =>
        Reflect.set(cache.entries[key]?.successors ?? {}, "length", 3),
      (cache: SafetyExpansionCache) =>
        Reflect.set(
          cache.entries[key]?.successors[0]?.values ?? {},
          "length",
          3,
        ),
      (cache: SafetyExpansionCache) =>
        Reflect.set(
          cache.entries[key]?.successors[0]?.values[0] ?? {},
          "value",
          "01",
        ),
      (cache: SafetyExpansionCache) =>
        Reflect.set(cache.entries[key]?.successors[0]?.values ?? {}, "0", {
          type: "set",
          value: [
            { type: "int", value: "2" },
            { type: "int", value: "1" },
          ],
        }),
      (cache: SafetyExpansionCache) => {
        Reflect.deleteProperty(cache, "format");
        Reflect.deleteProperty(cache, "entries");
        Reflect.set(cache, "entries,format", {});
      },
    ]) {
      const broken = structuredClone(snapshot);
      damage(broken);
      assert.throws(() => decodeSafetyExpansionCache(broken));
    }
    assert.deepEqual(decodeSafetyExpansionCache(snapshot), snapshot);
  });
  test("evaluation creates missing parents and refuses to overwrite a saved run", async () => {
    const directory = await mkdtemp(join(tmpdir(), "idealis-expansion-eval-"));
    const output = join(directory, "fresh", "parent", "run");
    try {
      const first = spawnSync(
        process.execPath,
        ["dist/evals/experiments/cached-expansions/run.js", output],
        { encoding: "utf8" },
      );
      assert.equal(first.status, 0, first.stdout + first.stderr);
      const manifest = await readFile(join(output, "manifest.json"), "utf8");
      const result = JSON.parse(
        await readFile(join(output, "result.json"), "utf8"),
      );
      assert.equal(result.status, "complete");
      assert.equal(result.guardCases, 32);
      const duplicate = spawnSync(
        process.execPath,
        ["dist/evals/experiments/cached-expansions/run.js", output],
        { encoding: "utf8" },
      );
      assert.notEqual(duplicate.status, 0);
      assert.match(duplicate.stderr, /EEXIST/);
      assert.equal(
        await readFile(join(output, "manifest.json"), "utf8"),
        manifest,
      );
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
  test("expansion caching defaults off and ignores stored entries without an enable flag", () => {
    const program = compile();
    const seeded = check(program);
    const before = JSON.stringify(seeded.cache);
    const disabled = checkSafetyIncremental(core, program, bounds, {
      cache: seeded.cache,
      buildDigest,
      maxEntries: 100,
    });
    assert.deepEqual(disabled.result, checkSafety(core, program, bounds));
    assert.deepEqual(disabled.stats, {
      hits: 0,
      misses: 0,
      writes: 0,
      invalidated: false,
    });
    assert.equal(JSON.stringify(disabled.cache), before);
    const corrupt = JSON.parse(before);
    const key = Object.keys(corrupt.entries)[0];
    assert.ok(key);
    corrupt.entries[key].digest = "0".repeat(64);
    const bypassed = checkSafetyIncremental(core, program, bounds, {
      cache: corrupt,
      buildDigest,
      maxEntries: 100,
    });
    assert.deepEqual(bypassed.result, checkSafety(core, program, bounds));
    assert.equal(JSON.stringify(bypassed.cache), JSON.stringify(corrupt));
  });
  test("reused edges rebuild temporal and fairness graphs for the current property", () => {
    for (const name of [
      "eventually-stuttering",
      "eventually-WF",
      "always-eventually-WF",
      "eventually-always-WF",
      "WF-intermittent",
      "SF-intermittent",
      "multiple-initial-temporal",
    ]) {
      const input = cases.find((input) => input.name === name);
      assert.ok(input, name);
      const program = compileTlaSource(core, {
        source: input.source,
        config: input.config,
        irVersion: 2,
      });
      const cold = check(program, createSafetyExpansionCache(), temporalBounds);
      const warm = check(program, cold.cache, temporalBounds);
      assert.equal(warm.result.status, input.expected, input.name);
      sameResult(warm.result, checkSafety(core, program, temporalBounds));
      assert.ok(warm.stats.hits > 0);
      const withoutTemporal = check(
        { ...program, properties: [], fairness: [] },
        createSafetyExpansionCache(),
        temporalBounds,
      );
      assert.equal(withoutTemporal.result.status, "complete");
      const changedProperty = check(
        program,
        withoutTemporal.cache,
        temporalBounds,
      );
      assert.equal(changedProperty.result.status, input.expected);
      assert.ok(changedProperty.stats.hits > 0);
      sameResult(
        changedProperty.result,
        checkSafety(core, program, temporalBounds),
      );
    }
  });
  test("Quint expansions retain TLC outcomes and counts in both IR versions", async () => {
    const oracle = JSON.parse(
      await readFile("fixtures/quint-generated/tlc-oracle.json", "utf8"),
    );
    for (const irVersion of [1, 2] as const)
      for (const name of names) {
        const source = await readFile(
          `fixtures/quint-generated/generated/${name}.tla`,
          "utf8",
        );
        const normal = compileTlaSource(core, {
          source,
          config: config("safety"),
          irVersion,
        });
        const cold = check(normal);
        const warm = checkSafetyIncremental(
          await createPortableWasm(bytes),
          normal,
          bounds,
          {
            enabled: true,
            cache: JSON.parse(JSON.stringify(cold.cache)),
            buildDigest,
            maxEntries: 100,
          },
        );
        sameResult(cold.result, checkSafety(core, normal, bounds));
        sameResult(warm.result, cold.result);
        assert.equal(warm.stats.hits, warm.result.states);
        assert.equal(warm.stats.misses, 0);
        const reference = oracle.find(
          (row: { name: string; property: string }) =>
            row.name === name && row.property === "safety",
        );
        assert.equal(warm.result.status, reference.status);
        assert.equal(warm.result.states, reference.states);
        assert.equal(warm.result.generated, reference.generated);
        const broken = compileTlaSource(core, {
          source,
          config: config("broken"),
          irVersion,
        });
        sameResult(
          check(broken, warm.cache).result,
          checkSafety(core, broken, bounds),
        );
        assert.equal(
          check(broken, warm.cache).result.status,
          "invariant-violation",
        );
      }
  });

  test("cached targets recheck invariants, initial roots and action properties", () => {
    const cold = check();
    const changed = compile(source.replace("Safe == x<4", "Safe == x<2"));
    const violation = check(changed, cold.cache);
    assert.equal(violation.result.status, "invariant-violation");
    assert.equal(violation.stats.hits, 2);
    sameResult(violation.result, checkSafety(core, changed, bounds));
    const roots = compile(source.replace("Init == x=0", "Init == x=2"));
    assert.equal(check(roots, cold.cache).stats.hits, 4);
    sameResult(
      check(roots, cold.cache).result,
      checkSafety(core, roots, bounds),
    );
    const action = {
      ...compile(),
      actionProperties: [
        {
          name: "Never",
          expression: {
            op: "ne" as const,
            left: { op: "read" as const, slot: 0, next: true },
            right: {
              op: "literal" as const,
              value: { type: "int" as const, value: "0" },
            },
          },
        },
      ],
    };
    const result = check(action, cold.cache);
    assert.equal(result.result.status, "property-violation");
    assert.equal(result.stats.hits, 4);
    sameResult(result.result, checkSafety(core, action, bounds));
  });

  test("changed constraints recheck boundary candidates and explore newly included states", () => {
    const limited = {
      ...compile(),
      constraints: [
        {
          name: "Cut",
          expression: {
            op: "lt" as const,
            left: { op: "read" as const, slot: 0, next: false },
            right: {
              op: "literal" as const,
              value: { type: "int" as const, value: "2" },
            },
          },
        },
      ],
    };
    const cold = check(limited);
    assert.equal(cold.result.states, 2);
    const expanded = check(compile(), cold.cache);
    assert.equal(expanded.stats.hits, 2);
    assert.equal(expanded.stats.misses, 2);
    assert.equal(expanded.result.states, 4);
    sameResult(expanded.result, checkSafety(core, compile(), bounds));
    const guarded = {
      ...limited,
      invariants: limited.constraints.map((constraint) => ({
        ...constraint,
        name: "Boundary",
      })),
    };
    assert.equal(
      check(guarded, expanded.cache).result.status,
      "invariant-violation",
    );
  });

  test("unfinished expansions, empty models and cache capacity preserve bounded outcomes", () => {
    for (const limits of [
      { ...bounds, maxStates: 1 },
      { ...bounds, maxWork: 1 },
    ]) {
      const limited = check(compile(), createSafetyExpansionCache(), limits);
      assert.equal(limited.result.status, "bounded");
      assert.equal(limited.stats.writes, 0);
      const full = check(compile(), limited.cache);
      assert.equal(full.result.status, "complete");
      assert.equal(full.stats.hits, 0);
    }
    const shallow = check(compile(), createSafetyExpansionCache(), {
      ...bounds,
      maxDepth: 0,
    });
    assert.equal(shallow.result.status, "bounded");
    assert.equal(check(compile(), shallow.cache).result.status, "complete");
    for (const capacity of [0, 2]) {
      const cold = check(
        compile(),
        createSafetyExpansionCache(),
        bounds,
        capacity,
      );
      assert.equal(Object.keys(cold.cache.entries).length, capacity);
      const warm = check(compile(), cold.cache, bounds, capacity);
      assert.equal(warm.result.status, "complete");
      assert.equal(warm.stats.hits, capacity);
    }
    const empty = compile(source.replace("Init == x=0", "Init == x \\in {}"));
    assert.equal(check(empty, check().cache).result.status, "empty");
  });

  test("empty cached successor lists still trigger current deadlock checking", () => {
    const terminal = compile(
      source.replace(
        "Next == x'=IF x<3 THEN x+1 ELSE 0",
        "Next == FALSE /\\ UNCHANGED x",
      ),
    );
    const cold = check({ ...terminal, checkDeadlock: false });
    assert.equal(cold.result.status, "complete");
    assert.equal(cold.stats.writes, 1);
    const warm = check(terminal, cold.cache);
    assert.equal(warm.stats.hits, 1);
    assert.equal(warm.result.status, "deadlock");
    sameResult(warm.result, checkSafety(core, terminal, bounds));
  });

  test("build and Next edits invalidate expansions and persisted corruption is rejected", () => {
    const cold = check();
    const changed = compile(source.replace("THEN x+1", "THEN x+2"));
    const next = check(changed, cold.cache);
    assert.equal(next.stats.invalidated, true);
    assert.equal(next.stats.hits, 0);
    sameResult(next.result, checkSafety(core, changed, bounds));
    const build = checkSafetyIncremental(core, compile(), bounds, {
      enabled: true,
      cache: cold.cache,
      buildDigest: "b".repeat(64),
      maxEntries: 100,
    });
    assert.equal(build.stats.invalidated, true);
    assert.equal(build.stats.hits, 0);
    const renamed = check({ ...compile(), variables: ["renamed"] }, cold.cache);
    assert.equal(renamed.stats.invalidated, true);
    assert.equal(renamed.stats.hits, 0);
    const helperSource =
      "---- MODULE HelperCache ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nInc(n) == n+1\nNext == x'=IF x=0 THEN Inc(0) ELSE 0\nSafe == x<2\n====";
    const first = compileTlaSource(core, {
      source: helperSource,
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
      irVersion: 2,
    });
    const second = compileTlaSource(core, {
      source: helperSource.replace("Inc(n) == n+1", "Inc(n) == n+2"),
      config: "INIT Init\nNEXT Next\nINVARIANT Safe",
      irVersion: 2,
    });
    assert.deepEqual(first.next, second.next);
    const helperChanged = check(second, check(first).cache);
    assert.equal(helperChanged.stats.invalidated, true);
    assert.equal(helperChanged.stats.hits, 0);
    assert.equal(helperChanged.result.status, "invariant-violation");
    sameResult(helperChanged.result, checkSafety(core, second, bounds));
    const corrupt = JSON.parse(JSON.stringify(cold.cache));
    const key = Object.keys(corrupt.entries)[0];
    assert.ok(key);
    corrupt.entries[key].successors = [];
    assert.throws(() => check(compile(), corrupt), /digest mismatch/);
  });

  test("independent root shards merge complete expansions and reject conflicts", () => {
    const one = source.replace(
      "Next == x'=IF x<3 THEN x+1 ELSE 0",
      "Next == UNCHANGED x",
    );
    const left = check(compile(one));
    const right = check(compile(one.replace("Init == x=0", "Init == x=1")));
    const merged = mergeSafetyExpansions(core, left.cache, right.cache, 100);
    const both = check(
      compile(one.replace("Init == x=0", "Init == x \\in {0,1}")),
      merged,
    );
    assert.equal(both.stats.hits, 2);
    assert.equal(both.stats.misses, 0);
    assert.throws(
      () => mergeSafetyExpansions(core, left.cache, check().cache, 100),
      /namespaces/,
    );
    const conflict = JSON.parse(JSON.stringify(left.cache));
    const key = Object.keys(conflict.entries)[0];
    assert.ok(key);
    conflict.entries[key].successors[0].values[0].value = "99";
    conflict.entries[key].digest = createHash("sha256")
      .update(
        JSON.stringify([
          conflict.buildDigest,
          conflict.transitionIdentity,
          key,
          conflict.entries[key].successors,
        ]),
      )
      .digest("hex");
    assert.throws(
      () => mergeSafetyExpansions(core, left.cache, conflict, 100),
      /Conflicting/,
    );
  });

  test("native process restart reuses portable compound states without arena IDs", async () => {
    const program = compile(
      source
        .replace("Init == x=0", "Init == x=[n |-> 0]")
        .replace(
          "Next == x'=IF x<3 THEN x+1 ELSE 0",
          "Next == x'=[n |-> IF x.n<3 THEN x.n+1 ELSE 0]",
        )
        .replace("Safe == x<4", "Safe == x.n<4"),
    );
    const nativeDigest = createHash("sha256")
      .update(await readFile("crates/idealis-core/target/release/idealis-core"))
      .digest("hex");
    const first = startNative();
    const cold = await first
      .request({
        version: 1,
        request: {
          kind: "check-safety-incremental",
          enabled: true,
          program,
          bounds,
          cache: createSafetyExpansionCache(),
          buildDigest: nativeDigest,
          maxEntries: 100,
        },
      })
      .finally(() => first.close());
    assert.ok(isRecord(cold) && cold.kind === "incremental-safety");
    const snapshot = decodeSafetyExpansionCache(cold.cache);
    const second = startNative();
    const warm = await second
      .request({
        version: 1,
        request: {
          kind: "check-safety-incremental",
          enabled: true,
          program,
          bounds,
          cache: snapshot,
          buildDigest: nativeDigest,
          maxEntries: 100,
        },
      })
      .finally(() => second.close());
    assert.ok(isRecord(warm) && isRecord(warm.stats) && isRecord(warm.result));
    assert.equal(warm.stats.hits, 4);
    assert.equal(warm.result.status, "complete");
    const wasm = check(program);
    assert.equal(wasm.result.states, warm.result.states);
  });
};
