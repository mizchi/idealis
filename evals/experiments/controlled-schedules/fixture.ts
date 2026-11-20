import type {
  TestingCase,
  TestingGenerator,
  TestingJson,
  TestingPlan,
  TestingSchedule,
} from "../../../packages/idealis/src/testing.js";

export const schedule = (actors: number): TestingSchedule => ({
  id: `increment-${actors}`,
  version: "1",
  actors: Array.from({ length: actors }, (_, index) => ({
    id: `actor-${index}`,
    operations: ["read", "write"],
  })),
});
export const plan = (
  generator: TestingGenerator,
  seed: number,
): TestingPlan => ({
  generator,
  seed,
  cases: 64,
  maxAttempts: 64,
  maxShrinkChecks: 128,
  maxWork: 1_000_000,
});
export const saved = (
  actors: number,
  actions: readonly TestingJson[],
): TestingCase => ({
  format: "idealis.test-case",
  version: 1,
  rng: "xorshift32-v1",
  generatorId: schedule(actors).id,
  generatorVersion: "1",
  seed: 0x3801,
  index: 0,
  actions,
});
export const completeOrders = (
  actors: number,
): readonly (readonly TestingJson[])[] => {
  const visit = (
    positions: readonly number[],
    prefix: readonly TestingJson[],
  ): readonly (readonly TestingJson[])[] =>
    positions.every((position) => position === 2)
      ? [prefix]
      : positions.flatMap((position, actor) =>
          position === 2
            ? []
            : visit(
                positions.map(
                  (value, index) => value + Number(index === actor),
                ),
                [
                  ...prefix,
                  {
                    actor: `actor-${actor}`,
                    step: position,
                    operation: position === 0 ? "read" : "write",
                  },
                ],
              ),
        );
  return visit(
    Array.from({ length: actors }, () => 0),
    [],
  );
};
export const graphOrders = (
  generator: TestingGenerator,
): readonly (readonly TestingJson[])[] => {
  const visit = (
    key: string,
    actions: readonly TestingJson[],
  ): readonly (readonly TestingJson[])[] => {
    const edges = generator.states[key];
    if (!edges || actions.length > generator.maxLength)
      throw new Error("Invalid schedule graph");
    return edges.length === 0
      ? [actions]
      : edges.flatMap((edge) => visit(edge.target, [...actions, edge.action]));
  };
  return generator.initial.flatMap((key) => visit(key, []));
};
