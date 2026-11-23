# usage: bench-table.py <runName> [benchName...]  -> table of before/after means (ns), change %, spreads, flags >= +10%
import json, os, sys
E='<scratch>'
run=sys.argv[1]; only=sys.argv[2:]
def estimates(root):
    out={}
    for dp,dn,fn in os.walk(root):
        if dp.endswith('/new') and 'estimates.json' in fn:
            name=os.path.relpath(os.path.dirname(dp),root)
            j=json.load(open(f'{dp}/estimates.json'))
            out[name]=(j['mean']['point_estimate'], j['mean']['confidence_interval']['lower_bound'], j['mean']['confidence_interval']['upper_bound'], j['median']['point_estimate'])
    return out
tot=0; flagged=[]
rows=[]
for bench in sorted(os.listdir(f'{E}/runs/{run}')):
    if only and bench not in only: continue
    base=f'{E}/runs/{run}/{bench}'
    if not os.path.isdir(base): continue
    runs=[estimates(f'{base}/{i}-{s}/criterion') for i,s in enumerate(['before','after','after','before'])]
    names=set(runs[0])
    assert all(set(r)==names for r in runs), (bench,[len(r) for r in runs])
    for n in sorted(names):
        e=[r[n][0] for r in runs]
        b=(e[0]+e[3])/2; a=(e[1]+e[2])/2
        change=(a/b-1)*100
        sb=abs(e[0]-e[3])/b*100; sa=abs(e[1]-e[2])/a*100
        rows.append((bench,n,e,b,a,change,sb,sa))
        tot+=1
        if change>=10: flagged.append((bench,n,change))
print(f"{'bench':<20}{'function':<52}{'before ns':>12}{'after ns':>12}{'chg%':>8}   B0 A1 A2 B3 (ns)   spreadB spreadA")
for bench,n,e,b,a,c,sb,sa in rows:
    print(f"{bench:<20}{n:<52}{b:>12.0f}{a:>12.0f}{c:>+8.2f}   {e[0]:.0f} {e[1]:.0f} {e[2]:.0f} {e[3]:.0f}   {sb:5.1f}% {sa:5.1f}%{'  FLAG>=10%' if c>=10 else ''}")
ch=[r[5] for r in rows]
print('functions',tot,'max change %',round(max(ch),2),'min',round(min(ch),2),'flagged >=+10%:',len(flagged))
