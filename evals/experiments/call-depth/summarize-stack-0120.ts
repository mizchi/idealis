import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Measurement } from "./stack.js";

const [directory, output, acceptedJson] = process.argv.slice(2);
assert.ok(
  directory && output && acceptedJson,
  "usage: summarize-stack-0120.js <dir with <side>-<profile>.json> <out.md> <boundaries-255.json>",
);
const sides = ["after", "before", "oracle-after"] as const;
const profiles = ["release", "debug"] as const;
const data = new Map<string, Map<string, Measurement>>();
for (const side of sides)
  for (const profile of profiles) {
    const measurements = JSON.parse(
      readFileSync(join(directory, `${side}-${profile}.json`), "utf8"),
    ) as Measurement[];
    data.set(
      `${side}-${profile}`,
      new Map(measurements.map((m) => [`${m.family}:${m.length}`, m])),
    );
  }
const accepted = JSON.parse(readFileSync(acceptedJson, "utf8")) as Record<
  string,
  { accepted: number }
>;
const cells = [...(data.get("after-release")?.keys() ?? [])];
const pick = (key: string, cell: string): Measurement | undefined =>
  data.get(key)?.get(cell);
const kib = (measurement: Measurement | undefined): string =>
  measurement === undefined
    ? "-"
    : measurement.minimumKibibytes === null
      ? "n/a"
      : String(measurement.minimumKibibytes);
const strip = (text: string): string => text.split(" peak=")[0] ?? text;
const resultOf = (measurement: Measurement | undefined): string => {
  if (!measurement) return "-";
  const text = strip(measurement.reference.stdout);
  if (text.includes("exceeds 256")) return "refused 256";
  if (text.includes("exceeds 128")) return "refused 128";
  if (text.includes("work=")) return `complete work ${text.split("work=")[1]}`;
  return text.replace("hop ", "");
};
const lines = [
  "| cell | after release | after debug | before release | before debug | oracle-after release | oracle-after debug | after result |",
  "| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
];
for (const cell of cells)
  lines.push(
    `| ${cell} | ${sides
      .flatMap((side) =>
        profiles.map((profile) => kib(pick(`${side}-${profile}`, cell))),
      )
      .join(" | ")} | ${resultOf(pick("after-release", cell))} |`,
  );

const notes: string[] = [];
const acceptedCells = new Map<string, string>();
for (const [kind, boundary] of Object.entries(accepted)) {
  const cell = `${kind === "hop" ? "hop" : kind}:${boundary.accepted}`;
  if (cells.includes(cell)) acceptedCells.set(kind, cell);
}
for (const profile of profiles) {
  let worst = { kind: "", kib: 0 };
  const failures: string[] = [];
  for (const [kind, cell] of acceptedCells) {
    const measurement = pick(`after-${profile}`, cell);
    const value = measurement?.minimumKibibytes ?? null;
    const completes = resultOf(measurement).startsWith("complete");
    if (value === null || !completes)
      failures.push(`${cell} (${resultOf(measurement)})`);
    else if (value > worst.kib) worst = { kind, kib: value };
  }
  const limit = profile === "release" ? 1024 : 6144;
  notes.push(
    `after ${profile}: largest minimum stack at the accepted maximum is ${worst.kib} KiB (${worst.kind}); limit ${limit} KiB: ${worst.kib <= limit && failures.length === 0 ? "met" : "NOT met"}; accepted maximum cells that did not complete: ${failures.length}${failures.length ? ` (${failures.join(", ")})` : ""}.`,
  );
}
const everyCell = [...data.values()].flatMap((cellsOf) => [
  ...cellsOf.values(),
]);
const measured = everyCell.filter((m) => m.minimumKibibytes !== null);
const aborted = measured.filter(
  (m) =>
    m.belowMinimum?.signal === "SIGABRT" &&
    m.belowMinimum.stderr.includes("overflowed its stack"),
);
notes.push(
  `${everyCell.length} cells; ${measured.length} have a minimum; one step (4 KiB) below the minimum, ${aborted.length} of them aborted with an overflowed stack.`,
);
const atMinimumFailed = measured.filter(
  (m) => m.atMinimum === null || m.atMinimum.stdout !== m.reference.stdout,
);
notes.push(
  `cells whose output at the minimum differs from the reference output: ${atMinimumFailed.length}.`,
);
const afterVsOracle = cells.filter((cell) =>
  profiles.some(
    (profile) =>
      strip(pick(`after-${profile}`, cell)?.reference.stdout ?? "") !==
        strip(pick(`oracle-after-${profile}`, cell)?.reference.stdout ?? "") &&
      !(pick(`oracle-after-${profile}`, cell)?.reference.stdout ?? "").includes(
        "hit:",
      ),
  ),
);
notes.push(
  `cells where the after output differs from the oracle-after output without a recorded 256 hit: ${afterVsOracle.length}${afterVsOracle.length ? ` (${afterVsOracle.join(", ")})` : ""}.`,
);
const unchangedKinds = Object.keys(accepted).filter(
  (kind) =>
    ![
      "hop",
      "hop-invariant",
      "forward",
      "capture",
      "mixed",
      "forward-set",
      "forward-function",
      "record-hop",
      "let-hop",
      "set-hop",
      "function-hop",
      "primed-hop",
    ].includes(kind),
);
const changedMinimum: string[] = [];
for (const kind of unchangedKinds)
  for (const cell of cells.filter((c) => c.startsWith(`${kind}:`)))
    for (const profile of profiles) {
      const before = pick(`before-${profile}`, cell);
      const after = pick(`after-${profile}`, cell);
      if (kib(before) !== kib(after))
        changedMinimum.push(
          `${cell} ${profile}: before ${kib(before)} after ${kib(after)}`,
        );
    }
notes.push(
  `the 12 kinds without forwarding whose minimum stack differs between before and after: ${changedMinimum.length}.`,
);
for (const row of changedMinimum) notes.push(`  ${row}`);
writeFileSync(output, `${lines.join("\n")}\n\n${notes.join("\n")}\n`, {
  flag: "wx",
});
console.log(notes.join("\n"));
