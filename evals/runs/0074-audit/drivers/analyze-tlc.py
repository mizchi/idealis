import json, re, collections, sys
E='<scratch>/eval'
tlc=json.load(open(f'{E}/runs/tlc/results.json'))
A=json.load(open(f'{E}/runs/paths/after.json'))['cases']
B=json.load(open(f'{E}/runs/paths/before.json'))['cases']
def verdict(name,v):
    log=open(f"{v['dir']}/tlc.log").read()
    if v['completed']: return ('complete',v['states'],v['generated'])
    if re.search(r'Invariant \w+ is violated',log):
        return ('violation', max(v['trace'],1))
    if re.search(r'second argument of \\div is 0|second argument of % is 0|division by zero',log): return ('division-by-zero',)
    if re.search(r'non-boolean expression',log): return ('non-boolean',)
    if re.search(r'when [PQ] was\s+not|when Q was|when P was',log) and re.search(r'boolean|BOOLEAN',log): return ('non-boolean',)
    if re.search(r'Parsing or semantic analysis failed',log):
        m=re.search(r"Unknown operator: `(\w+)'",log)
        return ('semantic-error', m.group(1) if m else '?')
    return ('other-error', v['errors'][:2])
def ours(o):
    s=o['stage']
    if s=='result':
        if o['status']=='complete': return ('complete',o['states'],o['generated'])
        return ('violation',o['trace'])
    if s=='check':
        if o['message']=='Integer division by zero': return ('division-by-zero',)
        if o['message'].startswith('Expected a boolean'): return ('non-boolean',)
        if o['message'].startswith('Safety expression nesting exceeds'): return ('limit-refusal','checker-48')
        return ('check-refusal',o['message'])
    if s=='compile':
        m=o['message']
        if m.startswith('expression or operator nesting exceeds 40'): return ('limit-refusal','frontend-40')
        if m.startswith('TLA parsing: Expression nesting exceeds 64'): return ('limit-refusal','parser-64')
        mm=re.match(r'operator (\w+)',m)
        if mm: return ('semantic-error',mm.group(1))
        if m.startswith('expression Apply'): return ('unsupported-by-frontend',m)
        return ('compile-refusal',m)
    return ('exception',o)
rows=[]
cnt=collections.Counter(); bad=[]
for name,v in tlc.items():
    tv=verdict(name,v)
    for side,D in (('before',B),('after',A)):
        for ver in ('v1','v2'):
            for path in ('rust-native','ts-native'):
                o=ours(D[name][ver][path]['outcome'])
                if o==tv: cls='match'
                elif o[0]=='limit-refusal': cls='limit-refusal ('+o[1]+')'
                elif o[0]=='unsupported-by-frontend': cls='frontend lacks operator ('+o[1]+')'
                elif tv[0]=='semantic-error' and o[0]=='semantic-error': cls='match' if o[1]==tv[1] else 'MISMATCH'
                elif tv[0]=='complete' and o[0]=='complete': cls='MISMATCH counts'
                else: cls='MISMATCH'
                cnt[(side,ver,path,cls)]+=1
                if cls.startswith('MISMATCH'): bad.append((name,side,ver,path,tv,o))
for k,c in sorted(cnt.items()): print(c,k)
print('MISMATCH rows:',len(bad))
for b in bad[:60]: print(b)
json.dump({n:verdict(n,v) for n,v in tlc.items()}, open(f'{E}/runs/tlc/verdicts.json','w'), indent=1)
# summary of TLC verdicts
vc=collections.Counter(verdict(n,v)[0] for n,v in tlc.items()); print('TLC verdicts:',dict(vc))
