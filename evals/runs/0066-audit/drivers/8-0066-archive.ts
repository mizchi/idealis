import assert from 'node:assert/strict';
import{createHash}from'node:crypto';
import{appendFile,mkdir,readdir,readFile,writeFile}from'node:fs/promises';
import{gzipSync}from'node:zlib';
const directory='evals/runs/0066-audit';await mkdir(directory);
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const save=(path,value)=>writeFile(directory+'/'+path,JSON.stringify(value,null,2)+'\n',{flag:'wx'});
const snapshots=[];
for(const name of['before','after','final']){
 const root='.artifacts/0066-'+name;
 const bytes=await readFile(root+'/manifest.json');const manifest=JSON.parse(bytes);
 for(const file of manifest.files)assert.equal(digest(await readFile(root+'/'+file.path)),file.sha256);
 const archive=gzipSync(await readFile(root+'/source.tar'));await writeFile(directory+'/'+name+'-source.tar.gz',archive,{flag:'wx'});await save(name+'-manifest.json',manifest);
 snapshots.push({name,manifestSha256:digest(bytes),archiveSha256:digest(archive),files:manifest.files});
}
const verification=JSON.parse(await readFile('evals/runs/0066-verification/result.json','utf8'));
assert.equal(verification.status,'failed');assert.equal(verification.unchanged,true);
assert.equal(verification.results[0].status,1);assert.ok(verification.results.slice(1).every(result=>result.status===0));
const retry=JSON.parse(await readFile('evals/runs/0066-check-retry/result.json','utf8'));assert.equal(retry.status,'complete');
const drivers=['.artifacts/0066-confirm.ts','evals/p0-checker/corpus.ts','evals/evaluator-tuning/corpus-summary.ts','evals/oracle-tuning/kernel.ts','evals/evaluator-tuning/summarize.ts','evals/p0-checker/verify.ts','evals/temporal-tuning/parity.ts','evals/symmetry/evaluate.ts','.artifacts/0066-archive.ts'];
await mkdir(directory+'/drivers');const copied=[];
for(const[index,path]of drivers.entries()){const bytes=await readFile(path);const name='drivers/'+index+'-'+path.split('/').at(-1);await writeFile(directory+'/'+name,bytes,{flag:'wx'});copied.push({path,name,sha256:digest(bytes)});}
const confirmation=JSON.parse(await readFile('evals/runs/0066-tcommit-confirmation/manifest.json','utf8'));assert.equal(digest(await readFile('.artifacts/0066-confirm.ts')),confirmation.driverSha256);
await writeFile(directory+'/unformatted-iteration.md',await readFile('.artifacts/0066-iteration-unformatted.md'),{flag:'wx'});
await writeFile(directory+'/function-before.rs',await readFile('.artifacts/0066-function-before.rs'),{flag:'wx'});
await mkdir(directory+'/logs');const logs=[];
for(const name of(await readdir('.artifacts')).filter(name=>name.startsWith('0066-')&&name.endsWith('.log')).sort()){const bytes=await readFile('.artifacts/'+name);const archive=gzipSync(bytes);await writeFile(directory+'/logs/'+name+'.gz',archive,{flag:'wx'});logs.push({name,sha256:digest(bytes),gzipSha256:digest(archive)});}
const runs=(await readdir('evals/runs')).filter(name=>name.startsWith('0066-')&&name!=='0066-audit').sort();
await save('result.json',{status:'complete',plan:'0066-domain-offset',baseline:'54f39529',planCommit:'7d48352a',snapshots,drivers:copied,runs,logs,verification:{first:'0066-verification',failed:'just check: new iteration table format',passedAfterFix:'0066-check-retry',otherChecks:20,productionSourceUnchanged:true},tlcSource:{revision:'d8fb231de4e4b5d02c1b2ed1ac759c086f694021',path:'tlc2/value/impl/FcnRcdValue.java',referenceJar:'1.7.4 5a47802'}});
await appendFile('evals/runs/ledger.jsonl',JSON.stringify({runId:'0066-audit',kind:'source-audit',status:'complete'})+'\n');
