import { copyFile, mkdir } from "node:fs/promises";
import { join } from "node:path";

const assets = "packages/idealis/assets/rmodel-patterns";
const examples = "examples/model-patterns";
for (const [name, files] of [
  ["browser", ["project.rmodel.json", "routes.model", "history.model"]],
  ["node-event-loop", ["io-batch.model", "next-tick.model"]],
  ["object-store", ["project.rmodel.json", "values.model", "single-key.model"]],
] as const) {
  const directory = join(assets, name);
  await mkdir(directory, { recursive: true });
  for (const file of files) {
    await copyFile(join(examples, name, file), join(directory, file));
  }
}
