# usage: ceiling-depths.py <harness built with the oracle-peak feature> <name=ceilings.json,...> <output.json>
# Adds the evaluation depth (the oracle's record) at the largest accepted size and at the first failing size.
import json, re, subprocess, sys
harness, out = sys.argv[1], sys.argv[3]
def depth(cell, size):
    args = ["hop", str(size), "1048576"] if cell in ("hop", "hop-source") else ["kind", cell, str(size), "1048576"]
    text = subprocess.run([harness, *args], capture_output=True, text=True, timeout=900).stdout
    m = re.search(r"peak=(\d+)", text)
    return int(m.group(1)) if m else None
rows = {}
for pair in sys.argv[2].split(","):
    name, path = pair.split("=", 1)
    for ceiling in json.load(open(path)):
        if ceiling["largest"] is None: continue
        cell = ceiling["cell"]
        rows.setdefault(name, {})[cell] = {**ceiling, "depthAtLargest": depth(cell, ceiling["largest"]),
            "depthAtFirstFailing": depth(cell, ceiling["firstFailing"]) if ceiling["firstFailing"] else None}
json.dump(rows, open(out, "x"), indent=1)
for name, cells in rows.items():
    print(name)
    for cell, c in cells.items():
        print(f"  {cell:11s} size {c['largest']:5d} depth {c['depthAtLargest']} | next {c['firstFailing']} depth {c['depthAtFirstFailing']} | {(c['failure'] or '-')[:60]}")
