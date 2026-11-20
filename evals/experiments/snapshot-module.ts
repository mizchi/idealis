import { existsSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

// Select the layout of the saved run, including runs recorded before experiments moved.
export const snapshotModule = (
  root: string,
  output: string,
  module: string,
): string => {
  const evals = join(root, output, "evals");
  const experiments = join(evals, "experiments");
  return pathToFileURL(
    join(existsSync(experiments) ? experiments : evals, module),
  ).href;
};
