# usage: chain-bisect.py <binary> <families> <ns> <version> <out.json>: smallest thread stack (KiB, resolution 4) at which the check outcome equals the 1 GiB outcome
import subprocess, sys, json
binary, families, ns, version, out = sys.argv[1:6]
def run(family, n, kib):
    r = subprocess.run([binary, 'chain', family, str(n), str(kib), version], capture_output=True, text=True, timeout=600)
    return r.returncode, r.stdout.strip()
res = {}
for family in families.split(','):
    row = {}
    for n in [int(x) for x in ns.split(',')]:
        code, reference = run(family, n, 1 << 20)
        if code != 0:
            row[str(n)] = {'status': 'failed at 1 GiB', 'exit': code}
            continue
        lo, hi = 4, 1 << 20
        while hi - lo > 4:
            mid = (lo + hi) // 2
            c, text = run(family, n, mid)
            if c == 0 and text == reference: hi = mid
            else: lo = mid
        row[str(n)] = {'outcome': reference, 'minKiB': hi}
    res[family] = row
    print(family, version, {k: (v.get('minKiB'), v.get('outcome', v.get('status'))) for k, v in row.items()}, flush=True)
json.dump(res, open(out, 'w'), indent=1)
