# usage: boundary.py <binary> <kib> <kinds>: largest size whose outcome equals size 1's status, and the next outcome
import subprocess, sys
binary, kib, kinds = sys.argv[1], sys.argv[2], sys.argv[3].split(',')
def run(kind, size):
    r = subprocess.run([binary, 'kind', kind, str(size), kib], capture_output=True, text=True, timeout=600)
    return r.returncode, (r.stdout.strip().splitlines() or [''])[-1]
for kind in kinds:
    lo, hi = 1, 1
    while True:
        code, text = run(kind, hi)
        if code != 0 or 'complete' not in text.split()[1:2]:
            break
        lo, hi = hi, hi * 2
        if hi > 4096: break
    while hi - lo > 1:
        mid = (lo + hi) // 2
        code, text = run(kind, mid)
        if code == 0 and text.split()[1:2] == ['complete']: lo = mid
        else: hi = mid
    print(kind, lo, run(kind, lo)[1], '|', hi, run(kind, hi), flush=True)
