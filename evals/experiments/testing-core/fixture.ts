import type {
  TestingGenerator,
  TestingPlan,
} from "../../../packages/idealis/src/testing.js";
export const generator = (): TestingGenerator => {
  const add = [
    { type: "add", id: "a", amount: 1 },
    { type: "add", id: "b", amount: 2 },
  ];
  return {
    id: "durable-counter",
    version: "1",
    initial: ["running"],
    states: {
      running: [
        ...add.map((action) => ({ action, target: "running" })),
        { action: { type: "crash" }, target: "stopped" },
        { action: { type: "restart" }, target: "running" },
      ],
      stopped: [
        { action: { type: "crash" }, target: "stopped" },
        { action: { type: "restart" }, target: "running" },
      ],
    },
    minLength: 0,
    maxLength: 24,
  };
};
export const plan = (seed: number): TestingPlan => ({
  generator: generator(),
  seed,
  cases: 16,
  maxAttempts: 64,
  maxShrinkChecks: 128,
  maxWork: 100_000,
});
