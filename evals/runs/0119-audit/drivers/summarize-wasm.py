# usage: summarize-wasm.py <runs/wasm dir> <boundaries-255.json> <output.md>
# Reads <side>-node-<mode>.json and <side>-chromium.json for side oracle-r2 and after, plus the oracle depths JSON.
import json, sys, os
d, boundaries_path, out = sys.argv[1:4]
boundaries = json.load(open(boundaries_path))
expected = {k: v["accepted"] for k, v in boundaries.items()}
expected["hop"] = 83
expected["hop-source"] = 83
depths = json.load(open(os.path.join(d, "oracle-r2-depths.json")))
hosts = [("node default", "node-default"), ("node --liftoff-only", "node-liftoff-only"), ("node --no-liftoff", "node-no-liftoff"),
         ("chromium main", "chromium:main"), ("chromium worker", "chromium:worker")]
def load(side, key):
    if key.startswith("chromium"):
        return json.load(open(os.path.join(d, f"{side}-chromium.json")))[key.split(":")[1]]
    return json.load(open(os.path.join(d, f"{side}-{key}.json")))
lines = ["Oracle (limit 4,096): largest accepted size per cell and the evaluation depth it reaches; the minimum over cells per host.", "",
         "| host | minimum depth | cell | size | next failure |", "| --- | ---: | --- | ---: | --- |"]
oracle_min = {}
for label, key in hosts:
    cells = depths[f"{key}"]
    low = min(((c["depthAtLargest"], name, c) for name, c in cells.items() if c["failure"] and not c["failure"].startswith("error ")), key=lambda t: t[0])
    oracle_min[label] = low[0]
    lines.append(f"| {label} | {low[0]} | {low[1]} | {low[2]['largest']} | {low[2]['failure'][:60]} |")
lines += ["", "After (limit 256): largest accepted size per cell, compared with the size whose depth is 255.", "",
          "| host | cells | largest = expected | typed refusal next | untyped failure |", "| --- | ---: | ---: | ---: | ---: |"]
problems = []
for label, key in hosts:
    cells = load("after", key)
    same = typed = untyped = 0
    for c in cells:
        ok = c["largest"] == expected[c["cell"]]
        same += ok
        failure = c["failure"] or ""
        typed += failure.startswith("error protocol: Safety evaluation nesting exceeds 256")
        untyped += failure.startswith(("RuntimeError", "RangeError"))
        if not ok: problems.append((label, c["cell"], c["largest"], expected[c["cell"]], failure[:60]))
    lines.append(f"| {label} | {len(cells)} | {same} | {typed} | {untyped} |")
lines.append("")
lines.append(f"cells whose largest accepted size differs from the expected one: {len(problems)}")
lines += [f"- {p}" for p in problems]
open(out, "w").write("\n".join(lines) + "\n")
print("\n".join(lines))
