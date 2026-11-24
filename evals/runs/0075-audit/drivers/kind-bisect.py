# usage: kind-bisect.py <binary> <kind:size,...> <out.json>: smallest thread stack (KiB, resolution 4) whose outcome equals the 1 GiB outcome
import subprocess, sys, json
binary, pairs, out = sys.argv[1], sys.argv[2].split(','), sys.argv[3]
def run(kind, size, kib):
    r = subprocess.run([binary, 'kind', kind, size, str(kib)], capture_output=True, text=True, timeout=600)
    return r.returncode, r.stdout.strip()
res = {}
for pair in pairs:
    kind, size = pair.split(':')
    code, reference = run(kind, size, 1 << 20)
    if code != 0:
        res[pair] = {'status': 'failed at 1 GiB', 'exit': code}
        continue
    lo, hi = 4, 1 << 20
    while hi - lo > 4:
        mid = (lo + hi) // 2
        c, text = run(kind, size, mid)
        if c == 0 and text == reference: hi = mid
        else: lo = mid
    res[pair] = {'outcome': reference, 'minKiB': hi}
    print(pair, hi, reference, flush=True)
json.dump(res, open(out, 'w'), indent=1)
