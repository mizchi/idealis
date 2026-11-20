import assert from "node:assert/strict";
import {
  createNative,
  createZ3,
  maxWork,
  processClient,
} from "../smt-subset/clients.js";
import type { Input } from "../smt-subset/corpus.js";
import type { createWasm } from "../smt-subset/wasm.js";
import {
  type Assumption,
  decode,
  type Reply,
  type Request,
  solution,
} from "../smt-subset/wire.js";
import { assertOracle } from "../workspace-oracle/node.js";
import { controls, integer } from "./corpus.js";

export const verify = async (
  inputs: readonly Input[],
  seed: number,
  core: Awaited<ReturnType<typeof createWasm>>,
  onRecord: (record: unknown) => void,
  baselinePath?: string,
  baselineCore?: Awaited<ReturnType<typeof createWasm>>,
) => {
  const native = createNative();
  const z3 = await createZ3();
  const baseline = baselinePath
    ? processClient(baselinePath, [], true)
    : undefined;
  let sat = 0;
  let unsat = 0;
  let baselineAccepted = 0;
  let checks = 0;
  const request = async (command: Request): Promise<Reply> => {
    const wasm = core.request(command);
    const actual = await native.request(command);
    onRecord({ request: command, native: actual, wasm });
    assertOracle(actual, wasm);
    return wasm;
  };
  const validate = async (input: Input, reply: Reply) => {
    const actual = solution(reply);
    const reference = await z3.solve(input);
    onRecord({ input, actual, reference });
    assert.equal(actual.status, input.expected, input.name);
    assert.equal(actual.status, reference.status, input.name);
    if (actual.status === "sat") {
      assertOracle(
        Object.keys(actual.values).sort(),
        [...input.integers].sort(),
      );
      assertOracle(
        Object.keys(actual.booleans).sort(),
        [...input.booleans].sort(),
      );
      await z3.validate(input, actual);
      sat++;
    } else unsat++;
    checks++;
  };
  try {
    for (const input of inputs) {
      await validate(
        input,
        await request({ kind: "solve", source: input.source, maxWork }),
      );
      if (baseline) {
        const reply = decode(
          JSON.parse(
            await baseline.request(
              JSON.stringify({
                version: 1,
                request: { kind: "solve", source: input.source, maxWork },
              }),
            ),
          ),
        );
        const wasmReply = baselineCore?.request({
          kind: "solve",
          source: input.source,
          maxWork,
        });
        assertOracle(reply, wasmReply, "baseline native/Wasm");
        onRecord({ baseline: input.name, reply, wasm: wasmReply });
        if (reply.kind === "solution" && reply.result.status !== "unknown") {
          assert.equal(reply.result.status, input.expected);
          if (reply.result.status === "sat")
            await z3.validate(input, reply.result);
          baselineAccepted++;
        } else
          assert.equal(
            reply.kind,
            "error",
            `baseline unexpected unknown: ${input.name}`,
          );
      }
    }
    for (const source of controls)
      assert.equal(
        (await request({ kind: "solve", source, maxWork })).kind,
        "error",
        source,
      );

    // Expected assertion stacks are maintained independently of the Rust context.
    const declarations =
      "(declare-const x Int)(declare-const y Int)(declare-const b Bool)";
    const initial = "(>= x 0)";
    const frames: string[][] = [[initial]];
    const prepared = await request({
      kind: "prepare",
      source: `${declarations}(assert ${initial})(check-sat)`,
    });
    const other = await request({
      kind: "prepare",
      source: `${declarations}(assert (= x 99))`,
    });
    assert.equal(prepared.kind, "prepared");
    assert.equal(other.kind, "prepared");
    if (prepared.kind !== "prepared" || other.kind !== "prepared")
      throw new Error("Expected handles");
    const handle = prepared.handle;
    const check = async (
      expected: "sat" | "unsat",
      assumptions: readonly Assumption[] = [],
      target = handle,
    ) => {
      const assertions = target === handle ? frames.flat() : ["(= x 99)"];
      const source =
        declarations +
        [
          ...assertions,
          ...assumptions.map((value) =>
            value.positive ? value.symbol : `(not ${value.symbol})`,
          ),
        ]
          .map((value) => `(assert ${value})`)
          .join("");
      onRecord({
        intendedContext: {
          handle: target,
          frames: structuredClone(frames),
          assertions,
          assumptions,
          source,
        },
      });
      await validate(
        {
          name: `context-${checks}`,
          family: "context",
          integers: ["x", "y"],
          booleans: ["b"],
          source,
          expected,
          iterations: 1,
        },
        await request({ kind: "check", handle: target, maxWork, assumptions }),
      );
    };
    const edit = async (command: Request) =>
      assert.equal((await request(command)).kind, "updated");
    const add = async (expression: string) => {
      await edit({ kind: "assert", handle, expression });
      frames[frames.length - 1]?.push(expression);
    };
    const push = async () => {
      await edit({ kind: "push", handle, count: 1 });
      frames.push([]);
    };
    const pop = async () => {
      await edit({ kind: "pop", handle, count: 1 });
      frames.pop();
    };
    const positive = [{ symbol: "b", positive: true }];
    const negative = [{ symbol: "b", positive: false }];
    const value = 1 + (seed % 17);
    await push();
    await add(
      `(= (* 2 x) (ite b ${integer(2 * value)} ${integer(2 * (value + 1))}))`,
    );
    await check("sat", positive);
    await check("sat", negative);
    await check("sat");
    await push();
    await add(`(= x ${integer(value)})`);
    await check("sat", positive);
    await check("unsat", negative);
    await add(`(= x ${integer(value + 1)})`);
    await check("unsat");
    await pop();
    await check("sat", positive);
    for (const command of [
      { kind: "assert", handle, expression: "true)(assert false" },
      { kind: "assert", handle, expression: "(declare-const extra Int)" },
      { kind: "assert", handle, expression: "(= (* x y) 2)" },
      { kind: "pop", handle, count: 2 },
      { kind: "push", handle, count: 256 },
      {
        kind: "check",
        handle,
        maxWork,
        assumptions: [{ symbol: "x", positive: true }],
      },
      {
        kind: "check",
        handle,
        maxWork,
        assumptions: [{ symbol: "absent", positive: true }],
      },
      {
        kind: "check",
        handle,
        maxWork,
        assumptions: Array.from({ length: 257 }, () => ({
          symbol: "b",
          positive: true,
        })),
      },
    ] satisfies Request[]) {
      assert.equal((await request(command)).kind, "error");
      await check("sat", positive);
    }
    await check("unsat", [...positive, ...negative]);
    await check("sat");
    await check(
      "sat",
      Array.from({ length: 256 }, () => ({ symbol: "b", positive: true })),
    );
    const bounded = solution(
      await request({
        kind: "check",
        handle,
        maxWork: 0,
        assumptions: negative,
      }),
    );
    assert.equal(bounded.status, "unknown");
    assertOracle(bounded.values, {});
    assertOracle(bounded.booleans, {});
    await check("sat", [], other.handle);
    await pop();
    await add("(<= x 0)");
    await check("sat");
    await edit({ kind: "push", handle, count: 256 });
    assert.equal(
      (await request({ kind: "push", handle, count: 1 })).kind,
      "error",
    );
    await edit({ kind: "pop", handle, count: 256 });
    await check("sat");
    await edit({ kind: "push", handle, count: 0 });
    await edit({ kind: "pop", handle, count: 0 });
    for (const target of [handle, other.handle]) {
      assert.equal(
        (await request({ kind: "release", handle: target })).kind,
        "released",
      );
      assert.equal(
        (await request({ kind: "check", handle: target, maxWork })).kind,
        "error",
      );
      assert.equal(
        (await request({ kind: "assert", handle: target, expression: "true" }))
          .kind,
        "error",
      );
    }
    assert.ok(sat > 0 && unsat > 0);
    return {
      sat,
      unsat,
      checks,
      controls: controls.length,
      budgetUnknown: true,
      baselineAccepted: baseline ? baselineAccepted : null,
      inputs: inputs.length,
      z3Version: z3.version,
    };
  } finally {
    await Promise.all([native.close(), z3.close(), baseline?.close()]);
  }
};
