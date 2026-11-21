# usage: stack-bisect.py <binary> <stage> <families> <ns> <version> <out.json>
# Smallest thread stack (KiB, resolution 4) at which `<binary> <stage> <family> <n> <kib> <version>` prints exactly "<stage> ok".
import subprocess, sys, json
binary, stage, families, ns, version, out = sys.argv[1:7]
families = families.split(','); ns = [int(x) for x in ns.split(',')]
def run(family, n, kib):
    r = subprocess.run([binary, stage, family, str(n), str(kib), version], capture_output=True, text=True, timeout=180)
    return r.returncode, r.stdout.strip()
res = {}
for family in families:
    row = {}
    for n in ns:
        code, text = run(family, n, 1 << 20)
        if not (code == 0 and text == f'{stage} ok'):
            row[str(n)] = {'status': 'refused-or-failed at 1 GiB', 'exit': code, 'stdout': text[:160]}
            continue
        lo, hi = 4, 1 << 20
        while hi - lo > 4:
            mid = (lo + hi) // 2
            code, text = run(family, n, mid)
            if code == 0 and text == f'{stage} ok': hi = mid
            else: lo = mid
        row[str(n)] = {'status': 'ok', 'minKiB': hi}
    res[family] = row
    print(stage, family, version, {k: (v.get('minKiB') or v['status']) for k, v in row.items()}, flush=True)
json.dump(res, open(out, 'w'), indent=1)
