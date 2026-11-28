import { copyFile, mkdir } from "node:fs/promises";
import { join } from "node:path";

const assets = "packages/idealis/assets/model-patterns";
const examples = "examples/model-patterns";
for (const [name, files] of [
  ["browser", ["model.project.json", "routes.ideal", "history.ideal"]],
  ["node-event-loop", ["io-batch.ideal", "next-tick.ideal"]],
  ["object-store", ["model.project.json", "values.ideal", "single-key.ideal"]],
] as const) {
  const directory = join(assets, name);
  await mkdir(directory, { recursive: true });
  for (const file of files) {
    await copyFile(join(examples, name, file), join(directory, file));
  }
}
