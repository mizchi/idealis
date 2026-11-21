import json, collections
E='<scratch>/eval'
R='<scratch>/eval-r2'
B=json.load(open(f'{E}/runs/ir-corpora/before.json'))
A1=json.load(open(f'{E}/runs/ir-corpora/after.json'))
A2=json.load(open(f'{R}/runs/ir-corpora/after.json'))
in1={i['name']:i for i in json.load(open(f'{E}/runs/ir-corpora/inputs.json'))}
in2={i['name']:i for i in json.load(open(f'{R}/runs/ir-corpora/inputs.json'))}
print('first inputs',len(in1),'r2 inputs',len(in2),'r2 inputsSha256',A2['inputsSha256'],'first inputsSha256',B['inputsSha256'])
assert A1['inputsSha256']==B['inputsSha256']
missing=[n for n in in1 if n not in in2]
changed=[n for n in in1 if n in in2 and (in1[n]['source'],in1[n]['config'])!=(in2[n]['source'],in2[n]['config'])]
extra=[n for n in in2 if n not in in1]
print('first inputs missing from r2:',len(missing),'source/config changed:',len(changed),'extra in r2:',len(extra),collections.Counter(in2[n]['group'] for n in extra))
S=('ts','wasm','native')
def cmp(X,Y,label):
    tot=same=0; diffs=[]
    for n in in1:
        for v in ('v1','v2'):
            for s in S:
                tot+=1
                if X['cells'][n][v][s]==Y['cells'][n][v][s]: same+=1
                else: diffs.append((n,v,s,X['cells'][n][v][s][:100],Y['cells'][n][v][s][:100]))
    print(f'{label}: cells compared {tot} identical {same} differing {len(diffs)}')
    for d in diffs[:20]: print('  DIFF',d)
cmp(B,A2,'before vs after-r2 (original 4651 inputs)')
cmp(A1,A2,'first after vs after-r2 (original 4651 inputs)')
k=collections.Counter()
for n in extra:
    for v in ('v1','v2'):
        for s in S: k[(v,s,A2['cells'][n][v][s][0])]+=1
print('extra (limit) inputs, cell kinds:',dict(sorted(k.items())))
for side,D in (('after-r2',A2),):
    c=d=0
    for n in in1:
        for v in ('v1','v2'):
            t=D['cells'][n][v]['ts']; r=D['cells'][n][v]['native']
            if t.startswith('p:') and r.startswith('p:'):
                c+=1; d+= t.split(':')[2]!=r.split(':')[2]
    print(f'[{side}] TS vs Rust-native normalized IR compared {c}, differing {d} (original inputs)')
    c=d=0
    for n in in1:
        for v in ('v1','v2'):
            w=D['cells'][n][v]['wasm']; r=D['cells'][n][v]['native']
            if w.startswith('p:') and r.startswith('p:'):
                c+=1; d+= w.split(':')[2]!=r.split(':')[2]
    print(f'[{side}] Rust wasm vs native normalized IR compared {c}, differing {d} (original inputs)')
