import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import {
  measureSize,
  sizeCounts,
  sizeSurfaces,
  type SizeSurface,
} from "./sizes.js";

const [output] = process.argv.slice(2);
assert.ok(output, "output");
const recorded: Record<string, Record<SizeSurface, unknown>> = {};
for (const terms of sizeCounts) {
  const cells = {} as Record<SizeSurface, unknown>;
  for (const surface of sizeSurfaces) {
    const measured = await measureSize(surface, terms);
    console.log(
      JSON.stringify({
        terms,
        surface,
        elapsedMs: Math.round(measured.elapsedMs),
        peakRssBytes: measured.run.peakRssBytes,
      }),
    );
    cells[surface] = measured.outcome;
  }
  recorded[String(terms)] = cells;
}
await writeFile(output, `${JSON.stringify(recorded, null, 2)}\n`, {
  flag: "wx",
});
