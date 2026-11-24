import json, collections
E='<scratch>'
B=json.load(open(f'{E}/runs/ir-corpora/before.json'))
A=json.load(open(f'{E}/runs/ir-corpora/after.json'))
inputs={i['name']:i for i in json.load(open(f'{E}/runs/ir-corpora/inputs.json'))}
assert B['inputsSha256']==A['inputsSha256']
print('inputs',B['count'],A['count'],'sha256',A['inputsSha256'])
total=0; same=0; diffs=[]
kinds=collections.Counter()
for n,row in A['cells'].items():
    for v in ('v1','v2'):
        for s in ('ts','wasm','native'):
            b=B['cells'][n][v][s]; a=row[v][s]
            total+=1
            if b==a: same+=1
            else: diffs.append((n,v,s,b,a))
for n,row in A['cells'].items():
    for v in ('v1','v2'):
        for s in ('ts','wasm','native'):
            kinds[(v,s,row[v][s][0])]+=1
print('cells compared (input x version x surface):',total,'identical:',same,'differing:',len(diffs))
print('after cell kinds (p=program, e=refusal):',dict(sorted(kinds.items())))
cls=collections.Counter(); other=[]
for n,v,s,b,a in diffs:
    if b.startswith('e:') and 'expression or operator nesting exceeds 40' in b:
        k='before refused at 40 -> after '+('program' if a.startswith('p:') else 'refusal: '+a[2:])
    else:
        k='OTHER'; other.append((n,v,s,b[:90],a[:90]))
    cls[(v,s,k)]+=1
for k,c in sorted(cls.items()): print(c,k)
print('OTHER:',len(other))
for o in other[:20]: print(o)
# list the inputs whose cells changed
chg=sorted({d[0] for d in diffs}); print('inputs with a changed cell:',len(chg))
with open(f'{E}/runs/ir-corpora/changed-inputs.tsv','w') as f:
    f.write('input\tgroup\tversion\tsurface\tbefore\tafter\n')
    for n,v,s,b,a in diffs:
        f.write(f"{n}\t{inputs[n]['group']}\t{v}\t{s}\t{b[:200]}\t{a[:200]}\n")
print('by group:',collections.Counter(inputs[n]['group'] for n in chg))
# TS vs Rust IR parity per side where both programs
for side,D in (('before',B),('after',A)):
    cmp=0; d=0
    for n,row in D['cells'].items():
        for v in ('v1','v2'):
            t=row[v]['ts']; r=row[v]['native']
            if t.startswith('p:') and r.startswith('p:'):
                cmp+=1
                if t.split(':')[2]!=r.split(':')[2]: d+=1
    print(f'[{side}] TS vs Rust-native normalized IR compared {cmp}, differing {d}')
# Rust wasm vs native raw IR
for side,D in (('before',B),('after',A)):
    cmp=0; d=0
    for n,row in D['cells'].items():
        for v in ('v1','v2'):
            w=row[v]['wasm']; r=row[v]['native']
            if w.startswith('p:') and r.startswith('p:'):
                cmp+=1
                if w.split(':')[2]!=r.split(':')[2]: d+=1
    print(f'[{side}] Rust wasm vs native normalized IR compared {cmp}, differing {d}')
