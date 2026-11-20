import type { JsonValue } from "../../../packages/idealis/src/generated/roracle.js";

export const snapshot = (value: unknown): JsonValue => {
  const ancestors = new Set<object>();
  const visit = (input: unknown, depth: number): JsonValue => {
    if (depth > 48) throw new Error("Oracle snapshot nesting exceeds 48");
    if (input === null) return { type: "null" };
    switch (typeof input) {
      case "undefined":
        return { type: "undefined" };
      case "boolean":
        return { type: "boolean", value: input };
      case "string":
        return { type: "string", value: input };
      case "bigint":
        return { type: "bigint", value: input.toString() };
      case "number":
        return {
          type: "number",
          value: Object.is(input, -0) ? "-0" : String(input),
        };
      case "object":
        break;
      default:
        throw new Error(`Unsupported oracle snapshot: ${typeof input}`);
    }
    if (ancestors.has(input)) throw new Error("Cyclic oracle snapshot");
    if (Object.getOwnPropertySymbols(input).length > 0)
      throw new Error("Symbol keys are not portable oracle snapshots");
    ancestors.add(input);
    if (Array.isArray(input)) {
      if (Object.getOwnPropertyNames(input).length !== input.length + 1)
        throw new Error(
          "Sparse or extended arrays are not portable oracle snapshots",
        );
      const items = Array.from({ length: input.length }, (_, index) => {
        const descriptor = Object.getOwnPropertyDescriptor(
          input,
          String(index),
        );
        if (!descriptor || !("value" in descriptor))
          throw new Error("Sparse array or accessor in oracle snapshot");
        return visit(descriptor.value, depth + 1);
      });
      ancestors.delete(input);
      return { type: "array", value: items };
    }
    if (Object.getPrototypeOf(input) !== Object.prototype)
      throw new Error("Oracle snapshots require plain objects");
    const descriptors = Object.getOwnPropertyDescriptors(input);
    const fields: Record<string, JsonValue> = {};
    for (const key of Object.keys(descriptors)) {
      const descriptor = descriptors[key];
      if (!descriptor || !("value" in descriptor))
        throw new Error("Object accessor in oracle snapshot");
      if (!descriptor.enumerable)
        throw new Error("Nonenumerable oracle snapshot property");
      const child = visit(descriptor.value, depth + 1);
      if (key in fields)
        Object.defineProperty(fields, key, {
          value: child,
          enumerable: true,
          writable: true,
          configurable: true,
        });
      else fields[key] = child;
    }
    ancestors.delete(input);
    return { type: "object", value: fields };
  };
  return visit(value, 0);
};
