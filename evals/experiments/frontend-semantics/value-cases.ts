import type { FrontendCase } from "./cases.js";
import { lazyCases } from "./lazy-cases.js";

const properties = new Map([
  ["lazy-alias", "x=7"],
  ["lazy-actual", "x=7"],
  ["lazy-selected", "x=7"],
  ["lazy-record", "x=7"],
  ["lazy-tuple", "x=7"],
  ["lazy-domain", "x=(0..7)"],
  ["lazy-unused-loop", "x=7"],
  ["lazy-capture", "x=7"],
  ["lazy-old", "x=7"],
  ["lazy-pattern", "x=7"],
  ["lazy-product", "x=7"],
  ["lazy-prime", "x \\in 0..2"],
  [
    "lazy-materialize",
    "DOMAIN x=(0..3) /\\ x[0]=0 /\\ x[1]=1 /\\ x[2]=2 /\\ x[3]=3",
  ],
]);

export const valueCases: readonly FrontendCase[] = lazyCases.flatMap(
  (input) => {
    const property = properties.get(input.name);
    return property === undefined
      ? []
      : [
          {
            ...input,
            name: `${input.name}-value`,
            source: input.source.replace(
              "====",
              `ValueSafe == ${property}\n====`,
            ),
            config: `${input.config}\nINVARIANT ValueSafe`,
          },
        ];
  },
);
