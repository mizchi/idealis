#!/usr/bin/env python3
"""Copy the evaluation outputs from the scratch directory into evals/runs/0120-audit.

usage: S=<scratch> W=<worktree> assemble-audit.py
Text files get the placeholders of $S/replace.tsv. Large dumps are not copied: reduced.json lists their number, bytes and sha256.
Files it writes are overwritten when the script runs again; README.md is never touched. Raw logs, per-point sweep and stack outputs and per-case Go stdout lines are not written into the run: they go to target/0120-audit-raw/ (drivers live under evals/experiments/call-depth/audit-0120/). reduced.json is read first and only the sections it does not hold yet are added, so records written by later rounds stay."""
import hashlib, json, os, re, shutil, sys

S, W = os.environ["S"], os.environ["W"]
T = f"{W}/evals/runs/0120-audit"
rules = []
for line in open(f"{S}/replace.tsv"):
    a, b = line.rstrip("\n").split("\t")
    rules.append((a, b))
rules += [("<home>/.local/share/mise", "<mise>")]
rules.sort(key=lambda r: -len(r[0]))

def clean(text):
    for a, b in rules:
        text = text.replace(a, b)
    return text

def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()

HAND_KEPT = re.compile(r"^(faults/(summary\.md|.*\.(compiled|failed|result)\.txt|pub-.*\.tsv)|oracle/baseline-.*\.json|parallel/(before|after|oracle|oracle-after)\.txt|tlc/results\.json)$")

def put(src, dst, tail=None):
    if HAND_KEPT.match(dst):
        return
    os.makedirs(os.path.dirname(f"{T}/{dst}"), exist_ok=True)
    text = open(src, errors="replace").read()
    if tail is not None:
        rows = text.split("\n")
        if len(rows) > tail + 20:
            text = f"[{len(rows) - tail} earlier lines removed]\n" + "\n".join(rows[-tail:])
    open(f"{T}/{dst}", "w").write(clean(text))

def reduce(entry, files, note):
    total = sum(os.path.getsize(f) for f in files)
    digest = hashlib.sha256("".join(f"{sha(f)}  {os.path.relpath(f, S)}\n" for f in sorted(files)).encode()).hexdigest()
    entry[note] = {"files": len(files), "bytes": total, "sha256OfSortedFileDigests": digest, "fileDigests": {os.path.relpath(f, S): sha(f) for f in sorted(files)}}

RAW = f"{W}/target/0120-audit-raw"
reduced_path = f"{T}/reduced.json"
reduced = json.load(open(reduced_path)) if os.path.exists(reduced_path) else {}

def raw_copy(src, dst):
    os.makedirs(os.path.dirname(f"{RAW}/{dst}"), exist_ok=True)
    shutil.copyfile(src, f"{RAW}/{dst}")

put(f"{S}/commands.tsv", "commands.tsv")
# patches
shutil.copy(f"{S}/oracle-0120.patch", f"{W}/evals/experiments/call-depth/oracle-0120.patch")
shutil.copy(f"{S}/oracle-0120-after.patch", f"{W}/evals/experiments/call-depth/oracle-0120-after.patch")
# checks
for side in ["before", "after", "oracle", "oracle-after"]:
    put(f"{S}/snapshots/{side}/snapshot.sha256", f"checks/snapshot-{side}.sha256")
for name in ["protected-before.txt", "protected-after-start.txt", "protected-after-end.txt", "tools.txt", "code-digests.txt", "frozen-final.sha256"]:
    put(f"{S}/checks/{name}", f"checks/{name}")
put(f"{S}/runs/tlc-analysis/cited.txt", "tlc/cited.txt")
# oracle baselines (small) and the sweeps
for name in ["baseline-before-0120.json", "baseline-oracle-0120-r2.json", "baseline-oracle-after-0120.json"]:
    put(f"{S}/runs/oracle/{name}", f"oracle/{name}")
put(f"{S}/runs/sweep-analysis/boundaries-255.json", "oracle/boundaries-255.json")
put(f"{S}/runs/sweep-analysis/sweep-analysis.txt", "oracle/sweep-analysis.txt")
put(f"{S}/runs/paths-analysis-r10/declared-depths.txt", "oracle/declared-depths.txt")
sweep = {}
for side in ["before", "oracle", "after", "oracle-after"]:
    data = json.load(open(f"{S}/runs/sweep/{side}.json"))
    rows = ["cell\toutcome\tsignal"]
    for c in data["cells"]:
        rows.append(f"{c['cell']}\t{c['stdout']}\t{c['signal'] or ''}{'' if c['exit'] == 0 else ' exit=' + str(c['exit'])}")
    os.makedirs(f"{RAW}/sweep", exist_ok=True)
    open(f"{RAW}/sweep/{side}.tsv", "w").write(clean("\n".join(rows) + "\n"))
