import { evaluate } from "../../evals/experiments/controlled-schedules/driver.js";
import {
  plan,
  schedule,
} from "../../evals/experiments/controlled-schedules/fixture.js";
import { createTestingWasm } from "../../packages/idealis/src/testing.js";

export const run = async (assetBase = "") => {
  const bytes = new Uint8Array(
    await (await fetch(`${assetBase}/testing.wasm`)).arrayBuffer(),
  );
  const module = await WebAssembly.compile(bytes);
  const runtime = await createTestingWasm(module);
  const generator = await runtime.schedule(schedule(3));
  const input = plan(generator, 0x3801);
  const normal = await runtime.property(
    input,
    async (value) => (await evaluate(3, value, false)).verdict,
  );
  const broken = await runtime.property(
    input,
    async (value) => (await evaluate(3, value, true)).verdict,
  );
  if (!broken.counterexample)
    throw new Error("Missing schedule counterexample");
  const fault = await runtime.replay(
    broken.counterexample,
    async (value) => (await evaluate(3, value, true)).verdict,
    "lost-update",
  );
  const fixed = await runtime.replay(
    broken.counterexample,
    async (value) => (await evaluate(3, value, false)).verdict,
  );
  let called = false;
  const emptyBudget = await runtime.property({ ...input, maxWork: 0 }, () => {
    called = true;
    return { status: "matched" };
  });
  return {
    normal,
    broken,
    fault,
    fixed,
    emptyBudget,
    called,
    imports: WebAssembly.Module.imports(module),
    bytes: bytes.length,
    isolated: globalThis.crossOriginIsolated ?? false,
  };
};
