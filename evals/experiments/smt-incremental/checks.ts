import { generated, handCases } from "../smt-scripts/corpus.js";
import { sequences } from "./context.js";
import { verify } from "./verify.js";

export const checks = async (): Promise<void> => {
  await verify(
    {
      scripts: [...handCases(), ...generated(0x7401, 2)],
      contexts: sequences(0x7401, 6),
    },
    () => {},
  );
};