reduce(sweep, [f"{S}/runs/sweep/{s}.json" for s in ["before", "oracle", "after", "oracle-after"]], "sweepDumps")
sweep["note"] = "The harness line of every cell is under target/0120-audit-raw/sweep/<side>.tsv; the JSON dumps hold the same lines with timings and are not copied."
reduced.setdefault("sweep", sweep)
# paths
for name in ["analysis.txt", "differences.tsv"]:
    put(f"{S}/runs/paths-analysis-r10/{name}", f"paths/{name}")
paths = {}
reduce(paths, [f"{S}/runs/paths-r8/{s}.json" for s in ["before", "oracle", "after", "oracle-after"]], "finalDumps")
reduce(paths, [f"{S}/runs/paths/{s}.json" for s in ["before", "oracle", "after", "oracle-after"]], "r5Dumps")
reduce(paths, [f"{S}/runs/paths-r6/{s}.json" for s in ["before", "oracle", "after", "oracle-after"]], "r6Dumps")
reduce(paths, [f"{S}/runs/{d}/{f}" for d in ["paths-r1", "paths-r2", "paths-r3", "paths-r4"] for f in os.listdir(f"{S}/runs/{d}")], "supersededDumps")
paths["note"] = "Per cell: normalized outcome, error code, result summary, canonical sha256 of the raw response without the probe field, violation trace, probe, IR sha256 and normalized IR sha256."
reduced.setdefault("paths", paths)
# tlc
put(f"{S}/runs/tlc-analysis-r12/analysis.txt", "tlc/analysis.txt")
put(f"{S}/runs/tlc-analysis-r12/verdicts.tsv", "tlc/verdicts.tsv")
put(f"{S}/runs/tlc-analysis-ctl-r12/analysis.txt", "tlc/control-bounded-states.txt")
tl = {}
reduce(tl, [os.path.join(dp, f) for dp, _, fs in os.walk(f"{S}/runs/tlc/tlc") for f in fs if f != "results.json"], "perCaseDirectories")
reduce(tl, [os.path.join(dp, f) for dp, _, fs in os.walk(f"{S}/runs/tlc/tlc-r1") for f in fs] + [os.path.join(dp, f) for dp, _, fs in os.walk(f"{S}/runs/tlc/tlc-r2") for f in fs], "supersededRuns")
reduced.setdefault("tlc", tl)
# parallel
put(f"{S}/runs/parallel/summary.txt", "parallel/summary.txt")
for side in ["before", "oracle", "after", "oracle-after"]:
    put(f"{S}/runs/parallel/{side}.txt", f"parallel/{side}.txt")
# stack, wasm, cli, go
put(f"{S}/runs/stack/summary.md", "stack/summary.md")
for side in ["before", "after", "oracle-after"]:
    for prof in ["release", "debug"]:
        raw_copy(f"{S}/runs/stack/{side}-{prof}.json", f"stack/{side}-{prof}.json")
put(f"{S}/runs/wasm/summary.md", "wasm/summary.md")
for name in sorted(os.listdir(f"{S}/runs/wasm")):
    if name.endswith(".json"):
        raw_copy(f"{S}/runs/wasm/{name}", f"wasm/{name}")
put(f"{S}/runs/cli/after.json", "cli/after.json")
def without_lines(node):
    if isinstance(node, list):
        return [without_lines(item) for item in node]
    if isinstance(node, dict):
        return {k: without_lines(v) for k, v in node.items() if k not in ("line", "largestLine", "failingLine")}
    return node

for name in sorted(os.listdir(f"{S}/runs/go")):
    raw_copy(f"{S}/runs/go/{name}", f"go/{name}")
    os.makedirs(f"{T}/go", exist_ok=True)
    open(f"{T}/go/{name}", "w").write(clean(json.dumps(without_lines(json.load(open(f"{S}/runs/go/{name}"))), indent=1)) + "\n")
goLines = {}
reduce(goLines, [f"{S}/runs/go/{name}" for name in sorted(os.listdir(f"{S}/runs/go"))], "removedStdoutLines")
reduced.setdefault("go", goLines)
# safety
put(f"{S}/runs/safety/result.json", "safety/result.json")
safety = {}
reduce(safety, [f"{S}/runs/safety/manifest.json", f"{S}/runs/safety/benchmark.json"], "removed")
reduced.setdefault("safety", safety)
# faults
for name in sorted(os.listdir(f"{S}/runs/faults")):
    put(f"{S}/runs/faults/{name}", f"faults/{name}")
# corrections of the stage T freeze
for name in ["correction-0001-record-primed-248.md", "correction-0002-records-oracle-columns.md", "frozen.sha256", "frozen.sha256.before-correction", "frozen.sha256.before-correction-0002"]:
    put(f"{W}/target/0120-stage-t/{name}", f"corrections/{name}")
open(f"{T}/reduced.json", "w").write(clean(json.dumps(reduced, indent=1)) + "\n")
print("assembled")
