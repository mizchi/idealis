import json, sys, os
D = sys.argv[1]
man = json.load(open(f'{D}/manifest.json'))
res = json.load(open(f'{D}/result.json'))
def flat(x, p=''):
    out = {}
    if isinstance(x, dict):
        for k, v in x.items(): out.update(flat(v, f'{p}.{k}' if p else k))
    elif isinstance(x, list) and len(x) <= 50 and not any(isinstance(i,(dict,list)) for i in x):
        out[p] = json.dumps(x)
    elif isinstance(x, list):
        for i, v in enumerate(x): out.update(flat(v, f'{p}[{i}]'))
    else:
        out[p] = json.dumps(x)
    return out
pairs = 0; diffs = []
workvals = {}
fields_seen = set()
for m in man['inputs']:
    for v in (1, 2):
        for it in range(4):
            recs = {}
            for side in ('before', 'after'):
                for be in ('native', 'wasm'):
                    r = json.load(open(f'{D}/{m["name"]}-{it}-{side}-{be}-v{v}.json'))
                    recs[(side, be)] = r
            for be in ('native', 'wasm'):
                b, a = recs[('before', be)], recs[('after', be)]
                pairs += 1
                for key in ('status',):
                    if b.get(key) != a.get(key): diffs.append((m['name'], v, it, be, key, b.get(key), a.get(key)))
                for key in ('program', 'result', 'response'):
                    fb, fa = flat(b.get(key)), flat(a.get(key))
                    fields_seen.update(k for k in fb if key == 'result')
                    for k in sorted(set(fb) | set(fa)):
                        if fb.get(k) != fa.get(k):
                            diffs.append((m['name'], v, it, be, f'{key}.{k}' if k else key, (fb.get(k) or '')[:80], (fa.get(k) or '')[:80]))
                if b.get('status') == 'complete':
                    workvals.setdefault((m['name'], v), set()).add((b['result']['work'], a['result']['work']))
print('pairs compared (before vs after, per native/wasm, per input x version x iteration):', pairs)
print('result fields seen:', sorted(fields_seen))
print('differing fields:', len(diffs))
for d in diffs[:60]: print(d)
print('work (before, after) per input/version, distinct pairs:')
for k, vs in workvals.items(): print(' ', k, sorted(vs))
cold = res['cold']
cd = 0
bymap = {}
for c in cold:
    bymap[(c['input'], c['iteration'], c['route'])] = c['result']
for (inp, it, route), r in bymap.items():
    if route.startswith('before-native'):
        a = bymap[(inp, it, route.replace('before', 'after'))]
        if r != a:
            cd += 1; print('cold diff', inp, it, route, {k: (r.get(k), a.get(k)) for k in set(r) | set(a) if r.get(k) != a.get(k)})
print('cold (fresh process, check-tla) before/after result pairs compared:', sum(1 for k in bymap if k[2].startswith('before-native')), 'differing:', cd)
