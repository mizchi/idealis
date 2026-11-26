import json, subprocess, sys
harness, kinds, output = sys.argv[1], sys.argv[2].split(","), sys.argv[3]
def run(kind, size):
    return subprocess.run([harness, "kind", kind, str(size), "1048576"], capture_output=True, text=True, timeout=600).stdout.strip()
result = {}
for kind in kinds:
    lo, hi = 1, 1500
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if run(kind, mid).startswith("kind complete"): lo = mid
        else: hi = mid
    result[kind] = {"accepted": lo, "acceptedOutcome": run(kind, lo), "refused": lo + 1, "refusedOutcome": run(kind, lo + 1)}
    print(kind, lo, result[kind]["acceptedOutcome"].split("work=")[-1], "|", lo + 1, result[kind]["refusedOutcome"], flush=True)
json.dump(result, open(output, "x"), indent=1)
