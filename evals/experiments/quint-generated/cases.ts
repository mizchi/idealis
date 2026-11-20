export const names = [
  "Counter",
  "SetFold",
  "SequenceFold",
  "Choice",
  "Match",
  "NestedFold",
] as const;

export const bounds = {
  maxDepth: 64,
  maxStates: 10000,
  maxWork: 1000000,
};

export const config = (property: "safety" | "broken"): string =>
  `INIT q_init\nNEXT q_step\nINVARIANT ${property}\n`;
