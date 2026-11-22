import json, sys, collections
E='<scratch>/eval'
B='<after>'
before=json.load(open(f'{E}/runs/paths/before.json'))['cases']
after=json.load(open(f'{E}/runs/paths/after.json'))['cases']
cases={c['name']:c for c in json.load(open(f'{B}/fixtures/nesting-chains/cases.json'))}
paths=['ts-wasm','ts-native','rust-wasm','rust-native']
def short(o):
    s=o['stage']
    if s=='result':
        if o['status']=='complete': return f"complete {o['states']}/{o['generated']} w{o['work']}"
        return f"violation trace{o['trace']} w{o['work']}"
    return f"{s}: {o['message']}"
def nowork(o):
    return {**o,'work':None} if o['stage']=='result' else o
# 1. consistency within each side
def divmsg(m): return m.startswith('compile: expression Apply \\\\div') or m.startswith('compile: expression Apply %')
for side,data in (('before',before),('after',after)):
    ts_split=[];rust_split=[];cross=collections.Counter();other=[]
    for n,row in data.items():
        for v in ('v1','v2'):
            cell=row[v]
            if cell['ts-wasm']['outcome']!=cell['ts-native']['outcome']: ts_split.append((n,v))
            if cell['rust-wasm']['outcome']!=cell['rust-native']['outcome']: rust_split.append((n,v))
            t=cell['ts-native']['outcome']; r=cell['rust-native']['outcome']
            if t==r: cross['equal']+=1
            elif t['stage']=='compile' and (t['message'].startswith('expression Apply') ) : cross['ts refuses unsupported operator (\\div or %)']+=1
            elif r['stage']=='compile' and r['message'].startswith('TLA parsing: Expression nesting exceeds 64'): cross['rust parser limit (TS parses)']+=1
            else: cross['OTHER']+=1; other.append((n,v,short(t),short(r)))
    print(f'[{side}] ts-wasm vs ts-native outcome differs: {len(ts_split)}; rust-wasm vs rust-native differs: {len(rust_split)}; TS-vs-Rust: {dict(cross)}')
    for o in other[:30]: print('   OTHER',o)
# 2. frontend IR equality (ts vs rust), per side
for side,data in (('before',before),('after',after)):
    dif=[];cmp=0
    for n,row in data.items():
        for v in ('v1','v2'):
            c=row[v]
            if c['ts-native']['ir'] and c['rust-native']['ir']:
                cmp+=1
                if c['ts-native']['irNormalized']!=c['rust-native']['irNormalized']: dif.append((n,v))
            # wasm vs native IR from the Rust frontend
            if c['rust-wasm']['ir'] and c['rust-native']['ir'] and c['rust-wasm']['ir']!=c['rust-native']['ir']: dif.append((n,v,'wasm!=native'))
    print(f'[{side}] TS vs Rust IR (normalized) compared where both compile: {cmp}; differing: {len(dif)}', dif[:10])
# 3. before vs after
diffs=[]
for n in after:
    for v in ('v1','v2'):
        for p in paths:
            b=before[n][v][p]; a=after[n][v][p]
            if b['outcome']!=a['outcome'] or b['ir']!=a['ir']:
                diffs.append((n,v,p,b,a))
print('before vs after: differing (case,version,path) cells:',len(diffs),'of',len(after)*2*4)
byv=collections.Counter((d[1]) for d in diffs)
print(' by version:',dict(byv))
# classify
cls=collections.Counter(); weird=[]
for n,v,p,b,a in diffs:
    bo,ao=b['outcome'],a['outcome']
    if bo['stage']=='compile' and bo['message']=='expression or operator nesting exceeds 40':
        if ao['stage']=='compile': k='before 40-refusal -> after other compile refusal: '+ao['message']
        elif ao['stage']=='check': k='before 40-refusal -> after check refusal: '+ao['message']
        else: k='before 40-refusal -> after accepted ('+ao['stage']+('/'+ao['status'] if 'status' in ao else '')+')'
    else:
        k='OTHER'; weird.append((n,v,p,short(bo),short(ao),b['ir']==a['ir']))
    cls[(v,k)]+=1
for k,c in sorted(cls.items()): print(' ',c,k)
print('OTHER differences:',len(weird))
for w in weird[:40]: print('  ',w)
# 4. after vs implementer's expectations (cases.json) per frontend/version, ignoring work where expectation null
mism=[]
for n,row in after.items():
    if n.startswith('quint:'): continue
    exp=cases[n]['expect']
    for v in ('v1','v2'):
        for p in paths:
            fe='ts' if p.startswith('ts') else 'rust'
            e=exp[fe][v]; a=row[v][p]['outcome']
            if a!=e and not (e.get('work',1) is None and nowork(a)==e):
                mism.append((n,v,p,short(e),short(a)))
print('after vs recorded expectations mismatches:',len(mism)); 
for m in mism[:20]: print('  ',m)
# 5. equivalence partner: IR identical (v2) for entries with equivalent
neq=0;ok=0;bad=[]
for n,c in cases.items():
    if c['equivalent']:
        for p in paths:
            a=after[n]['v2'][p]; q=after[c['equivalent']]['v2'][p]
            if a['ir'] is None and q['ir'] is None: continue
            ok+=1
            if a['ir']!=q['ir']: bad.append((n,c['equivalent'],p,short(a['outcome']),short(q['outcome'])))
print(f'after: IR(v2) equals IR of equivalent spelling: compared {ok} (case,path) pairs with a compiled program; differing {len(bad)}')
for b in bad[:30]: print('  ',b)
# same for before side to show which pairs were not equal before
bad2=[]; ok2=0
for n,c in cases.items():
    if c['equivalent']:
        for p in paths:
            a=before[n]['v2'][p]; q=before[c['equivalent']]['v2'][p]
            if a['ir'] is None and q['ir'] is None: continue
            ok2+=1
            if a['ir']!=q['ir']: bad2.append((n,p))
print(f'before: pairs compared {ok2}; differing {len(bad2)} (before: chains exceeding 40 have no IR, so these count pairs where only one side compiled)')
