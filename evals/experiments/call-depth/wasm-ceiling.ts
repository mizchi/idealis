import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export type Dispatch = (command: unknown) => string;

export const options = { completeOnly: false, maxWork: 1_000_000 };

export const parseFlags = (
  argv: readonly string[],
): { positional: string[] } => {
  const positional: string[] = [];
  for (const argument of argv) {
    if (argument === "--complete-only") options.completeOnly = true;
    else if (argument.startsWith("--max-work="))
      options.maxWork = Number(argument.slice("--max-work=".length));
    else positional.push(argument);
  }
  return { positional };
};

export const typedRefusal = (outcome: string): boolean =>
  outcome.startsWith("error ");
export const accepted = (outcome: string): boolean =>
  (options.completeOnly ? /^complete/ : /^(complete|bounded)/).test(outcome);

export const hopSource = (levels: number): string =>
  [
    "---- MODULE chain ----",
    "EXTENDS Integers",
    "VARIABLE counter",
    "hop0(x) == x + 1",
    ...Array.from(
      { length: levels },
      (_, index) => `hop${index + 1}(x) == hop${index}(x) + 1`,
    ),
    "Init == counter = 0",
    `Next == counter' = hop${levels}(counter) % 5`,
    "Inv == counter >= 0",
    "====",
    "",
  ].join("\n");

export const instantiateDispatch = (bytes: Uint8Array): Dispatch => {
  const module = new WebAssembly.Module(new Uint8Array(bytes));
  return (command) => {
    const instance = new WebAssembly.Instance(module, {});
    const exports = instance.exports as Record<string, WebAssembly.ExportValue>;
    const resize = exports.modelt_input_resize as (length: number) => number;
    const dispatch = exports.modelt_dispatch as () => void;
    const outputPointer = exports.modelt_output_ptr as () => number;
    const outputLength = exports.modelt_output_len as () => number;
    const memory = exports.memory as WebAssembly.Memory;
    const input = new TextEncoder().encode(JSON.stringify(command));
    const pointer = resize(input.length) >>> 0;
    new Uint8Array(memory.buffer, pointer, input.length).set(input);
    try {
      dispatch();
    } catch (error) {
      assert.ok(error instanceof Error);
      return `${error.constructor.name}: ${error.message}`;
    }
    const output = new Uint8Array(
      memory.buffer,
      outputPointer() >>> 0,
      outputLength() >>> 0,
    ).slice();
    const reply = JSON.parse(new TextDecoder().decode(output));
    if (reply.kind === "error")
      return `error ${reply.code}: ${reply.message}${reply.probe ? ` peak=${reply.probe.peakDepth} total=${reply.probe.peakTotal} hits=${reply.probe.hits256}` : ""}`;
    const result = reply.result ?? reply;
    const probe = reply.probe
      ? ` peak=${reply.probe.peakDepth} total=${reply.probe.peakTotal} hits=${reply.probe.hits256}`
      : "";
    return `${result.status} ${result.states} ${result.generated} ${result.work}${probe}`;
  };
};

const hopBounds = { maxDepth: 64, maxStates: 10000, maxWork: 100_000_000 };
export const kindBounds = () => ({
  maxDepth: 10,
  maxStates: 100,
  maxWork: options.maxWork,
  maxTemporalNodes: 100_000,
});

export const programOf = (harness: string, cell: string, size: number) =>
  JSON.parse(
    execFileSync(
      harness,
      cell === "hop"
        ? ["emit-hop-ir", String(size)]
        : ["emit-kind", cell, String(size)],
      { encoding: "utf8", maxBuffer: 1 << 28 },
    ),
  );

export const probe =
  (dispatch: Dispatch, harness: string, cell: string) =>
  (size: number): string => {
    if (cell === "hop-source")
      return dispatch({
        version: 1,
        request: {
          kind: "check-tla",
          source: hopSource(size),
          config: "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
          bounds: hopBounds,
          irVersion: 2,
        },
      });
    return dispatch({
      version: 1,
      request: {
        kind: "check-safety",
        program: programOf(harness, cell, size),
        bounds: cell === "hop" ? hopBounds : kindBounds(),
      },
    });
  };

export interface Ceiling {
  readonly cell: string;
  readonly largest: number | null;
  readonly accepted: string | null;
  readonly firstFailing: number | null;
  readonly failure: string | null;
  readonly searched: readonly [number, number];
}

export const search = async (
  run: (size: number) => string | Promise<string>,
  cell: string,
  low: number,
  high: number,
): Promise<Ceiling> => {
  const outcomes = new Map<number, string>();
  const outcome = async (size: number): Promise<string> => {
    const known = outcomes.get(size);
    if (known !== undefined) return known;
    const fresh = await run(size);
    outcomes.set(size, fresh);
    return fresh;
  };
  const searched = [low, high] as const;
  if (!accepted(await outcome(low)))
    return {
      cell,
      largest: null,
      accepted: null,
      firstFailing: low,
      failure: await outcome(low),
      searched,
    };
  if (accepted(await outcome(high)))
    return {
      cell,
      largest: high,
      accepted: await outcome(high),
      firstFailing: null,
      failure: null,
      searched,
    };
  let lower = low;
  let upper = high;
  while (upper - lower > 1) {
    const middle = (lower + upper) >> 1;
    if (accepted(await outcome(middle))) lower = middle;
    else upper = middle;
  }
  return {
    cell,
    largest: lower,
    accepted: await outcome(lower),
    firstFailing: upper,
    failure: await outcome(upper),
    searched,
  };
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [wasmPath, harness, output, ...cells] = parseFlags(
    process.argv.slice(2),
  ).positional;
  assert.ok(
    wasmPath && harness && output,
    "usage: wasm-ceiling.js <wasm> <harness> <output.json> <cell:low:high>...",
  );
  assert.ok(cells.length > 0, "cells: <cell>:<low>:<high>");
  const dispatch = instantiateDispatch(readFileSync(wasmPath));
  const ceilings: Ceiling[] = [];
  for (const spec of cells) {
    const [cell, low, high] = spec.split(":");
    assert.ok(cell && low && high, `cell ${spec}`);
    ceilings.push(
      await search(
        probe(dispatch, harness, cell),
        cell,
        Number(low),
        Number(high),
      ),
    );
  }
  writeFileSync(output, `${JSON.stringify(ceilings, null, 1)}\n`, {
    flag: "wx",
  });
  for (const ceiling of ceilings)
    console.log(
      ceiling.cell,
      ceiling.largest,
      ceiling.firstFailing,
      ceiling.failure,
    );
}
