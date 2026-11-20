import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { test } from "node:test";
import { isRecord } from "../../packages/idealis/src/trace.js";

const supplementalSource = `
import { registerHooks } from 'node:module';
const registryUrl = 'data:text/javascript,' + encodeURIComponent('export const registered = []; export const test = (name, run) => registered.push({name,run});');
const { registered } = await import(registryUrl);
registerHooks({resolve(specifier,context,next){return specifier === 'node:test' ? {url:registryUrl,shortCircuit:true} : next(specifier,context);}});
for (const module of ['analysis-parity-controls','analysis-ir-corpus','analysis-boundary-tests','frame-decoder-test']) await import(new URL('./dist/tests/tlc-differential/'+module+'.js','file://'+process.cwd()+'/'));
const tests = registered.filter(entry => entry.name.startsWith('analysis parity'));
process.on('message', async ({index}) => {
  try { await tests[index].run(); process.send({passed:true}); }
  catch (error) { process.send({passed:false,message:error.message,stack:error.stack,code:error.code}); }
});
process.send({names:tests.map(entry=>entry.name)});
`;

test(
  "analysis parity supplemental corpus with outside watchdog",
  {
    timeout: 181_000,
  },
  async (t) => {
    const fixed = JSON.parse(
      await readFile(
        "fixtures/tlc-operator-sharing/analysis-corpus-manifest.json",
        "utf8",
      ),
    );
    for (const [name, expected] of Object.entries(fixed.supplementalSources))
      assert.equal(
        createHash("sha256")
          .update(await readFile(`tests/tlc-differential/${name}`))
          .digest("hex"),
        expected,
        `fixed supplemental source ${name}`,
      );
    const results: { name: string; outcome: string; assertion?: string }[] = [];
    const createSession = () => {
      const child = spawn(
        process.execPath,
        ["--input-type=module", "-e", supplementalSource],
        { detached: true, stdio: ["ignore", "pipe", "pipe", "ipc"] },
      );
      const exited = new Promise<void>((resolve) =>
        child.once("close", () => resolve()),
      );
      let reply: ((value: unknown) => void) | undefined;
      let prepared: ((names: readonly string[]) => void) | undefined;
      const ready = new Promise<readonly string[]>((resolve, reject) => {
        prepared = resolve;
        child.once("error", reject);
      });
      child.on("message", (value) => {
        assert.ok(isRecord(value));
        if (Array.isArray(value.names)) prepared?.(value.names);
        else {
          const resolve = reply;
          reply = undefined;
          resolve?.(value);
        }
      });
      return {
        ready,
        run: (index: number) =>
          new Promise<unknown>((resolve) => {
            reply = resolve;
            child.send({ index });
          }),
        stop: async () => {
          try {
            if (child.pid) process.kill(-child.pid, "SIGKILL");
          } catch (error) {
            assert.equal((error as NodeJS.ErrnoException).code, "ESRCH");
          }
          await exited;
        },
      };
    };
    let session = createSession();
    const bounded = async <Value>(promise: Promise<Value>): Promise<Value> => {
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        return await Promise.race([
          promise,
          new Promise<never>((_, reject) => {
            timer = setTimeout(
              () => reject(new Error("input timeout")),
              Math.min(1000, Math.max(1, 180_000 - performance.now())),
            );
          }),
        ]);
      } finally {
        clearTimeout(timer);
      }
    };
    try {
      const names = await bounded(session.ready);
      assert.deepEqual(
        names,
        fixed.registeredTests,
        "fixed supplemental test registrations before execution",
      );
      for (const [index, name] of names.entries()) {
        if (performance.now() >= 180_000) {
          results.push({ name, outcome: "unexecuted" });
          continue;
        }
        await t.test(name, async () => {
          try {
            const result = await bounded(session.run(index));
            assert.ok(isRecord(result));
            if (!result.passed) assert.fail(String(result.message));
            results.push({ name, outcome: "passed" });
          } catch (error) {
            const assertion =
              error instanceof Error ? error.message : String(error);
            results.push({
              name,
              outcome: assertion === "input timeout" ? "timeout" : "failed",
              assertion,
            });
            if (assertion === "input timeout") {
              await session.stop();
              if (performance.now() < 180_000) {
                session = createSession();
                assert.deepEqual(await bounded(session.ready), names);
              }
            }
            throw error;
          }
        });
      }
    } finally {
      await session.stop();
      if (process.env.IDEALIS_ANALYSIS_REPORT) {
        await mkdir(process.env.IDEALIS_ANALYSIS_REPORT, { recursive: true });
        await writeFile(
          `${process.env.IDEALIS_ANALYSIS_REPORT}/supplement-ledger.json`,
          `${JSON.stringify({ elapsedMs: performance.now(), results }, null, 2)}\n`,
        );
      }
      assert.equal(
        results.length,
        fixed.registeredTests.length,
        "all supplemental IDs including unexecuted IDs",
      );
      assert.ok(performance.now() <= 180_000, "180 second total corpus bound");
    }
  },
);
