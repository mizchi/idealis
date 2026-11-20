import type {
  SafetyBounds,
  TlaSourceInput,
} from "../../../packages/idealis/src/contracts.js";

export interface CoverageInput extends TlaSourceInput {
  readonly name: string;
  readonly expected: string;
  readonly compareTlcCoverage: boolean;
}
export const coverageBounds: SafetyBounds = {
  maxDepth: 100,
  maxStates: 1000,
  maxWork: 1_000_000,
  maxTemporalNodes: 10000,
};
const source = (body: string) =>
  `---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\n${body}\n====`;
const config = "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE";
const counter =
  "Init == x = 0\nAdvance == x < 3 /\\ x' = x + 1\nStay == UNCHANGED x\nUnused == FALSE /\\ x' = 0\nNext == Advance \\/ Stay \\/ Unused";
export const coverageInputs: readonly CoverageInput[] = [
  {
    name: "counter",
    source: source(counter),
    config,
    expected: "complete",
    compareTlcCoverage: true,
  },
  {
    name: "duplicate-actions",
    source: source(
      "Init == x = 0\nFirst == x < 2 /\\ x' = x + 1\nSecond == x < 2 /\\ x' = x + 1\nNext == First \\/ Second",
    ),
    config,
    expected: "complete",
    compareTlcCoverage: true,
  },
  {
    name: "multiple-initial",
    source: source("Init == x = 0 \\/ x = 0 \\/ x = 1\nNext == UNCHANGED x"),
    config,
    expected: "complete",
    compareTlcCoverage: true,
  },
  {
    name: "rejected-initial",
    source: source("Init == x \\in 0..1 /\\ x > 0\nNext == UNCHANGED x"),
    config,
    expected: "complete",
    compareTlcCoverage: true,
  },
  {
    name: "choose",
    source: source("Init == x \\in 0..2\nNext == x' \\in 0..2"),
    config,
    expected: "complete",
    compareTlcCoverage: true,
  },
  {
    name: "empty",
    source: source("Init == x \\in {}\nNext == UNCHANGED x"),
    config,
    expected: "empty",
    compareTlcCoverage: true,
  },
  {
    name: "constraint",
    source: source(`${counter}\nLimit == x < 2`),
    config: `${config}\nCONSTRAINT Limit`,
    expected: "complete",
    compareTlcCoverage: true,
  },
  {
    name: "action-constraint",
    source: source(`${counter}\nLimit == x' < 2`),
    config: `${config}\nACTION_CONSTRAINT Limit`,
    expected: "complete",
    compareTlcCoverage: true,
  },
  {
    name: "invariant",
    source: source(`${counter}\nSafe == x < 2`),
    config: `${config}\nINVARIANT Safe`,
    expected: "invariant-violation",
    compareTlcCoverage: false,
  },
  {
    name: "deadlock",
    source: source("Init == x = 0\nNext == FALSE /\\ UNCHANGED x"),
    config: "INIT Init\nNEXT Next",
    expected: "deadlock",
    compareTlcCoverage: false,
  },
  {
    name: "action-property",
    source: source(`${counter}\nProgress == [][x' < 2]_x`),
    config: `${config}\nPROPERTY Progress`,
    expected: "property-violation",
    compareTlcCoverage: false,
  },
  {
    name: "temporal",
    source: source(`${counter}\nLive == <> (x = 9)`),
    config: `${config}\nPROPERTY Live`,
    expected: "liveness-violation",
    compareTlcCoverage: false,
  },
  {
    name: "fair",
    source: source(
      `${counter}\nSpec == Init /\\ [][Next]_x /\\ WF_x(Advance)\nLive == <> (x = 3)`,
    ),
    config: "SPECIFICATION Spec\nPROPERTY Live\nCHECK_DEADLOCK FALSE",
    expected: "complete",
    compareTlcCoverage: true,
  },
  {
    name: "error",
    source: source("Init == x = 1 \\div 0\nNext == UNCHANGED x"),
    config,
    expected: "error",
    compareTlcCoverage: false,
  },
];
