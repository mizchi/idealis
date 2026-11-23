# usage: bench-abba.py <runName> <bench name>...   ABBA (before, after, after, before), serial, isolated CRITERION_HOME per run
import subprocess, sys, os, json, time, datetime, hashlib
E='<scratch>'
run=sys.argv[1]; names=sys.argv[2:]
manifest=json.load(open(f'{E}/snapshots/bench-manifest-final.json'))
def load(): return subprocess.run(['uptime'],capture_output=True,text=True).stdout.split('load averages:')[-1].strip()
for name in names:
    base=f'{E}/runs/{run}/{name}'
    os.makedirs(base)
    rec=[]
    for i,side in enumerate(['before','after','after','before']):
        exe=f'{E}/snapshots/{ {"before": "before", "after": "after-r5"}[side]}/bench/{name}'
        sha=hashlib.sha256(open(exe,'rb').read()).hexdigest()
        assert sha==manifest[side][name]['sha256'], (side,name)
        d=f'{base}/{i}-{side}'; os.makedirs(d)
        env=dict(os.environ); env['CRITERION_HOME']=f'{d}/criterion'
        env.pop('CARGO_TARGET_DIR',None); env.pop('CARGO_BUILD_TARGET_DIR',None)
        t0=datetime.datetime.utcnow().isoformat()+'Z'; l0=load(); s=time.time()
        p=subprocess.run([exe,'--bench','--noplot'],env=env,capture_output=True,text=True)
        e=time.time()
        open(f'{d}/stdout.log','w').write(p.stdout); open(f'{d}/stderr.log','w').write(p.stderr)
        rec.append({'index':i,'side':side,'exe':exe,'sha256':sha,'exit':p.returncode,'start':t0,'seconds':round(e-s,1),'load_start':l0,'load_end':load()})
        print(name,i,side,'exit',p.returncode,round(e-s,1),'s load',l0,flush=True)
        if p.returncode!=0: break
    json.dump(rec,open(f'{base}/runs.json','w'),indent=1)
