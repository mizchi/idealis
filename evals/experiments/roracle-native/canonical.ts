import type { JsonValue } from "../../../packages/idealis/src/generated/roracle.js";

export const canonicalObject = (input: JsonValue) => {
  const ancestors = new Set<object>();
  let remaining = 1_000_000;
  const visit = (value: JsonValue, depth: number): void => {
    if (--remaining < 0 || depth > 128)
      throw new Error("Native JSON conversion budget exceeded");
    if (value === null || typeof value === "boolean") return;
    if (typeof value === "string") {
      if (/[\uD800-\uDFFF]/u.test(value))
        throw new Error("Native JSON requires Unicode scalar strings");
      return;
    }
    if (typeof value === "number") {
      if (!Number.isFinite(value) || Object.is(value, -0))
        throw new Error(
          "Native JSON requires canonical finite numbers; use snapshot for JS values",
        );
      return;
    }
    if (typeof value !== "object" || ancestors.has(value))
      throw new Error("Native JSON requires acyclic JSON data");
    if (Object.getOwnPropertySymbols(value).length)
      throw new Error("Native JSON rejects symbol keys");
    ancestors.add(value);
    if (Array.isArray(value)) {
      if (Object.getOwnPropertyNames(value).length !== value.length + 1)
        throw new Error("Native JSON rejects sparse or extended arrays");
      for (let index = 0; index < value.length; index++) {
        const property = Object.getOwnPropertyDescriptor(value, index);
        if (!property || !("value" in property))
          throw new Error("Native JSON rejects accessors");
        visit(property.value, depth + 1);
      }
    } else {
      if (Object.getPrototypeOf(value) !== Object.prototype)
        throw new Error("Native JSON requires plain objects");
      for (const name in value)
        if (!Object.hasOwn(value, name))
          throw new Error("Native JSON rejects inherited enumerable fields");
      for (const [name, property] of Object.entries(
        Object.getOwnPropertyDescriptors(value),
      )) {
        visit(name, depth + 1);
        if (!property.enumerable || !("value" in property))
          throw new Error("Native JSON rejects accessors and hidden fields");
        visit(property.value, depth + 1);
      }
    }
    ancestors.delete(value);
  };
  visit(input, 0);
};
