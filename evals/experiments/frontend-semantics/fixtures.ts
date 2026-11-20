import { readFile, writeFile } from "node:fs/promises";
import { benchmarkCases, levelCases } from "./cases.js";
import { generatedCases } from "./generated.js";
import { lazyCases, temporalCases } from "./lazy-cases.js";
import { valueCases } from "./value-cases.js";

interface Plan {
  readonly developmentSeed: number;
  readonly confirmationSeed: number;
  readonly casesPerSeed: number;
}
const plan: Plan = JSON.parse(
  await readFile("evals/plans/0072-frontend-properties-2.json", "utf8"),
);
await writeFile(
  "crates/rtla-compiler/tests/frontend-semantics.json",
  `${JSON.stringify(
    [
      ...levelCases,
      ...lazyCases,
      ...valueCases,
      ...benchmarkCases,
      ...temporalCases,
      ...generatedCases(plan.developmentSeed, plan.casesPerSeed),
      ...generatedCases(plan.confirmationSeed, plan.casesPerSeed),
    ],
    null,
    2,
  )}\n`,
);

await writeFile(
  "crates/rtla-compiler/tests/frontend-semantics-bench.json",
  `${JSON.stringify(benchmarkCases, null, 2)}\n`,
);
