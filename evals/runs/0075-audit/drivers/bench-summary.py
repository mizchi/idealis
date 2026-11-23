# usage: bench-summary.py <run> [<run2> ...]   per-bench summary; with several runs also per-function flag persistence
import json, os, sys, statistics
E='<scratch>'
def estimates(root):
    out={}
    for dp,dn,fn in os.walk(root):
        if dp.endswith('/new') and 'estimates.json' in fn:
            name=os.path.relpath(os.path.dirname(dp),root)
            out[name]=json.load(open(f'{dp}/estimates.json'))['mean']['point_estimate']
    return out
def load(run):
    res={}
    for bench in sorted(os.listdir(f'{E}/runs/{run}')):
        base=f'{E}/runs/{run}/{bench}'
        if not os.path.isdir(base): continue
        runs=[estimates(f'{base}/{i}-{s}/criterion') for i,s in enumerate(['before','after','after','before'])]
        for n in runs[0]:
            e=[r[n] for r in runs]
            res[(bench,n)]=e
    return res
runs=sys.argv[1:]
data={r:load(r) for r in runs}
keys=sorted(set.intersection(*[set(d) for d in data.values()]))
print('functions',len(keys))
for r in runs:
    ch=[]; chmin=[]; flagged=0
    for k in keys:
        e=data[r][k]; b=(e[0]+e[3])/2; a=(e[1]+e[2])/2
        ch.append((a/b-1)*100); chmin.append((min(e[1],e[2])/min(e[0],e[3])-1)*100)
        flagged+= (a/b-1)*100>=10
    print(f'{r}: mean-based change median {statistics.median(ch):+.2f}% (p10 {sorted(ch)[len(ch)//10]:+.2f}, p90 {sorted(ch)[9*len(ch)//10]:+.2f}) flagged>=+10%: {flagged}; best-of-2 based change median {statistics.median(chmin):+.2f}% max {max(chmin):+.2f}%')
if len(runs)>1:
    pers=[]
    for k in keys:
        fl=[((sum(data[r][k][1:3])/2)/((data[r][k][0]+data[r][k][3])/2)-1)*100>=10 for r in runs]
        if all(fl): pers.append(k)
    print('flagged (>= +10% mean) in ALL runs:',len(pers))
    for k in pers: print('  ',k,[round(((sum(data[r][k][1:3])/2)/((data[r][k][0]+data[r][k][3])/2)-1)*100,2) for r in runs])
