import type { FrontendCase } from "./cases.js";

export const generatedCases = (
  seed: number,
  count: number,
): readonly FrontendCase[] =>
  Array.from({ length: count }, (_, index) => {
    const value =
      (Math.imul(seed, 1664525) + Math.imul(index + 1, 1013904223)) >>> 0;
    const width = 2 + ((value >>> 8) % 5);
    const depth = 1 + ((value >>> 16) % 4);
    return [false, true].map((fault): FrontendCase => ({
      name: `generated-${seed}-${index}-${fault ? "fault" : "normal"}`,
      family: "template",
      expected: fault ? "invariant-violation" : "complete",
      irVersion: 2,
      source: `---- MODULE Differential ----\nEXTENDS Naturals\nVARIABLE x\nA0(v) == x'=v\n${Array.from({ length: depth }, (_, inner) => `A${inner + 1}(v) == A${inner}(v) \\/ A${inner}(v)`).join("\n")}\nT(limit) == []<>(x<limit)\nLive == T(${width}) /\\ T(${width})\nInit == x=0\nNext == A${depth}((x+1)%${width})\nSafe == x<${width - Number(fault)}\n====`,
      config:
        "INIT Init\nNEXT Next\nINVARIANT Safe\nPROPERTY Live\nCHECK_DEADLOCK FALSE",
    }));
  }).flat();
