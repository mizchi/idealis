import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { chromium } from "@playwright/test";
import {
  type Ceiling,
  hopSource,
  kindBounds,
  parseFlags,
  programOf,
  search,
} from "./wasm-ceiling.js";

const dispatchSource = `
const dispatch = (module, command) => {
  const instance = new WebAssembly.Instance(module, {});
  const exports = instance.exports;
  const input = new TextEncoder().encode(JSON.stringify(command));
  const pointer = exports.modelt_input_resize(input.length) >>> 0;
  new Uint8Array(exports.memory.buffer, pointer, input.length).set(input);
  try {
    exports.modelt_dispatch();
  } catch (error) {
    return error.constructor.name + ": " + error.message;
  }
  const output = new Uint8Array(
    exports.memory.buffer,
    exports.modelt_output_ptr() >>> 0,
    exports.modelt_output_len() >>> 0,
  ).slice();
  const reply = JSON.parse(new TextDecoder().decode(output));
  const probe = reply.probe ? " peak=" + reply.probe.peakDepth + " total=" + reply.probe.peakTotal + " hits=" + reply.probe.hits256 : "";
  if (reply.kind === "error") return "error " + reply.code + ": " + reply.message + probe;
  const result = reply.result ?? reply;
  return result.status + " " + result.states + " " + result.generated + " " + result.work + probe;
};
`;

const hopBounds = { maxDepth: 64, maxStates: 10000, maxWork: 100_000_000 };
const [wasmPath, harness, output, ...cells] = parseFlags(
  process.argv.slice(2),
).positional;
assert.ok(
  wasmPath && harness && output && cells.length > 0,
  "usage: browser-ceiling.js <wasm> <harness> <output.json> <cell:low:high>...",
);
const server = createServer((_, response) => {
  response.setHeader("content-type", "text/html");
  response.end("<!doctype html><title>ceiling</title>");
});
await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
const address = server.address();
assert.ok(address !== null && typeof address === "object");
const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto(`http://127.0.0.1:${address.port}/`);
await page.evaluate(
  async ({ bytes, source }) => {
    const global = globalThis as unknown as Record<string, unknown>;
    const raw = Uint8Array.from(atob(bytes), (character) =>
      character.charCodeAt(0),
    );
    const module = await WebAssembly.compile(raw);
    global.ceilingModule = module;
    global.ceilingDispatch = new Function(`${source}; return dispatch;`)();
    const workerSource = `${source}
      let module;
      onmessage = (event) => {
        if (event.data.module) { module = event.data.module; postMessage("ready"); return; }
        postMessage(dispatch(module, event.data.command));
      };`;
    const worker = new Worker(
      URL.createObjectURL(
        new Blob([workerSource], { type: "text/javascript" }),
      ),
    );
    await new Promise((resolve) => {
      worker.onmessage = resolve;
      worker.postMessage({ module });
    });
    global.ceilingWorker = worker;
  },
  {
    bytes: readFileSync(wasmPath).toString("base64"),
    source: dispatchSource,
  },
);

const request = (cell: string, size: number) => ({
  version: 1,
  request:
    cell === "hop-source"
      ? {
          kind: "check-tla",
          source: hopSource(size),
          config: "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
          bounds: hopBounds,
          irVersion: 2,
        }
      : {
          kind: "check-safety",
          program: programOf(harness, cell, size),
          bounds: cell === "hop" ? hopBounds : kindBounds(),
        },
});

const hosts = {
  main: (command: unknown): Promise<string> =>
    page.evaluate((payload) => {
      const global = globalThis as unknown as {
        ceilingDispatch: (module: unknown, command: unknown) => string;
        ceilingModule: unknown;
      };
      return global.ceilingDispatch(global.ceilingModule, payload);
    }, command),
  worker: (command: unknown): Promise<string> =>
    page.evaluate(
      (payload) =>
        new Promise<string>((resolve) => {
          const global = globalThis as unknown as { ceilingWorker: Worker };
          global.ceilingWorker.onmessage = (event) => resolve(event.data);
          global.ceilingWorker.postMessage({ command: payload });
        }),
      command,
    ),
};

const results: Record<string, Ceiling[]> = { main: [], worker: [] };
for (const [name, run] of Object.entries(hosts))
  for (const spec of cells) {
    const [cell, low, high] = spec.split(":");
    assert.ok(cell && low && high, `cell ${spec}`);
    const ceiling = await search(
      (size) => run(request(cell, size)),
      cell,
      Number(low),
      Number(high),
    );
    results[name]?.push(ceiling);
    console.log(
      name,
      cell,
      ceiling.largest,
      ceiling.firstFailing,
      ceiling.failure,
    );
  }
writeFileSync(output, `${JSON.stringify(results, null, 1)}\n`, { flag: "wx" });
await browser.close();
server.close();
