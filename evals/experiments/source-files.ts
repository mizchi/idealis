import { readdir } from "node:fs/promises";
import { join } from "node:path";

// Both the experiment and its regression tests contribute to its source fingerprint.
export const typeScriptSources = async (...directories: string[]) =>
  (
    await Promise.all(
      directories.map(async (directory) =>
        (await readdir(directory))
          .filter((name) => name.endsWith(".ts"))
          .map((name) => join(directory, name)),
      ),
    )
  )
    .flat()
    .sort();
