import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const { TLC_JAVA, TLC_JAR, TLC_APALACHE } = process.env;
const fixtures = "fixtures/tlc-apalache-folds";
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
if (digest(readFileSync(TLC_JAR)) !== "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88")
  throw new Error("TLC jar does not match the pinned SHA-256");
if (digest(readFileSync(TLC_APALACHE)) !== "a9115b404e72c58fd085f81bc59cc918f8adac49feabfb57f8e285bc7ab5cfac")
  throw new Error("Apalache.tla does not match the pinned SHA-256");

const { foldIndependentControls } = await import(
  "../../../dist/evals/experiments/tlc-differential/corpus.js"
);
const refusals = {
  "fold-independent-union-error": "records requires TLC field-name intern IDs",
  "fold-independent-union-nonset": "strings requires TLC intern IDs",
  "fold-dependent-last": "records requires TLC field-name intern IDs",
  "fold-dependent-reads-acc": "strings requires TLC intern IDs",
  "fold-dependent-if-leaf": "strings requires TLC intern IDs",
  "fold-dependent-sum": "records requires TLC field-name intern IDs",
};
const refusalText = {
  strings:
    "Unsupported TLA input: set fold order for strings requires TLC intern IDs absent from the IR",
  records:
    "Unsupported TLA input: set fold order for records requires TLC field-name intern IDs absent from the IR",
};

const scrub = (text, replacements) => {
  let result = text;
  for (const [from, to] of replacements) result = result.split(from).join(to);
  return result
    .replace(/\[pid: \d+\]/g, "[pid: <pid>]")
    .replace(/\bon \d+ cores\b/g, "on <cores> cores")
    .replace(/\([^\n]*, (MSBDiskFPSet, DiskStateQueue)\)/g, "(<host>, $1)")
    .replace(/\b\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\b/g, "<time>");
};

const parseValue = (text) => {
  let position = 0;
  const skip = () => {
    while (text[position] === " " || text[position] === "\n") position += 1;
  };
  const list = (close, item) => {
    const items = [];
    skip();
    if (text.startsWith(close, position)) {
      position += close.length;
      return items;
    }
    for (;;) {
      items.push(item());
      skip();
      if (text.startsWith(close, position)) {
        position += close.length;
        return items;
      }
      if (text[position] !== ",") throw new Error(`Bad value at ${position}: ${text}`);
      position += 1;
      skip();
    }
  };
  const value = () => {
    skip();
    if (text.startsWith("<<", position)) {
      position += 2;
      return { type: "list", value: list(">>", value) };
    }
    if (text[position] === "{") {
      position += 1;
      return normalize({ type: "set", value: list("}", value) });
    }
    if (text[position] === "[") {
      position += 1;
      const fields = list("]", () => {
        const match = /^([A-Za-z_][A-Za-z0-9_]*) \|-> /.exec(text.slice(position));
        if (!match) throw new Error(`Bad record at ${position}: ${text}`);
        position += match[0].length;
        return [match[1], value()];
      });
      return normalize({ type: "record", value: fields });
    }
    if (text[position] === '"') {
      const end = text.indexOf('"', position + 1);
      const result = { type: "string", value: text.slice(position + 1, end) };
      position = end + 1;
      return result;
    }
    const match = /^(TRUE|FALSE|-?\d+)/.exec(text.slice(position));
    if (!match) throw new Error(`Bad value at ${position}: ${text}`);
    position += match[0].length;
    if (match[0] === "TRUE" || match[0] === "FALSE")
      return { type: "bool", value: match[0] === "TRUE" };
    return { type: "int", value: match[0] };
  };
  const result = value();
  skip();
  if (position !== text.length) throw new Error(`Trailing text: ${text.slice(position)}`);
  return result;
};
const normalize = (value) => {
  if (value.type === "set") {
    const entries = new Map(value.value.map((item) => [JSON.stringify(item), item]));
    return { type: "set", value: [...entries.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)).map(([, item]) => item) };
  }
  if (value.type === "record")
    return { type: "record", value: [...value.value].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)) };
  return value;
};

