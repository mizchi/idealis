#!/usr/bin/env python3
"""usage: compare-pub.py <dir with pub-<side>.tsv> <out.txt>

Each pub file holds one line per `check()` call of the rtlc unit tests (tab separated: PUB, test thread, call index,
probe summary, response). Compares the published256 of the observation copies with the real before / after response of
the same call (same test, same index)."""
import collections, os, sys

directory, out = sys.argv[1:3]

def load(side):
    rows = {}
    for line in open(os.path.join(directory, f"pub-{side}.tsv")):
        parts = line.rstrip("\n").split("\t")
        if len(parts) != 5 or parts[0] != "PUB":
            continue
        rows[(parts[1], int(parts[2]))] = (parts[3], parts[4])
    return rows

lines = []
bad = 0
for oracle, real_side in (("oracle", "before"), ("oracle-after", "after")):
    probes, real = load(oracle), load(real_side)
    common = [k for k in probes if k in real]
    counts = collections.Counter()
    for key in common:
        probe, own = probes[key]
        _, actual = real[key]
        if "published=own" in probe:
            kind = "own"
            ok = own == actual
        elif "published=hit:" in probe:
            kind = "hit"
            message = probe.split("published=hit:", 1)[1]
            ok = actual == f"err protocol: {message}"
        else:
            kind = "other"
            ok = False
        counts[(kind, ok)] += 1
        if not ok:
            bad += 1
            lines.append(f"MISMATCH {oracle} vs {real_side} {key}: probe {probe}; own {own}; real {actual}")
        if "Unsupported TLA input" in (actual if kind == "hit" else "") or ("published=hit:Unsupported TLA input" in probe):
            counts[("fold-diagnostic", ok)] += 1
    lines.insert(0, f"{oracle} vs {real_side}: calls recorded {len(probes)} / {len(real)}, compared {len(common)}; " + ", ".join(f"{k[0]} {'ok' if k[1] else 'FAIL'} {v}" for k, v in sorted(counts.items())))
lines.append(f"mismatches: {bad}")
open(out, "x").write("\n".join(lines) + "\n")
print("\n".join(lines))
