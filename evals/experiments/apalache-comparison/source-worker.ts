import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { compileTlaSafety } from "../../../packages/idealis/src/tla-safety.js";

const input = JSON.parse(await readFile(process.argv[2] ?? "", "utf8"));
const program = compileTlaSafety(input);
process.stdout.write(
  execFileSync("crates/idealis-core/target/release/idealis-core", [], {
    input: `${JSON.stringify({ version: 1, request: { kind: "check-safety", program, bounds: input.bounds } })}\n`,
  }),
);
