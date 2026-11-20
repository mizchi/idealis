import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { gunzipSync } from "node:zlib";
import { summarizeNative } from "../profile-tuning/hotspots.js";

const profilePath = process.argv[2];
const symbolsPath = process.argv[3];
const output = process.argv[4];
assert.ok(profilePath && symbolsPath && output);
const profile = JSON.parse(gunzipSync(await readFile(profilePath)).toString());
const symbols = JSON.parse(await readFile(symbolsPath, "utf8"));
const samples = summarizeNative(profile, symbols, "samples");
const cpu = summarizeNative(profile, symbols, "cpu");
await writeFile(output, JSON.stringify({ samples, cpu }, null, 2), {
  flag: "wx",
});
console.log(
  JSON.stringify({
    samples: samples.totalWeight,
    hotspots: samples.hotspots
      .filter(
        (row) => row.name.includes("rusmt") || row.name.includes("idealis_smt"),
      )
      .sort((left, right) => right.inclusive - left.inclusive)
      .slice(0, 12),
  }),
);
