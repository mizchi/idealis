import assert from 'node:assert/strict';
import{execFileSync}from'node:child_process';
import{createHash}from'node:crypto';
import{appendFile,mkdir,readdir,readFile,writeFile}from'node:fs/promises';
import{cpus}from'node:os';
import{resolve}from'node:path';
import{gzipSync}from'node:zlib';
const directory='evals/runs/0067-audit';await mkdir(directory);
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const save=(path,value)=>writeFile(directory+'/'+path,JSON.stringify(value,null,2)+'\n',{flag:'wx'});
const full=[];
for(const name of['full-before','full-after','final']){
 const root='.artifacts/0067-'+name;const bytes=await readFile(root+'/manifest.json');const manifest=JSON.parse(bytes);
 for(const file of manifest.files)assert.equal(digest(await readFile(root+'/'+file.path)),file.sha256);
 const archive=gzipSync(await readFile(root+'/source.tar'));await writeFile(directory+'/'+name+'-source.tar.gz',archive,{flag:'wx'});await save(name+'-manifest.json',manifest);
 full.push({name,manifestSha256:digest(bytes),archiveSha256:digest(archive),files:manifest.files});
}
const symmetry=[];
for(const name of['before','after']){
 const root='.artifacts/0067-'+name;const bytes=await readFile(root+'/criterion.json');const metadata=JSON.parse(bytes);
 assert.equal(digest(await readFile(root+'/symmetry-bench')),metadata.binarySha256);
 for(const source of metadata.sources)assert.equal(digest(execFileSync('tar',['-xOf',resolve(root,'source.tar'),source.path])),source.sha256);
 const archive=gzipSync(await readFile(root+'/source.tar'));await writeFile(directory+'/'+name+'-symmetry-source.tar.gz',archive,{flag:'wx'});await save(name+'-symmetry.json',metadata);
 symmetry.push({name,...metadata,archiveSha256:digest(archive)});
}
const verification=JSON.parse(await readFile('evals/runs/0067-verification/result.json','utf8'));
assert.equal(verification.status,'complete');assert.equal(verification.unchanged,true);assert.ok(verification.results.every(result=>result.status===0));
const drivers=['.artifacts/0067-freeze.ts','evals/symmetry/allocation.ts','evals/oracle-tuning/kernel.ts','evals/evaluator-tuning/summarize.ts','evals/p0-checker/verify.ts','evals/temporal-tuning/parity.ts','evals/symmetry/evaluate.ts','.artifacts/0067-archive.ts'];
await mkdir(directory+'/drivers');const copied=[];
for(const[index,path]of drivers.entries()){const bytes=await readFile(path);const name='drivers/'+index+'-'+path.split('/').at(-1);await writeFile(directory+'/'+name,bytes,{flag:'wx'});copied.push({path,name,sha256:digest(bytes)});}
const boundary=JSON.parse(await readFile('evals/runs/0067-boundary/manifest.json','utf8'));assert.equal(digest(await readFile('evals/symmetry/allocation.ts')),boundary.driverSha256);
await writeFile(directory+'/symmetry-before.rs',await readFile('.artifacts/0067-before-symmetry.rs'),{flag:'wx'});
await writeFile(directory+'/plan.md',await readFile('evals/plans/0067-record-schema.md'),{flag:'wx'});
await mkdir(directory+'/logs');const logs=[];
for(const name of(await readdir('.artifacts')).filter(name=>name.startsWith('0067-')&&name.endsWith('.log')).sort()){const bytes=await readFile('.artifacts/'+name);const archive=gzipSync(bytes);await writeFile(directory+'/logs/'+name+'.gz',archive,{flag:'wx'});logs.push({name,sha256:digest(bytes),gzipSha256:digest(archive)});}
const runs=(await readdir('evals/runs')).filter(name=>name.startsWith('0067-')&&name!=='0067-audit').sort();
await save('result.json',{status:'complete',plan:'0067-record-schema',baseline:'4587a3ea',planCommit:'6ce7bb5c',full,symmetry,drivers:copied,runs,logs,environment:{cpu:cpus()[0]?.model,node:process.version,arch:process.arch,platform:process.platform,rustc:execFileSync('rustc',['-vV']).toString(),cargo:execFileSync('cargo',['-V']).toString()},tlcSource:{revision:'d8fb231de4e4b5d02c1b2ed1ac759c086f694021',paths:['tlc2/tool/TLCStateMut.java','tlc2/value/impl/FcnRcdValue.java'],referenceJar:'1.7.4 5a47802'}});
await appendFile('evals/runs/ledger.jsonl',JSON.stringify({runId:'0067-audit',kind:'source-audit',status:'complete'})+'\n');
