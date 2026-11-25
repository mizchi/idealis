# usage: summarize-stack.py <dir with 256-<side>-<profile>.json> <output.md>
import json, sys
d, out = sys.argv[1], sys.argv[2]
data = {}
for side in ("before", "after", "oracle"):
    for prof in ("release", "debug"):
        data[(side, prof)] = {(m["family"], m["length"]): m for m in json.load(open(f"{d}/256-{side}-{prof}.json"))}
cells = list(data[("after", "release")])
def kib(m):
    if m is None: return "-"
    return "n/a" if m["minimumKibibytes"] is None else str(m["minimumKibibytes"])
def result(m):
    text = m["reference"]["stdout"].split(" peak=")[0]
    if "exceeds 256" in text: return "refused 256"
    if "exceeds 128" in text: return "refused 128"
    if "work=" in text: return "complete work " + text.split("work=")[1]
    return text.replace("hop ", "")
lines = ["| cell | after release | after debug | before release | before debug | oracle release | oracle debug | after result |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |"]
for c in cells:
    row = [kib(data[(s, p)].get(c)) for s in ("after", "before", "oracle") for p in ("release", "debug")]
    lines.append(f"| {c[0]}:{c[1]} | " + " | ".join(row) + f" | {result(data[('after', 'release')][c])} |")
cellsN = sum(len(v) for v in data.values())
measured = [m for k in data for m in data[k].values() if m["minimumKibibytes"] is not None]
overflow = sum(1 for m in measured if m["belowMinimum"] and m["belowMinimum"]["signal"] == "SIGABRT" and "overflowed its stack" in m["belowMinimum"]["stderr"])
lines.append("")
lines.append(f"{cellsN} cells; {len(measured)} have a minimum; one step (4 KiB) below the minimum, {overflow} of them aborted with an overflowed stack.")
open(out, "w").write("\n".join(lines) + "\n")
print("\n".join(lines[-3:]))
