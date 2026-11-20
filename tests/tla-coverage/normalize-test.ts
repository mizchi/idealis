import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  normalizeRtla,
  normalizeSany,
} from "../../evals/experiments/tla-coverage/normalize.js";
import { isRecord } from "../../packages/idealis/src/trace.js";

const fixture: {
  readonly cases: readonly { readonly name: string; readonly tree: unknown }[];
} = JSON.parse(await readFile("fixtures/tla-coverage/sany-trees.json", "utf8"));
const sample = (name: string) => {
  const found = fixture.cases.find((entry) => entry.name === name);
  assert.ok(found);
  return found.tree;
};
const items = (tree: unknown) => {
  const value = normalizeSany(tree);
  assert.ok(isRecord(value));
  return value;
};
const definitions = (tree: unknown) => {
  const values = items(tree).definitions;
  assert.ok(Array.isArray(values));
  return values;
};

test("independent SANY normalization retains operator binding and argument order", () => {
  const definition = definitions(fixture.cases[0]?.tree)[0];
  assert.ok(isRecord(definition));
  assert.deepEqual(definition.body, {
    kind: "Apply",
    operator: "SUBSET",
    args: [
      {
        kind: "Apply",
        operator: "..",
        args: [
          { kind: "Number", value: "1" },
          { kind: "Number", value: "2" },
        ],
      },
    ],
  });
  assert.notDeepEqual(
    normalizeSany(fixture.cases[0]?.tree),
    normalizeRtla({
      definitions: [
        {
          kind: "Apply",
          operator: "..",
          args: [
            {
              kind: "Apply",
              operator: "SUBSET",
              args: [{ kind: "Number", value: 1 }],
            },
            { kind: "Number", value: 2 },
          ],
        },
      ],
    }),
  );
});

test("independent SANY normalization retains proof hierarchy, step references and QED", () => {
  const tree = items(sample("Proof by QED with implicit step level"));
  assert.deepEqual(tree.theorems, [
    {
      statement: { kind: "Identifier", name: "TRUE" },
      proof: {
        kind: "Steps",
        steps: [
          {
            number: "<1>a",
            level: 1,
            body: {
              kind: "Assert",
              suffices: false,
              statement: { kind: "Identifier", name: "TRUE" },
            },
          },
          {
            number: "<1>b",
            level: 1,
            body: { kind: "Qed" },
            proof: {
              kind: "By",
              only: false,
              facts: [
                {
                  kind: "Expression",
                  expression: { kind: "Identifier", name: "<1>a" },
                },
              ],
              definitions: [],
            },
          },
        ],
      },
    },
  ]);
});

test("native normalization retains binder patterns, captures and numeric selectors", () => {
  const root = {
    kind: "Selection",
    expression: { kind: "Identifier", name: "F" },
    selectors: [
      { kind: "Index", index: 1 },
      { kind: "Arguments", args: [{ kind: "Identifier", name: "bound" }] },
    ],
  };
  assert.notDeepEqual(
    normalizeRtla(root),
    normalizeRtla({
      ...root,
      selectors: [
        { kind: "Index", index: 2 },
        { kind: "Arguments", args: [{ kind: "Identifier", name: "bound" }] },
      ],
    }),
  );
  assert.notDeepEqual(
    normalizeRtla({
      kind: "Forall",
      bounds: [
        {
          pattern: { kind: "Tuple", names: ["left", "right"] },
          set: { kind: "Identifier", name: "S" },
        },
      ],
      body: root,
    }),
    normalizeRtla({
      kind: "Forall",
      bounds: [
        {
          pattern: { kind: "Name", name: "left" },
          set: { kind: "Identifier", name: "S" },
        },
      ],
      body: root,
    }),
  );
  assert.notDeepEqual(
    normalizeRtla({
      kind: "Lambda",
      params: [{ name: "value", arity: 0 }],
      body: { kind: "Identifier", name: "capture" },
    }),
    normalizeRtla({
      kind: "Lambda",
      params: [{ name: "value", arity: 0 }],
      body: { kind: "Identifier", name: "value" },
    }),
  );
});

test("normalization reports unknown syntax nodes instead of silently dropping them", () => {
  assert.throws(
    () => normalizeSany({ image: "N_FutureSyntax", children: [] }),
    /Unnormalized/,
  );
  assert.throws(
    () => normalizeSany({ image: "N_Module", children: [] }),
    /Missing required SANY/,
  );
});
