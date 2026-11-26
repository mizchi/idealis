import sys,json
def load(p):
    d={}
    for line in open(p):
        n,t=line.rstrip("\n").split("\t",1); d[n]=t
    return d
real,obs=load(sys.argv[1]),load(sys.argv[2])
m=b'"probe":{'
eq=ne=own=0; bad=[]
for n,t in obs.items():
    i=t.index('"probe":{')
    j=t.index('"published256":',i)
    p=t[j+len('"published256":'):-2]
    if p=="null":
        own+=1
        w=t[:i-1]+"}"
        if w!=real[n]: bad.append(("own-differs",n))
        continue
    if p==real[n]: eq+=1
    else: ne+=1; bad.append(("published-differs",n,p[:120],real[n][:120]))
print(f"cells {len(obs)}, own-response cells {own}, published256 cells {eq+ne}: byte-identical to the real response {eq}, different {ne}")
for b in bad[:10]: print(b)
