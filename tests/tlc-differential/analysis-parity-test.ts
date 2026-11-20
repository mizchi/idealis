import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createInterface } from "node:readline";
import { test } from "node:test";
import { Worker } from "node:worker_threads";
import { expandTemporalTemplates } from "../../evals/experiments/tlc-differential/expand-temporal.js";
import { analysisRoles } from "../../packages/idealis/src/tla-safety-analysis.js";
import { isRecord } from "../../packages/idealis/src/trace.js";
import { verifyAnalysisInventory } from "./analysis-inventory.js";
import {
  analysisBounds,
  analysisLimits,
  createAnalysisCorpus,
  sourceObjects,
  sourceReach,
} from "./analysis-parity-corpus.js";
import { assertIrReach, assertSourceReach } from "./analysis-reach.js";
import "./analysis-supplement-test.js";

const directory = "fixtures/tlc-operator-sharing";
const roles: typeof analysisRoles = JSON.parse(
  await readFile(`${directory}/analysis-roles.json`, "utf8"),
);
const inventory = JSON.parse(
  await readFile(`${directory}/analysis-inventory.json`, "utf8"),
);
const generatedCorpus = createAnalysisCorpus(inventory.sharedApplyArityEntries);
const analysisPositions = () => generatedCorpus.positions;
const generateAnalysisCases = () => generatedCorpus.cases;
const normalize = (input: unknown): unknown => {
  const value = expandTemporalTemplates(input);
  if (Array.isArray(value)) return value.map(normalize);
  if (!isRecord(value)) return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(
        ([key, entry]) =>
          !(key === "subscript" && entry === null) &&
          !(
            [
              "fairness",
              "initialProperties",
              "actionProperties",
              "properties",
            ].includes(key) &&
            Array.isArray(entry) &&
            entry.length === 0
          ),
      )
      .map(([key, entry]) => [key, normalize(entry)]),
  );
};
test("analysis parity production TS role table equals independent roles", () => {
  assert.deepEqual(
    analysisRoles,
    roles,
    "production TS descriptor table must enumerate the independent roles",
  );
});
test("analysis parity production Rust role table equals independent roles", async () => {
  const result = spawnSync(
    "crates/rtla-compiler/target/debug/examples/analysis_roles",
    [],
    { encoding: "utf8", timeout: 1000 },
  );
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  const rust = JSON.parse(result.stdout);
  const additions: typeof analysisRoles = JSON.parse(
    await readFile(`${directory}/analysis-rust-roles.json`, "utf8"),
  );
  const key = (row: (typeof analysisRoles)[number]) =>
    [row.space, row.constructor, row.operatorClass, row.position].join("\0");
  const expected = [...roles, ...additions].sort((left, right) =>
    key(left) < key(right) ? -1 : key(left) > key(right) ? 1 : 0,
  );
  assert.deepEqual(
    rust,
    expected,
    "Rust extensions and shared descriptors have independent expectations",
  );
  assert.deepEqual(
    rust.filter(
      (row: (typeof analysisRoles)[number]) =>
        !additions.some((addition) => key(addition) === key(row)),
    ),
    analysisRoles,
  );
});
test("analysis parity independent source inventory and two-way position difference", async () => {
  await verifyAnalysisInventory(analysisPositions());
  const sourceRows = roles.filter(
    (row) => row.space === "source" && row.position !== "",
  );
  assert.equal(sourceRows.length, 198);
  const key = (nodeKind: string, position: string) =>
    `${nodeKind}/${position.replaceAll(".Index", "")}`;
  const expected = new Set(
    sourceRows.map((row) => key(row.constructor, row.position)),
  );
  const actual = new Set(
    analysisPositions().map((row) => key(row.constructor, row.position)),
  );
  assert.deepEqual(
    [...expected].filter((id) => !actual.has(id)),
    [],
    "role positions missing from corpus",
  );
  assert.deepEqual(
    [...actual].filter((id) => !expected.has(id)),
    [],
    "corpus positions absent from roles",
  );
});

