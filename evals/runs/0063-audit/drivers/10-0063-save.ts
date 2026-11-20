import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readdir, readFile, writeFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';

const root = 'evals/runs/0063-audit';
const digest = (bytes: Uint8Array | string) => createHash('sha256').update(bytes).digest('hex');
const runs = (await readdir('evals/runs')).filter(name => name.startsWith('0063-') && name !== '0063-audit').sort();
await mkdir(root);
await mkdir(join(root, 'sources'));
await mkdir(join(root, 'logs'));
await mkdir(join(root, 'drivers'));
const save = (path: string, value: unknown) => writeFile(join(root,path), JSON.stringify(value,null,2)+'\n',{flag:'wx'});
const snapshots = [];
for (const stage of ['before','a','a-full','b','c','final']) {
  const path = '.artifacts/0063-'+stage;
  const manifest = await readFile(join(path,'manifest.json'));
  await writeFile(join(root,'sources',stage+'.manifest.json'),manifest,{flag:'wx'});
  const source = gzipSync(await readFile(join(path,'source.tar')));
  await writeFile(join(root,'sources',stage+'.tar.gz'),source,{flag:'wx'});
  const metadata = JSON.parse(manifest.toString());
  for(const file of metadata.files) assert.equal(digest(await readFile(join(path,file.path))),file.sha256);
  snapshots.push({stage,manifestSha256:digest(manifest),sourceSha256:digest(source),files:metadata.files});
}
const logs = [];
for (const name of (await readdir('.artifacts')).filter(name => name.startsWith('0063-') && name.endsWith('.log') && name !== '0063-audit.log').sort()) {
  const bytes = await readFile(join('.artifacts',name));
  await writeFile(join(root,'logs',name),bytes,{flag:'wx'});
  logs.push({name,sha256:digest(bytes)});
}
const driverPaths = [
  ... (await readdir('evals/evaluator-tuning')).map(name => 'evals/evaluator-tuning/'+name),
  'evals/p0-checker/corpus.ts','evals/p0-checker/profile.ts','evals/p0-checker/verify.ts',
  'evals/oracle-tuning/kernel.ts','scripts/check-support.ts','justfile',
  'evals/plans/0063-evaluator-tuning.md','evals/iterations/0063-evaluator-tuning.md',
  '.artifacts/0063-save.ts',
];
const drivers = [];
for (const [index,path] of driverPaths.entries()) {
  const bytes = await readFile(path);
  const name = String(index)+'-'+path.split('/').at(-1);
  await writeFile(join(root,'drivers',name),bytes,{flag:'wx'});
  drivers.push({path,archive:'drivers/'+name,sha256:digest(bytes)});
}
const failedManifest = JSON.parse(await readFile('evals/runs/0063-corpus-selected/manifest.json','utf8'));
const failedDriver = (await readFile('evals/p0-checker/corpus.ts','utf8')).replaceAll('route.name.startsWith("before-") && route.version === 1','route.name.startsWith("before-")');
const reconstructed = digest(failedDriver) === failedManifest.driverSha256;
if (reconstructed) await writeFile(join(root,'drivers','corpus-failed.ts'),failedDriver,{flag:'wx'});
const statuses = [];
for (const name of runs) {
  const resultPath = join('evals/runs',name,'result.json');
  const errorPath = join('evals/runs',name,'error.json');
  if (existsSync(resultPath)) {
    const bytes=await readFile(resultPath);
    statuses.push({name,status:JSON.parse(bytes.toString()).status,sha256:digest(bytes)});
  } else if (existsSync(errorPath)) {
    const bytes=await readFile(errorPath);
    statuses.push({name,status:'failed',sha256:digest(bytes)});
  } else {
    assert.ok(name.startsWith('0063-kernel-'));
    for (const index of [0,1,2,3]) assert.ok((await stat(join('evals/runs',name,String(index),'stdout.log'))).size>0);
    statuses.push({name,status:'complete',scope:'Four Criterion repetitions; corresponding summary validates expected counts.'});
  }
}
const verification=JSON.parse(await readFile('evals/runs/0063-verification/result.json','utf8'));
assert.equal(verification.status,'complete');
assert.equal(verification.unchanged,true);
assert.equal(verification.results.length,21);
await save('result.json',{status:'complete',plan:'0063-evaluator-tuning',baseline:'162fd73d',planCommit:'3e804b5f',runs:statuses,snapshots,logs,drivers,reconstructedFailedDriver:reconstructed,verification:{status:verification.status,unchanged:verification.unchanged,checks:verification.results.length},limitations:'Six source archives include a/a-full variants. Intermediate failed development drivers are not all archived. The before-v2 omission driver is reconstructed only when its exact recorded hash matches. Profile sampling and normal timing are separate.'});
const ledger='evals/runs/ledger.jsonl';
const previous=(await readFile(ledger,'utf8')).trim().split('\n').map(line=>JSON.parse(line));
const known=new Set(previous.map(row=>row.runId));
const additions=[...statuses.filter(row=>!known.has(row.name)).map(row=>({runId:row.name,plan:'0063-evaluator-tuning',kind:'audit-reference',status:row.status})),{runId:'0063-audit',plan:'0063-evaluator-tuning',kind:'audit',status:'complete'}];
await writeFile(ledger,additions.map(row=>JSON.stringify(row)+'\n').join(''),{flag:'a'});
console.log(JSON.stringify({runs:statuses.length,snapshots:snapshots.length,logs:logs.length,reconstructedFailedDriver:reconstructed}));