const expected = JSON.parse(readFileSync(`${fixtures}/expected.json`, "utf8"));
const existing = new Set(expected.cases.map((entry) => entry.name));
const added = [];
for (const input of foldIndependentControls()) {
  if (existing.has(input.name)) throw new Error(`${input.name} is already captured`);
  const caseDirectory = mkdtempSync(join(tmpdir(), "tlc-case-"));
  const temporaryDirectory = mkdtempSync(join(tmpdir(), "tlc-tmp-"));
  writeFileSync(join(caseDirectory, "Differential.tla"), input.source);
  writeFileSync(join(caseDirectory, "Differential.cfg"), input.config);
  copyFileSync(TLC_APALACHE, join(caseDirectory, "Apalache.tla"));
  const run = spawnSync(
    TLC_JAVA,
    [
      "-Xmx256m",
      `-Djava.io.tmpdir=${temporaryDirectory}`,
      "-cp",
      TLC_JAR,
      "tlc2.TLC",
      "-workers",
      "1",
      "-nowarning",
      "Differential.tla",
    ],
    { cwd: caseDirectory, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
  const raw = `${run.stdout}${run.stderr}`;
  const replacements = [
    [realpathSync(temporaryDirectory), "<tlc-tmp>"],
    [temporaryDirectory, "<tlc-tmp>"],
    [realpathSync(caseDirectory), "<capture>"],
    [caseDirectory, "<capture>"],
  ];
  const log = scrub(raw, replacements);
  rmSync(caseDirectory, { recursive: true });
  rmSync(temporaryDirectory, { recursive: true });
  const logName = `${input.name}.log`;
  writeFileSync(`${fixtures}/${logName}`, log);
  const entry = {
    name: input.name,
    sourceSha256: digest(input.source),
    configSha256: digest(input.config),
    log: logName,
    logSha256: digest(log),
    exit: run.status,
  };
  if (run.status === 12) {
    entry.status = "invariant-violation";
    const initial = /violated by the initial state:\nx = ([^\n]*(?:\n(?!\n)[^\n]*)*)\n\n/.exec(log);
    if (initial) entry.values = [parseValue(initial[1].replace(/\n/g, " "))];
    else {
      const states = [...log.matchAll(/^State \d+: [^\n]*\nx = ([^\n]*)\n/gm)];
      if (states.length === 0) throw new Error(`${input.name}: no state in log`);
      entry.values = [parseValue(states[0][1])];
      entry.trace = states.map((state) => [parseValue(state[1])]);
    }
  } else if (run.status === 0) {
    entry.status = "success";
    const counts = /(\d+) states generated, (\d+) distinct states found/.exec(log);
    entry.states = Number(counts[2]);
    entry.generated = Number(counts[1]);
  } else if (run.status === 75) {
    entry.status = "error";
  } else throw new Error(`${input.name}: unexpected exit ${run.status}\n${log}`);
  if (refusals[input.name])
    entry.unsupported = refusals[input.name].startsWith("strings")
      ? refusalText.strings
      : refusalText.records;
  added.push(entry);
  console.log(`${input.name}: exit ${run.status} ${entry.status}`);
}
const nowAccepted = new Set([
  "fold-unsupported-string-direct",
  "fold-unsupported-string-tuple",
  "fold-unsupported-string-set",
  "fold-unsupported-record-direct",
  "fold-unsupported-record-tuple",
  "fold-unsupported-record-set",
  "fold-set-functions-with-records",
  "fold-set-wide-single-function",
  "fold-set-boolean-function-keys",
]);
for (const entry of expected.cases) if (nowAccepted.has(entry.name)) delete entry.unsupported;
expected.cases.push(...added);
writeFileSync(`${fixtures}/expected.json`, `${JSON.stringify(expected, null, 2)}\n`);
