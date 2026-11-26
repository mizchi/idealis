import subprocess, sys, re, json
harness = sys.argv[1]
cap = int(sys.argv[4]) if len(sys.argv) > 4 else 511
def run(kind, size):
    out = subprocess.run([harness, "kind", kind, str(size), "1048576"], capture_output=True, text=True, timeout=600).stdout.strip()
    m = re.search(r"peak=(\d+)", out)
    return out, int(m.group(1)) if m else None
result = {}
for kind in sys.argv[2].split(","):
    lo, hi = 1, 1500
    while hi - lo > 1:
        mid = (lo + hi) // 2
        out, peak = run(kind, mid)
        if peak is not None and peak <= cap: lo = mid
        else: hi = mid
    a, pa = run(kind, lo); b, pb = run(kind, lo + 1)
    result[kind] = {"accepted": lo, "acceptedPeak": pa, "acceptedOutcome": a, "rejected": lo + 1, "rejectedPeak": pb, "rejectedOutcome": b}
    print(kind, lo, pa, "|", lo + 1, pb, "|", a.split("work=")[-1] if "work=" in a else a, flush=True)
json.dump(result, open(sys.argv[3], "x"), indent=1)
