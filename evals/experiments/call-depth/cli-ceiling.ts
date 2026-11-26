import assert from "node:assert/strict";
import {
  closeSync,
  mkdirSync,
  openSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { WASI } from "node:wasi";
import { type Ceiling, hopSource, search } from "./wasm-ceiling.js";

const negationSource = (levels: number): string =>
  [
    "---- MODULE chain ----",
    "EXTENDS Integers",
    "VARIABLE counter",
    "neg0(x) == x",
    ...Array.from(
      { length: levels },
      (_, index) => `neg${index + 1}(x) == -neg${index}(x)`,
    ),
    "Init == counter = 0",
    "Next == counter' = (counter + 1) % 5",
    `Inv == neg${levels}(counter) <= 4`,
    "====",
    "",
  ].join("\n");

const sources: Readonly<Record<string, (levels: number) => string>> = {
  hop: hopSource,
  operator: negationSource,
};

const config = "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";

export const runCli =
  (module: WebAssembly.Module, directory: string, cell: string) =>
  async (size: number): Promise<string> => {
    const work = join(directory, `${cell}-${size}`);
    mkdirSync(work, { recursive: true });
    const build = sources[cell];
    assert.ok(build, `cell ${cell}`);
    writeFileSync(join(work, "chain.tla"), build(size));
    writeFileSync(join(work, "chain.cfg"), config);
    const stdout = openSync(join(work, "stdout.txt"), "w");
    const stderr = openSync(join(work, "stderr.txt"), "w");
    const wasi = new WASI({
      version: "preview1",
      args: [
        "idealis",
        "tlc",
        "-workers",
        "1",
        "-config",
        "/work/chain.cfg",
        "/work/chain.tla",
      ],
      env: { IDEALIS_CWD: "/work" },
      preopens: { "/work": work },
      stdout,
      stderr,
      returnOnExit: true,
    });
    let outcome: string;
    try {
      const instance = await WebAssembly.instantiate(module, {
        wasi_snapshot_preview1: wasi.wasiImport,
      });
      const code = wasi.start(instance);
      closeSync(stdout);
      closeSync(stderr);
      const text =
        readFileSync(join(work, "stdout.txt"), "utf8") +
        readFileSync(join(work, "stderr.txt"), "utf8");
      const refusal = /Safety evaluation nesting exceeds \d+/.exec(text);
      outcome =
        code === 0
          ? "complete"
          : refusal
            ? `error ${refusal[0]}`
            : `exit ${code}: ${text.trim().split("\n")[0]?.slice(0, 120) ?? ""}`;
    } catch (error) {
      assert.ok(error instanceof Error);
      outcome = `${error.constructor.name}: ${error.message}`;
    }
    return outcome;
  };

const [wasmPath, directory, output, ...cells] = process.argv.slice(2);
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  assert.ok(
    wasmPath && directory && output && cells.length > 0,
    "usage: cli-ceiling.js <idealis.wasm> <work dir> <output.json> <cell:low:high>...",
  );
  const module = await WebAssembly.compile(
    new Uint8Array(readFileSync(wasmPath)),
  );
  const ceilings: Ceiling[] = [];
  for (const spec of cells) {
    const [cell, low, high] = spec.split(":");
    assert.ok(cell && low && high, `cell ${spec}`);
    const ceiling = await search(
      runCli(module, directory, cell),
      cell,
      Number(low),
      Number(high),
    );
    ceilings.push(ceiling);
    console.log(cell, ceiling.largest, ceiling.firstFailing, ceiling.failure);
  }
  writeFileSync(output, `${JSON.stringify(ceilings, null, 1)}\n`, {
    flag: "wx",
  });
}
