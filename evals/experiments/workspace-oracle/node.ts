import { AssertionError } from "node:assert";
import { readFileSync } from "node:fs";
import { createNativeOracle } from "../roracle-native/client.js";
import type { OracleClient } from "./protocol.js";
import { snapshot } from "./snapshot.js";
import { createOracle } from "./wasm.js";

let oracle: OracleClient | undefined;

const createWorkspaceOracle = (): OracleClient => {
  const backend = process.env.IDEALIS_ORACLE_BACKEND ?? "wasm";
  if (backend === "native-json" || backend === "native-object")
    return createNativeOracle(backend);
  if (backend !== "wasm") throw new Error(`Unknown oracle backend: ${backend}`);
  return createOracle(
    readFileSync(
      "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
    ),
  );
};

export const compareOracle = (
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) => {
  oracle ??= createWorkspaceOracle();
  return oracle.request({
    version: 1,
    group: "workspace-equality",
    reference: "expected",
    implementations: ["actual"],
    cases: [
      {
        id: "case",
        input: {
          label: message instanceof Error ? message.message : (message ?? null),
        },
        reference: { kind: "value", value: snapshot(expected) },
        actual: [{ kind: "value", value: snapshot(actual) }],
      },
    ],
  });
};

export const assertOracle = (
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) => {
  const checked = compareOracle(actual, expected, message);
  if (checked.status !== "matched")
    throw Object.assign(
      new AssertionError({
        actual,
        expected,
        operator: "roracle",
        message: message instanceof Error ? message.message : message,
      }),
      { report: checked.report },
    );
  return checked.report;
};
