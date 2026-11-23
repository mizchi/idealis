import { isRecord } from "../../../packages/idealis/src/trace.js";

export interface Scope {
  readonly signatures: readonly {
    readonly name: string;
    readonly maximum: number;
    readonly exact: boolean;
  }[];
  readonly integerBitwidth: number;
}
export interface Instance {
  readonly signatures: Readonly<Record<string, readonly string[]>>;
  readonly fields: Readonly<Record<string, readonly (readonly string[])[]>>;
}
export interface Analysis {
  readonly command: {
    readonly label: string;
    readonly kind: "run" | "check";
    readonly scope: Scope;
  };
  readonly status:
    | "instance"
    | "counterexample"
    | "no-instance"
    | "no-counterexample"
    | "unknown";
  readonly instance: Instance | null;
  readonly stats: {
    readonly work: number;
    readonly candidates: number;
    readonly factInstances: number;
  };
}
export interface Request {
  readonly kind: "analyze";
  readonly source: string;
  readonly command: number;
  readonly maxWork: number;
}
export type Reply =
  | { readonly kind: "analysis"; readonly result: Analysis }
  | {
      readonly kind: "error";
      readonly error: { readonly kind: string; readonly message: string };
    };

export const decode = (input: unknown): Reply => {
  if (!isRecord(input)) throw new Error("Invalid ralloy reply");
  if (
    input.kind === "error" &&
    isRecord(input.error) &&
    typeof input.error.kind === "string" &&
    ["syntax", "type", "unsupported", "scope", "resource", "protocol"].includes(
      input.error.kind,
    ) &&
    typeof input.error.message === "string"
  )
    return {
      kind: "error",
      error: { kind: input.error.kind, message: input.error.message },
    };
  const result = input.result;
  if (
    input.kind !== "analysis" ||
    !isRecord(result) ||
    !isRecord(result.command) ||
    typeof result.command.label !== "string" ||
    (result.command.kind !== "run" && result.command.kind !== "check") ||
    !isRecord(result.command.scope) ||
    !Array.isArray(result.command.scope.signatures) ||
    !Number.isSafeInteger(result.command.scope.integerBitwidth) ||
    typeof result.status !== "string" ||
    ![
      "instance",
      "counterexample",
      "no-instance",
      "no-counterexample",
      "unknown",
    ].includes(result.status) ||
    !isRecord(result.stats) ||
    [
      result.stats.work,
      result.stats.candidates,
      result.stats.factInstances,
    ].some(
      (count) =>
        typeof count !== "number" || !Number.isSafeInteger(count) || count < 0,
    )
  )
    throw new Error("Invalid ralloy analysis");
  const scope = result.command.scope;
  if (
    typeof scope.integerBitwidth !== "number" ||
    scope.integerBitwidth < 0 ||
    scope.integerBitwidth > 5
  )
    throw new Error("Invalid ralloy bitwidth");
  if (!Array.isArray(scope.signatures))
    throw new Error("Invalid ralloy scopes");
  const signatures = scope.signatures.map((bound: unknown) => {
    if (
      !isRecord(bound) ||
      typeof bound.name !== "string" ||
      typeof bound.maximum !== "number" ||
      !Number.isSafeInteger(bound.maximum) ||
      bound.maximum < 0 ||
      bound.maximum > 8 ||
      typeof bound.exact !== "boolean"
    )
      throw new Error("Invalid ralloy scope");
    return { name: bound.name, maximum: bound.maximum, exact: bound.exact };
  });
  const witness = result.instance;
  let instance: Instance | null = null;
  if (result.status === "instance" || result.status === "counterexample") {
    if (
      !isRecord(witness) ||
      !isRecord(witness.signatures) ||
      !isRecord(witness.fields)
    )
      throw new Error("Missing ralloy instance");
    const signatureEntries = Object.entries(witness.signatures).map(
      ([name, atoms]) => {
        if (
          !Array.isArray(atoms) ||
          atoms.some((atom) => typeof atom !== "string")
        )
          throw new Error("Invalid ralloy atoms");
        return [name, atoms] as const;
      },
    );
    const fieldEntries = Object.entries(witness.fields).map(
      ([name, tuples]) => {
        if (!Array.isArray(tuples)) throw new Error("Invalid ralloy field");
        const pairs = tuples.map((tuple: unknown): readonly string[] => {
          if (
            !Array.isArray(tuple) ||
            tuple.length < 2 ||
            tuple.some((atom) => typeof atom !== "string")
          )
            throw new Error("Invalid ralloy tuple");
          return tuple as string[];
        });
        return [name, pairs] as const;
      },
    );
    instance = {
      signatures: Object.fromEntries(signatureEntries),
      fields: Object.fromEntries(fieldEntries),
    };
  } else if (witness !== null) throw new Error("Unexpected ralloy instance");
  if (
    typeof result.stats.work !== "number" ||
    typeof result.stats.candidates !== "number" ||
    typeof result.stats.factInstances !== "number"
  )
    throw new Error("Invalid ralloy stats");
  if (
    result.status !== "instance" &&
    result.status !== "counterexample" &&
    result.status !== "no-instance" &&
    result.status !== "no-counterexample" &&
    result.status !== "unknown"
  )
    throw new Error("Invalid ralloy status");
  return {
    kind: "analysis",
    result: {
      command: {
        label: result.command.label,
        kind: result.command.kind,
        scope: { signatures, integerBitwidth: scope.integerBitwidth },
      },
      status: result.status,
      instance,
      stats: {
        work: result.stats.work,
        candidates: result.stats.candidates,
        factInstances: result.stats.factInstances,
      },
    },
  };
};
export const analysis = (reply: Reply): Analysis => {
  if (reply.kind !== "analysis") throw new Error(JSON.stringify(reply));
  return reply.result;
};