const workerSource = `
import { parentPort } from 'node:worker_threads';
import { readFile } from 'node:fs/promises';
const { compileTlaSafety } = await import(new URL('./dist/packages/idealis/src/tla-safety.js', 'file://' + process.cwd() + '/'));
const { createPortableWasm } = await import(new URL('./dist/packages/idealis/src/portable-wasm.js', 'file://' + process.cwd() + '/'));
const core = await createPortableWasm(await readFile('crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm'));
const capture = fn => { try { return fn(); } catch (e) { return {kind:'error',code:e.code,message:e.message}; } };
parentPort.on('message', ({id,kind,input,programs,bounds}) => {
  const result = kind === 'compile'
    ? [capture(() => ({kind:'compiled', program:compileTlaSafety(input)})),capture(() => core.request({version:1,request:{kind:'compile-tla',...input}}))]
    : programs.map(program => capture(() => core.request({version:1,request:{kind:'check-safety',program,bounds}})));
  parentPort.postMessage({id,result});
});
parentPort.postMessage({ready:true});
`;
test(
  "analysis parity generated four-probe corpus and reached ledger",
  {
    timeout: 181_000,
  },
  async (t) => {
    const start = 0;
    const expectedCases = generateAnalysisCases();
    assert.equal(analysisPositions().length, analysisLimits.positions);
    assert.equal(expectedCases.length, analysisLimits.sourceInputs);
    assert.equal(
      new Set(expectedCases.map((entry) => entry.id)).size,
      expectedCases.length,
    );
    const manifest = JSON.parse(
      await readFile(`${directory}/analysis-corpus-manifest.json`, "utf8"),
    );
    const digest = (source: string) =>
      createHash("sha256").update(source).digest("hex");
    assert.deepEqual(
      expectedCases.map((entry) => ({
        id: entry.id,
        sourceSha256: digest(entry.input.source),
        configSha256: digest(entry.input.config),
        routes: entry.refusal ? 3 : 7,
      })),
      manifest.source,
      "frozen source manifest before execution",
    );
    assert.equal(manifest.source.length, 3168);
    assert.ok(manifest.supplement.length <= 1696);
    const allIds = [...manifest.source, ...manifest.supplement].map(
      (entry: { id: string }) => entry.id,
    );
    assert.equal(new Set(allIds).size, allIds.length);
    assert.ok(allIds.length <= 4864);
    assert.ok(
      [...manifest.source, ...manifest.supplement].reduce(
        (sum: number, entry: { routes: number }) => sum + entry.routes,
        0,
      ) <= 34048,
    );
    let calls = 0;
    const diagnostics: string[] = [];
    const createSession = () => {
      const worker = new Worker(workerSource, { eval: true });
      const native = spawn(
        "crates/idealis-core/target/release/idealis-core",
        [],
        { stdio: ["pipe", "pipe", "pipe"] },
      );
      const lines = createInterface({ input: native.stdout });
      const exited = new Promise<void>((resolve) =>
        native.once("close", () => resolve()),
      );
      let nativeReply: ((value: unknown) => void) | undefined;
      let workerReply: ((value: unknown) => void) | undefined;
      let readyReply: (() => void) | undefined;
      let requestId = 0;
      native.stderr.on("data", (chunk) => diagnostics.push(String(chunk)));
      lines.on("line", (line) => {
        const reply = nativeReply;
        nativeReply = undefined;
        reply?.(JSON.parse(line));
      });
      const ready = new Promise<void>((resolve, reject) => {
        readyReply = resolve;
        worker.once("error", reject);
        native.once("error", reject);
      });
      worker.on("message", (message) => {
        if (message.ready) readyReply?.();
        else {
          const reply = workerReply;
          workerReply = undefined;
          reply?.(message.result);
        }
      });
      return {
        ready,
        native: (request: unknown) =>
          new Promise<unknown>((resolve) => {
            calls++;
            nativeReply = resolve;
            native.stdin.write(`${JSON.stringify({ version: 1, request })}\n`);
          }),
        worker: (kind: string, value: Record<string, unknown>) =>
          new Promise<unknown>((resolve) => {
            calls +=
              kind === "compile"
                ? 2
                : Array.isArray(value.programs)
                  ? value.programs.length
                  : 0;
            workerReply = resolve;
            worker.postMessage({ id: ++requestId, kind, ...value });
          }),
        stop: async () => {
          native.stdin.destroy();
          native.kill("SIGTERM");
          const force = setTimeout(() => native.kill("SIGKILL"), 100);
          await Promise.all([worker.terminate(), exited]);
          clearTimeout(force);
          lines.close();
        },
      };
    };
    let session = createSession();
    const requestNative = (request: unknown) => session.native(request);
    const requestWorker = (kind: string, value: Record<string, unknown>) =>
      session.worker(kind, value);
    const ledger: {
      id: string;
      outcome: string;
      semanticReached: boolean;
      refusalReached: boolean;
      controlReached?: boolean;
      unusedDefinitionReached?: boolean;
      assertion?: string;
    }[] = [];
    let deadline = false;
    let unavailable = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const waitReady = async () => {
      let preparationTimer: ReturnType<typeof setTimeout> | undefined;
      try {
        await Promise.race([
          session.ready,
          new Promise<never>((_, reject) => {
            preparationTimer = setTimeout(
              () => reject(new Error("worker setup timeout")),
              Math.min(
                1000,
                Math.max(1, analysisLimits.deadlineMs - performance.now()),
              ),
            );
          }),
        ]);
      } finally {
        clearTimeout(preparationTimer);
      }
    };
    try {
      await waitReady();
      for (const entry of expectedCases) {
        if (
          deadline ||
          unavailable ||
          performance.now() - start >= analysisLimits.deadlineMs
        ) {
          deadline = performance.now() - start >= analysisLimits.deadlineMs;
          ledger.push({
            id: entry.id,
            outcome: "unexecuted",
            semanticReached: false,
            refusalReached: false,
          });
          continue;
        }
        await t.test(entry.id, async () => {
          let semanticReached = false;
          let refusalReached = false;
          try {
            await Promise.race([
              (async () => {
                assert.ok(
                  Buffer.byteLength(entry.input.source) <= analysisLimits.bytes,
                  "source byte bound",
                );
                assertSourceReach(entry);
                const source = sourceReach(entry);
                assert.ok(
                  source.value !== undefined,
                  "source position must exist",
                );
                assert.equal(
                  source.sentinel.length,
                  1,
                  `exactly one probe at ${entry.sourcePath}`,
                );
                assert.ok(
                  source.module.definitions.length <=
                    analysisLimits.definitions,
                  "definition bound",
                );
                const [local, rust] = await Promise.all([
                  requestWorker("compile", { input: entry.input }),
                  requestNative({ kind: "compile-tla", ...entry.input }),
                ]);
                assert.ok(Array.isArray(local));
                const compiled = [local[0], rust, local[1]];
                if (entry.refusal) {
                  for (const result of compiled)
                    assert.deepEqual(result, {
                      kind: "error",
                      code: "protocol",
                      message: entry.refusal,
                    });
                  refusalReached = true;
                  return;
                }
                const programs = compiled.map((result) => {
                  assert.ok(isRecord(result));
                  assert.equal(result.kind, "compiled", JSON.stringify(result));
                  assert.ok(isRecord(result.program));
                  assert.equal(result.program.version, 2);
                  return result.program;
                });
                assertIrReach(entry, programs[0]);
                assertIrReach(entry, programs[1]);
                assert.deepEqual(
                  normalize(programs[0]),
                  normalize(programs[1]),
                );
                assert.deepEqual(
                  programs[1],
                  programs[2],
                  "native/Wasm complete compile response",
                );
                if (entry.probe !== "old") {
                  const nodes = sourceObjects(programs[0]);
                  assert.ok(
                    entry.position.constructor === "Always"
                      ? Array.isArray(programs[0]?.invariants) &&
                          programs[0].invariants.some(
                            (predicate) =>
                              isRecord(predicate) && predicate.name === "Safe",
                          )
                      : nodes.some(
                          (node) =>
                            node.op === entry.expectedOp ||
                            node.kind === entry.expectedOp,
                        ),
                    `independent IR constructor ${entry.expectedOp}`,
                  );
                  if (entry.position.spelling === "SubSeq") {
                    const subseq = nodes.find((node) => node.op === "subseq");
                    assert.ok(subseq);
                    assert.deepEqual(Object.keys(subseq).sort(), [
                      "from",
                      "op",
                      "sequence",
                      "to",
                    ]);
                    const path = ["sequence", "from", "to"][
                      Number(entry.position.position[5])
                    ];
                    assert.ok(path);
                    assert.ok(
                      sourceObjects(subseq[path]).some((node) =>
                        entry.probe === "binder" || entry.probe === "target"
                          ? node.op === "call" || node.op === "enabled"
                          : ["in", "function-in", "sequence-in"].includes(
                              String(node.op),
                            ),
                      ),
                      `probe reaches subseq.${path}`,
                    );
                  }
                  if (entry.probe === "membership")
                    assert.ok(
                      nodes.some((node) =>
                        ["in", "function-in", "sequence-in"].includes(
                          String(node.op),
                        ),
                      ),
                      "membership descriptor start",
                    );
                  if (entry.probe === "target")
                    assert.ok(
                      nodes.some((node) => node.op === "enabled"),
                      "resolved ENABLED branches",
                    );
                  if (entry.probe === "binder")
                    assert.ok(
                      nodes.some((node) => node.op === "parameter"),
                      "caller frame parameter retained",
                    );
                }
                const pair = [programs[0], programs[1]];
                const wasm = await requestWorker("check", {
                  programs: pair,
                  bounds: analysisBounds,
                });
                assert.ok(Array.isArray(wasm));
                const results = [
                  await requestNative({
                    kind: "check-safety",
                    program: pair[0],
                    bounds: analysisBounds,
                  }),
                  wasm[0],
                  await requestNative({
                    kind: "check-safety",
                    program: pair[1],
                    bounds: analysisBounds,
                  }),
                  wasm[1],
                ];
                for (const result of results) {
                  assert.deepEqual(
                    result,
                    results[0],
                    "four checker paths including diagnostics",
                  );
                  assert.ok(isRecord(result));
                  assert.equal(result.kind, "safety", JSON.stringify(result));
                  assert.ok(isRecord(result.result));
                  assert.deepEqual(
                    { ...result.result, work: undefined },
                    {
                      status: "complete",
                      reason: null,
                      states: 1,
                      generated: 2,
                      depth: 0,
                      work: undefined,
                      violation: null,
                    },
                  );
                }
                semanticReached =
                  entry.probe !== "old" ||
                  (entry.position.constructor === "Except" &&
                    entry.position.position.endsWith(".value"));
              })(),
              new Promise<never>((_, reject) => {
                timer = setTimeout(
                  () => reject(new Error("input timeout")),
                  Math.min(
                    1000,
                    Math.max(
                      1,
                      analysisLimits.deadlineMs - (performance.now() - start),
                    ),
                  ),
                );
              }),
            ]);
            ledger.push({
              id: entry.id,
              outcome: "passed",
              semanticReached,
              refusalReached,
              controlReached:
                entry.probe === "old" && entry.control && !semanticReached,
              unusedDefinitionReached:
                entry.probe === "old" &&
                !entry.control &&
                !semanticReached &&
                !refusalReached,
            });
          } catch (error) {
            const message =
              error instanceof Error ? error.message : String(error);
            ledger.push({
              id: entry.id,
              outcome: message === "input timeout" ? "timeout" : "failed",
              semanticReached: false,
              refusalReached: false,
              assertion: message,
            });
            if (message === "input timeout") {
              await session.stop();
              deadline = performance.now() - start >= analysisLimits.deadlineMs;
              if (!deadline) {
                session = createSession();
                try {
                  await waitReady();
                } catch (preparationError) {
                  unavailable = true;
                  diagnostics.push(String(preparationError));
                }
              }
            }
            throw error;
          } finally {
            clearTimeout(timer);
          }
        });
      }
    } catch (error) {
      const recorded = new Set(ledger.map((entry) => entry.id));
      for (const entry of expectedCases)
        if (!recorded.has(entry.id))
          ledger.push({
            id: entry.id,
            outcome: "unexecuted",
            semanticReached: false,
            refusalReached: false,
            assertion: String(error),
          });
      throw error;
    } finally {
      await session.stop();
      const elapsedMs = performance.now() - start;
      const report = {
        limits: analysisLimits,
        positions: analysisPositions(),
        expectedIds: expectedCases.map((entry) => entry.id),
        elapsedMs,
        calls,
        deadline,
        unavailable,
        stderr: diagnostics.join(""),
        ledger,
      };
      if (process.env.IDEALIS_ANALYSIS_REPORT) {
        await mkdir(process.env.IDEALIS_ANALYSIS_REPORT, { recursive: true });
        await writeFile(
          `${process.env.IDEALIS_ANALYSIS_REPORT}/corpus-ledger.json`,
          `${JSON.stringify(report, null, 2)}\n`,
        );
      }
      t.diagnostic(
        JSON.stringify({
          inputs: expectedCases.length,
          executed: ledger.filter((entry) => entry.outcome !== "unexecuted")
            .length,
          reached: ledger.filter((entry) => entry.outcome === "passed").length,
          elapsedMs,
          calls,
          deadline,
        }),
      );
      assert.deepEqual(
        ledger.map((entry) => entry.id),
        expectedCases.map((entry) => entry.id),
        "ledger two-way ID equality",
      );
      assert.ok(
        elapsedMs <= analysisLimits.deadlineMs,
        "180 second corpus bound",
      );
      assert.ok(calls <= analysisLimits.calls, "call count bound");
      assert.deepEqual(
        ledger
          .filter((entry) => entry.outcome !== "passed")
          .map((entry) => entry.id),
        [],
        "all generated IDs must reach their intended assertion",
      );
    }
  },
);
