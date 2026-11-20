import assert from "node:assert/strict";
import { handCases, source } from "../alloy-subset/corpus.js";
import { cases } from "../single-model/cases.js";
import { benchmarks } from "../smt-subset/corpus.js";

export interface Fixture {
  readonly id: string;
  readonly request: unknown;
  readonly requirements: readonly {
    readonly pointer: string;
    readonly value: unknown;
  }[];
}
export interface Suite {
  readonly id: string;
  readonly binary: string;
  readonly cases: readonly Fixture[];
}
export const catalog = (): readonly Suite[] => {
  const ring = cases("small").find((input) => input.name === "linear-ring");
  const cube = cases("small").find((input) => input.name === "function-cube");
  assert.ok(ring && cube);
  const models = [
    {
      ...ring,
      states: 1000,
      generated: 1001,
      config: ring.config.replace("N=17", "N=1000"),
    },
    {
      ...cube,
      states: 256,
      generated: 2049,
      config: cube.config.replace("N=4", "N=8"),
    },
  ];
  const fullCube = models[1];
  assert.ok(fullCube);
  return [
    {
      id: "rtla",
      binary: "crates/rtla/target/release/rtla",
      cases: models.map((input) => ({
        id: input.name,
        request: { kind: "module", source: input.source },
        requirements: [{ pointer: "/kind", value: "module" }],
      })),
    },
    {
      id: "idealis-core",
      binary: "crates/idealis-core/target/release/idealis-core",
      cases: [
        ...models.map((input) => ({
          id: input.name,
          request: {
            version: 1,
            request: {
              kind: "check-tla",
              source: input.source,
              config: input.config,
              bounds: {
                maxDepth: 100000,
                maxStates: 100000,
                maxWork: 10000000,
              },
            },
          },
          requirements: [
            { pointer: "/kind", value: "safety" },
            { pointer: "/result/status", value: "complete" },
            { pointer: "/result/states", value: input.states },
            { pointer: "/result/generated", value: input.generated },
          ],
        })),
        {
          id: "compile-cube",
          request: {
            version: 1,
            request: {
              kind: "compile-tla",
              source: fullCube.source,
              config: fullCube.config,
            },
          },
          requirements: [{ pointer: "/kind", value: "compiled" }],
        },
      ],
    },
    {
      id: "rusmt",
      binary: "crates/rusmt/target/release/rusmt",
      cases: benchmarks()
        .slice(0, 2)
        .map((input) => ({
          id: input.name,
          request: {
            version: 1,
            request: { kind: "solve", source: input.source, maxWork: 10000000 },
          },
          requirements: [
            { pointer: "/kind", value: "solution" },
            { pointer: "/result/status", value: input.expected },
          ],
        })),
    },
    {
      id: "ralloy",
      binary: "crates/ralloy/target/release/ralloy",
      cases: handCases()
        .slice(0, 2)
        .map((input) => ({
          id: input.name,
          request: {
            version: 1,
            request: {
              kind: "analyze",
              source: source(input),
              command: 0,
              maxWork: 1000000,
            },
          },
          requirements: [
            { pointer: "/kind", value: "analysis" },
            { pointer: "/result/status", value: input.expected },
          ],
        })),
    },
    {
      id: "idealis-connect",
      binary: "crates/idealis-cli/target/release/idealis-cli",
      cases: [1, 32].map((count) => ({
        id: `allowed-${count}`,
        request: {
          version: 1,
          request: {
            kind: "compare",
            oracle: {
              kind: "allowed",
              values: Array.from({ length: count }, (_, index) => index),
            },
            targets: [
              { id: "target", outcome: { kind: "value", value: count - 1 } },
            ],
          },
        },
        requirements: [
          { pointer: "/kind", value: "comparison" },
          { pointer: "/result/status", value: "matched" },
        ],
      })),
    },
    {
      id: "roracle",
      binary: "crates/roracle/target/release/roracle",
      cases: [1, 32].map((count) => ({
        id: `batch-${count}`,
        request: {
          version: 1,
          group: "batch",
          reference: "reference",
          implementations: ["candidate"],
          cases: Array.from({ length: count }, (_, index) => ({
            id: `case-${index}`,
            input: index,
            reference: { kind: "value", value: index },
            actual: [{ kind: "value", value: index }],
          })),
        },
        requirements: [
          { pointer: "/kind", value: "checked" },
          { pointer: "/report/status", value: "matched" },
        ],
      })),
    },
  ];
};
