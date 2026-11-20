import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {appendFile,mkdir,readdir,readFile,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {gzipSync} from 'node:zlib';
const directory='evals/runs/0065-audit';
await mkdir(directory);
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const save=(path,value)=>writeFile(join(directory,path),JSON.stringify(value,null,2)+'\n',{flag:'wx'});
const snapshots=[];
for(const name of ['before','after','final']){
 const root='.artifacts/0065-'+name;
 const bytes=await readFile(join(root,'manifest.json'));
 const manifest=JSON.parse(bytes);
 for(const file of manifest.files)assert.equal(digest(await readFile(join(root,file.path))),file.sha256);
 const archive=gzipSync(await readFile(join(root,'source.tar')));
 await writeFile(join(directory,name+'-source.tar.gz'),archive,{flag:'wx'});
 await save(name+'-manifest.json',manifest);
 snapshots.push({name,manifestSha256:digest(bytes),archiveSha256:digest(archive),files:manifest.files});
}
const symmetry=[];
for(const name of ['before','after']){
 const root='.artifacts/0065-'+name+'-sym';
 const bytes=await readFile(join(root,'symmetry-bench'));
 const metadata=JSON.parse(await readFile(join(root,'criterion.json'),'utf8'));
 assert.equal(digest(bytes),metadata.binarySha256);
 await save(name+'-symmetry.json',metadata);
 const archive=gzipSync(await readFile(join(root,'source.tar')));
 await writeFile(join(directory,name+'-symmetry-source.tar.gz'),archive,{flag:'wx'});
 symmetry.push({name,...metadata,archiveSha256:digest(archive)});
}
const scratch='.artifacts/0065-reconstructed';
await mkdir(scratch);
execFileSync('tar',['-xf',resolve('.artifacts/0065-before/source.tar'),'-C',scratch]);
const path='crates/rtlc/src/safety_symmetry.rs';
const original=execFileSync('git',['show','a42670fb:'+path]).toString();
const current=await readFile(path,'utf8');
const marker='#[cfg(test)]';
assert.ok(original.includes(marker)&&current.includes(marker));
await writeFile(join(scratch,path),original.slice(0,original.indexOf(marker))+current.slice(current.indexOf(marker)));
const paths=JSON.parse(await readFile('.artifacts/0065-before/manifest.json','utf8')).sources.map(source=>source.path);
execFileSync('tar',['-cf',resolve('.artifacts/0065-reconstructed.tar'),'-C',scratch,'--',...paths]);
const reconstructed=gzipSync(await readFile('.artifacts/0065-reconstructed.tar'));
await writeFile(join(directory,'before-symmetry-with-tests.tar.gz'),reconstructed,{flag:'wx'});
await save('worker-diagnosis.json',JSON.parse(await readFile('.artifacts/0065-worker-diagnosis.json','utf8')));
const drivers=[
 'evals/oracle-tuning/kernel.ts','evals/evaluator-tuning/summarize.ts',
 'evals/symmetry/allocation.ts','evals/symmetry/evaluate.ts',
 'evals/p0-checker/verify.ts','evals/temporal-tuning/parity.ts',
 '.artifacts/0065-allocation-first.ts','.artifacts/0065-allocation-workers.ts',
 '.artifacts/0065-allocation-measured.ts','.artifacts/0065-archive.ts',
];
await mkdir(join(directory,'drivers'));
const copied=[];
for(const[index,path]of drivers.entries()){
 const bytes=await readFile(path);
 const name='drivers/'+index+'-'+path.split('/').at(-1);
 await writeFile(join(directory,name),bytes,{flag:'wx'});
 copied.push({path,name,sha256:digest(bytes)});
}
const first=JSON.parse(await readFile('evals/runs/0065-boundary/manifest.json','utf8'));
assert.equal(digest(await readFile('.artifacts/0065-allocation-first.ts')),first.driverSha256);
const second=JSON.parse(await readFile('evals/runs/0065-boundary-fixed/manifest.json','utf8'));
assert.equal(digest(await readFile('.artifacts/0065-allocation-workers.ts')),second.driverSha256);
const measured=JSON.parse(await readFile('evals/runs/0065-boundary-options/manifest.json','utf8'));
assert.equal(digest(await readFile('.artifacts/0065-allocation-measured.ts')),measured.driverSha256);
const confirmed=JSON.parse(await readFile('evals/runs/0065-boundary-confirmation/manifest.json','utf8'));
assert.equal(digest(await readFile('evals/symmetry/allocation.ts')),confirmed.driverSha256);
await mkdir(join(directory,'logs'));
const logs=[];
for(const name of(await readdir('.artifacts')).filter(name=>name.startsWith('0065-')&&name.endsWith('.log')).sort()){
 const bytes=await readFile(join('.artifacts',name));
 const packed=gzipSync(bytes);
 await writeFile(join(directory,'logs',name+'.gz'),packed,{flag:'wx'});
 logs.push({name,sha256:digest(bytes),gzipSha256:digest(packed)});
}
const runs=(await readdir('evals/runs')).filter(name=>name.startsWith('0065-')&&name!=='0065-audit').sort();
await save('result.json',{
 status:'complete',plan:'0065-symmetry-allocation',baseline:'a42670fb',planCommit:'142cf22a',
 snapshots,symmetry,runs,logs,drivers:copied,
 beforeSymmetrySource:{
  archive:'before-symmetry-with-tests.tar.gz',sha256:digest(reconstructed),reconstructed:true,
  note:'Baseline production implementation from a42670fb plus the two characterization tests present at the baseline Criterion build. The original before-symmetry-source archive contains the unchanged baseline production implementation without those tests.'
 },
 tlcSource:{revision:'d8fb231de4e4b5d02c1b2ed1ac759c086f694021',paths:['TLCStateMut.java','impl/Tool.java'],referenceJar:'TLC 1.7.4 5a47802'},
});
await appendFile('evals/runs/ledger.jsonl',JSON.stringify({runId:'0065-audit',kind:'source-audit',status:'complete'})+'\n');
