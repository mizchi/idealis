import assert from "node:assert/strict";
import { isRecord } from "../../../packages/idealis/src/trace.js";

export const expandTemporalTemplates = (input: unknown): unknown => {
  if (
    !isRecord(input) ||
    input.version !== 2 ||
    !Array.isArray(input.definitions)
  )
    return input;
  const definitions = input.definitions.map((value, id) => {
    assert.ok(isRecord(value));
    assert.equal(value.id, id);
    return value;
  });
  const retained = definitions.filter(
    (definition) => definition.bodyKind !== "temporal",
  );
  const ids = new Map(retained.map((definition, id) => [definition.id, id]));
  const expand = (
    value: unknown,
    active: ReadonlySet<number>,
    depth: number,
  ): unknown => {
    assert.ok(depth <= 256, "test normalization depth bound");
    if (Array.isArray(value))
      return value.map((child) => expand(child, active, depth + 1));
    if (!isRecord(value)) return value;
    if (value.op === "call" || value.kind === "call") {
      assert.equal(typeof value.definition, "number");
      const id = Number(value.definition);
      const definition = definitions[id];
      assert.ok(definition, "call definition exists");
      if (definition.bodyKind === "temporal") {
        assert.equal(value.kind, "call");
        assert.notEqual(
          definition.recursive,
          true,
          "temporal template is acyclic",
        );
        assert.ok(!active.has(id), "temporal template cycle");
        assert.ok(
          Array.isArray(value.arguments) && Array.isArray(value.captures),
        );
        assert.equal(value.arguments.length, definition.parameterCount);
        assert.equal(value.captures.length, definition.captureCount);
        return {
          kind: "frame",
          arguments: expand(value.arguments, active, depth + 1),
          captures: expand(value.captures, active, depth + 1),
          parameterCount: definition.parameterCount,
          captureCount: definition.captureCount,
          localScope: definition.localScope,
          formula: expand(definition.body, new Set([...active, id]), depth + 1),
        };
      }
      assert.ok(ids.has(id));
      return {
        ...Object.fromEntries(
          Object.entries(value).map(([name, child]) => [
            name,
            expand(child, active, depth + 1),
          ]),
        ),
        definition: ids.get(id),
      };
    }
    return Object.fromEntries(
      Object.entries(value).map(([name, child]) => [
        name,
        expand(child, active, depth + 1),
      ]),
    );
  };
  const root = expand({ ...input, definitions: [] }, new Set(), 0);
  assert.ok(isRecord(root));
  return {
    ...root,
    definitions: retained.map((definition) => ({
      ...(expand(definition, new Set(), 0) as Record<string, unknown>),
      id: ids.get(definition.id),
    })),
  };
};
