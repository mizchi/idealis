import json, collections
E='<scratch>/eval'
R='<scratch>/eval-r2'
B='<after>'
first=json.load(open(f'{E}/runs/paths/after.json'))['cases']
r2=json.load(open(f'{R}/runs/paths-after-r2.json'))['cases']
cases={c['name']:c for c in json.load(open(f'{B}/fixtures/nesting-chains/cases.json'))}
paths=['ts-wasm','ts-native','rust-wasm','rust-native']
def short(o):
    s=o['stage']
    if s=='result':
        if o['status']=='complete': return f"complete {o['states']}/{o['generated']} w{o['work']}"
        return f"violation trace{o['trace']} w{o['work']}"
    return f"{s}: {o['message']}"
def nowork(o): return {**o,'work':None} if o['stage']=='result' else o
print('first after cases:',len(first),'r2 cases:',len(r2))
orig=[n for n in r2 if n in first]
new=[n for n in r2 if n not in first]
print('inputs present in both:',len(orig),' new in r2:',len(new),' missing from r2:',[n for n in first if n not in r2])
tot=0; diffs=[]
fields=collections.Counter()
for n in orig:
    for v in ('v1','v2'):
        for p in paths:
            a=first[n][v][p]; b=r2[n][v][p]; tot+=1
            for f in ('outcome','ir','irNormalized'):
                if a[f]!=b[f]: fields[f]+=1
            if a!=b: diffs.append((n,v,p,short(a['outcome']),short(b['outcome']),a['ir'],b['ir']))
print(f'first-after vs after-r2 over the original inputs: cells compared {tot} (inputs x v1,v2 x 4 paths), differing cells {len(diffs)}; differing fields {dict(fields)}')
for d in diffs[:20]: print('  DIFF',d)
mism=[]; checked=0
for n in new:
    exp=cases[n]['expect']
    for v in ('v1','v2'):
        for p in paths:
            fe='ts' if p.startswith('ts') else 'rust'
            e=exp[fe][v]; a=r2[n][v][p]['outcome']; checked+=1
            if a!=e and not (e.get('work',1) is None and nowork(a)==e):
                mism.append((n,v,p,short(e),short(a)))
print(f'limit cases: {len(new)} inputs, {checked} cells vs cases.json expectations; mismatches {len(mism)}')
for m in mism[:20]: print('  MISMATCH',m)
mism_all=[]; checked_all=0
for n,row in r2.items():
    if n.startswith('quint:'): continue
    exp=cases[n]['expect']
    for v in ('v1','v2'):
        for p in paths:
            fe='ts' if p.startswith('ts') else 'rust'
            e=exp[fe][v]; a=row[v][p]['outcome']; checked_all+=1
            if a!=e and not (e.get('work',1) is None and nowork(a)==e): mism_all.append((n,v,p))
print(f'all non-quint cases vs expectations: {checked_all} cells, mismatches {len(mism_all)}')
bad=[]; ok=0
for n,c in cases.items():
    if c['equivalent']:
        for p in paths:
            a=r2[n]['v2'][p]; q=r2[c['equivalent']]['v2'][p]
            if a['ir'] is None and q['ir'] is None: continue
            ok+=1
            if a['ir']!=q['ir']: bad.append((n,c['equivalent'],p))
print(f'after-r2: IR(v2) equals IR of equivalent spelling: {ok} pairs compared, differing {len(bad)}')
sp=[];cmp=0
for n,row in r2.items():
    for v in ('v1','v2'):
        c=row[v]
        if c['ts-native']['ir'] and c['rust-native']['ir']:
            cmp+=1
            if c['ts-native']['irNormalized']!=c['rust-native']['irNormalized']: sp.append((n,v))
        if c['rust-wasm']['ir'] and c['rust-native']['ir'] and c['rust-wasm']['ir']!=c['rust-native']['ir']: sp.append((n,v,'wasm!=native'))
        if c['ts-wasm']['outcome']!=c['ts-native']['outcome']: sp.append((n,v,'ts wasm!=native outcome'))
        if c['rust-wasm']['outcome']!=c['rust-native']['outcome']: sp.append((n,v,'rust wasm!=native outcome'))
print(f'after-r2 TS vs Rust normalized IR compared where both compile: {cmp}; differing (incl wasm/native splits) {len(sp)}', sp[:10])
exc=[(n,v,p) for n,row in r2.items() for v in ('v1','v2') for p in paths if row[v][p]['outcome']['stage']=='exception']
print('exception cells in r2:',len(exc), exc[:5])
