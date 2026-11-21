import json, subprocess, sys, os, hashlib, re
fixtures, before, after, out = sys.argv[1:5]
os.makedirs(out)
cases = json.load(open(fixtures))['cases']
bounds = {'maxDepth': 100, 'maxStates': 10000, 'maxWork': 100000}
rows = []
for version in (1, 2):
    case = next(c for c in cases if c['spec'] == 'huge' and c['variant'] == 'complete' and c['irVersion'] == version)
    req = json.dumps({'version': 1, 'request': {'kind': 'check-safety', 'program': case['program'], 'bounds': bounds}}) + '\n'
    reqpath = f'{out}/stdin-huge-v{version}.json'
    open(reqpath, 'x').write(req)
    for rep in range(3):
        for side, root in (('before', before), ('after', after)) if rep % 2 == 0 else (('after', after), ('before', before)):
            p = subprocess.run(['/usr/bin/time', '-l', f'{root}/idealis-core'], input=req, capture_output=True, text=True)
            tag = f'huge-v{version}-r{rep}-{side}'
            open(f'{out}/stdout-{tag}.json', 'x').write(p.stdout)
            open(f'{out}/stderr-{tag}.txt', 'x').write(p.stderr)
            msg = json.loads(p.stdout.strip().split('\n')[0])
            rss = int(re.search(r'(\d+)\s+maximum resident set size', p.stderr).group(1))
            wall = float(re.search(r'([\d.]+)\s+real', p.stderr).group(1))
            res = msg.get('result', msg)
            rows.append({'version': version, 'rep': rep, 'side': side, 'exit': p.returncode, 'kind': msg.get('kind'), 'wallS': wall, 'rssBytes': rss, 'result': res})
json.dump({'bounds': bounds, 'fixturesSha256': hashlib.sha256(open(fixtures, 'rb').read()).hexdigest(), 'before': before, 'after': after, 'rows': rows}, open(f'{out}/result.json', 'x'), indent=1)
print('bounds', bounds)
for v in (1, 2):
    for side in ('before', 'after'):
        rs = [r for r in rows if r['version'] == v and r['side'] == side]
        r0 = rs[0]['result']
        same = all(r['result'] == r0 for r in rs)
        print(f"v{v} {side:6} exit={[r['exit'] for r in rs]} kind={rs[0]['kind']} status={r0['status']} reason={r0.get('reason')} states={r0['states']} generated={r0['generated']} depth={r0['depth']} work={r0['work']} violation={r0.get('violation')} wall(s)={[r['wallS'] for r in rs]} rssMiB={[round(r['rssBytes']/1048576,1) for r in rs]} deterministic-across-reps={same}")
    b = next(r for r in rows if r['version'] == v and r['side'] == 'before')['result']
    a = next(r for r in rows if r['version'] == v and r['side'] == 'after')['result']
    print(f"v{v} differing keys: {sorted(k for k in set(a) | set(b) if a.get(k) != b.get(k))}  generated before/after {b['generated']}/{a['generated']}  work before/after {b['work']}/{a['work']}")
