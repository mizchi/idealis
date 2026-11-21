import hashlib, os, re, signal, subprocess, sys, time, datetime, json
F='<scratch>/fix1'
B='<after>'
env=dict(os.environ, NPM_CONFIG_USERCONFIG='/dev/null')
env.pop('CARGO_TARGET_DIR',None); env.pop('CARGO_BUILD_TARGET_DIR',None)
TS='packages/idealis/src/tla-safety.ts'; LOW='crates/rtla-compiler/src/typed_lower.rs'; EXP='crates/rtla-compiler/src/typed_expr.rs'
plants=[
 ('m1b',LOW,r'let bound = frame\.bound\s*\+\s*frame\.result','let bound = frame.result','rust'),
 ('m4',LOW,r'for \(condition, taken\) in arms\.into_iter\(\)\.rev\(\)','for (condition, taken) in arms.into_iter()','rust'),
 ('m5',EXP,r'for operand in layer\.iter\(\)\.skip\(1\)','for operand in layer.iter()','rust'),
 ('t1',TS,r'frame\.bound \+\n\s*\(frame\.result\[frame\.branch\]\?\.steps','(frame.result[frame.branch]?.steps','ts'),
 ('t2',TS,r'for \(const layer of layers\.reverse\(\)\)','for (const layer of layers)','ts'),
 ('t3',TS,r'limited\(\[\n\s*\.\.\.arm\.taken,','limited([','ts'),
 ('t4',TS,r'pending\.push\(\{ operands: inherited\(operand\), index: 0 \}\)','pending.push({ operands: [...inherited(operand)].reverse(), index: 0 })','ts'),
 ('t5',TS,r'branches: limited\(frame\.next\) \}','branches: frame.next }','ts'),
 ('F2',TS,r'branches: limited\(frame\.next\) \}','branches: frame.next }','ts'),
 ('F6',TS,r'frame\.result = limited\(frame\.next\);','frame.result = frame.next;','ts'),
]
only=sys.argv[1:]
def sha(p): return hashlib.sha256(open(os.path.join(B,p),'rb').read()).hexdigest()
def run(cmd,limit):
    p=subprocess.Popen(cmd,cwd=B,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True,text=True)
    try:
        out,_=p.communicate(timeout=limit); return p.returncode,out,False
    except subprocess.TimeoutExpired:
        os.killpg(p.pid,signal.SIGKILL); out,_=p.communicate(); return None,out,True
results={}
for name,path,pat,rep,kind in plants:
    if only and name not in only: continue
    full=os.path.join(B,path); orig=open(full,'rb').read(); s0=hashlib.sha256(orig).hexdigest()
    text=orig.decode(); new,n=re.subn(pat,rep,text)
    assert n==1,(name,n)
    start=datetime.datetime.utcnow().isoformat()
    try:
        open(full,'w').write(new)
        if kind=='ts':
            r=run(['pnpm','exec','tsc','-p','tsconfig.json'],300); assert r[0]==0,r[1][-500:]
            code,out,to=run(['node','--test','dist/tests/nesting-chains/contract-test.js'],900)
        else:
            code,out,to=run(['cargo','test','--locked','--manifest-path','crates/rtla-compiler/Cargo.toml','--test','nesting_chains','--target-dir',F+'/target-fix1'],900)
    finally:
        open(full,'wb').write(orig)
    ok=sha(path)==s0
    if kind=='ts':
        failed=sorted(set(re.findall(r'^✖ (.+?) \(\d',out,re.M)))
        failed=[f for f in failed if not f.startswith('failing')]
    else:
        failed=re.findall(r'^test (\S+) \.\.\. FAILED',out,re.M)
    results[name]={'file':path,'sha_restored':ok,'exit':code,'timeout':to,'failed':failed,'start':start,'end':datetime.datetime.utcnow().isoformat()}
    open(F+f'/logs/plant-{name}.log','w').write(out)
    print(name,json.dumps({k:v for k,v in results[name].items() if k!='failed'}),len(failed),failed[:4],flush=True)
    subprocess.run(['git','checkout','--','pnpm-lock.yaml'],cwd=B)
json.dump(results,open(F+'/plants-result%s.json'%('-'+'-'.join(only) if only else ''),'w'),indent=1)
