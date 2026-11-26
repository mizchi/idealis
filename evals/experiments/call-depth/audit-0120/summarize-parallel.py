#!/usr/bin/env python3
"""usage: summarize-parallel.py <dir with before.txt oracle.txt after.txt oracle-after.txt> <out.txt>

Each input holds the PROBE lines of the parallel probe test (20 parallel samples and one sequential run per cell).
Prints, per cell and side, the number of distinct results over the 20 samples, and checks the published256 of the
observation copies against the real before / after result (verdict, error kind and message, trace; work is not compared
when the request has a hit)."""
import collections, os, re, sys

directory, out = sys.argv[1:3]
sides = ["before", "oracle", "after", "oracle-after"]
cells = collections.OrderedDict()
for side in sides:
    for line in open(os.path.join(directory, f"{side}.txt")):
        m = re.search(r"PROBE (\S+) (parallel run=\d+|sequential) (ok|err) (.*)$", line)
        if not m:
            continue
        name, kind, status, rest = m.groups()
        probe = ""
        if " peak=" in rest:
            rest, probe = rest.split(" peak=", 1)
            probe = "peak=" + probe
        cells.setdefault((name, kind.split()[0]), {}).setdefault(side, collections.Counter())[(status, rest.strip(), probe)] += 1

def fields(status, rest):
    if status == "err":
        return ("err", rest)
    m = re.match(r"status=(\S+) reason=(.*?) states=(\d+) generated=(\d+) work=(\d+) trace=(.*)$", rest)
    return ("ok", m.group(1), m.group(2), m.group(3), m.group(4), m.group(6)) if m else ("ok", rest)

lines = []
problems = []
for (name, kind), by_side in cells.items():
    lines.append(f"{name} {kind}")
    for side in sides:
        counter = by_side[side]
        lines.append(f"  {side}: {sum(counter.values())} runs, {len(counter)} distinct")
        for (status, rest, probe), count in counter.most_common():
            lines.append(f"    {count} x {status} {rest} {probe}")
    before_real = list(by_side["before"])[0]
    after_real = list(by_side["after"])[0]
    for label, probed, real in (("oracle vs before", "oracle", before_real), ("oracle-after vs after", "oracle-after", after_real)):
        (status, rest, probe) = list(by_side[probed])[0]
        published = re.search(r"published=(\S.*)$", probe)
        published = published.group(1) if published else "own"
        real_f = fields(real[0], real[1])
        if published == "own":
            own_f = fields(status, rest)
            same = own_f == real_f if kind == "sequential" else (own_f[:2] == real_f[:2] and own_f[5:] == real_f[5:] and own_f[0] == real_f[0])
            lines.append(f"  {label}: published256 = own response; real {'matches' if same else 'DIFFERS'} (work compared only without a hit)")
            if not same:
                problems.append(f"{name} {kind} {label}")
        elif published.startswith("hit:"):
            same = real_f[0] == "err" and real_f[1].endswith(published[4:])
            lines.append(f"  {label}: published256 = {published}; real {'matches' if same else 'DIFFERS'}")
            if not same:
                problems.append(f"{name} {kind} {label}")
        elif published.startswith("cut:err "):
            same = real_f[0] == "err" and real_f[1].split("message=", 1)[-1] in published
            lines.append(f"  {label}: published256 = {published}; real {'matches' if same else 'DIFFERS'}")
            if not same:
                problems.append(f"{name} {kind} {label}")
        else:
            m = re.match(r"cut:ok status=(\S+) reason=(.*?) states=(\d+) generated=(\d+) work=(\d+) trace=(.*)$", published)
            real_m = re.match(r"status=(\S+) reason=(.*?) states=(\d+) generated=(\d+) work=(\d+) trace=(.*)$", real[1]) if real[0] == "ok" else None
            same = bool(m) and bool(real_m) and [m.group(i) for i in (1, 2, 3, 4, 6)] == [real_m.group(i) for i in (1, 2, 3, 4, 6)]
            lines.append(f"  {label}: published256 = {published}; real status, reason, states, generated and trace {'match' if same else 'DIFFER'} (work not compared)")
            if not same:
                problems.append(f"{name} {kind} {label}")
lines.append("")
distinct = [(n, k, s) for (n, k), by in cells.items() for s, c in by.items() if k == "parallel" and len(c) != 1]
lines.append(f"cells with more than one distinct result over the 20 parallel samples: {len(distinct)} {distinct}")
exhausted = [(n, k, s) for (n, k), by in cells.items() for s, c in by.items() for (st, rest, _) in c if "work=1000" in rest and "bounded" in rest]
lines.append(f"results that reached the work cap (bounded / work): {len(exhausted)}")
lines.append(f"published256 vs real mismatches: {len(problems)} {problems}")
open(out, "x").write("\n".join(lines) + "\n")
print("\n".join(lines[-4:]))
