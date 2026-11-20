import assert from 'node:assert/strict';
import{createHash}from'node:crypto';
import{appendFile,mkdir,readFile,writeFile}from'node:fs/promises';
import{startNative}from'../dist/evals/usable/native.js';
import{bounds}from'../dist/evals/single-model/cases.js';
const directory='evals/runs/0066-tcommit-confirmation';
await mkdir(directory);
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const save=(name,value)=>writeFile(directory+'/'+name,JSON.stringify(value,null,2)+'\n',{flag:'wx'});
const roots=['.artifacts/0066-before','.artifacts/0066-after'];
const snapshots=[];
for(const root of roots){const manifest=JSON.parse(await readFile(root+'/manifest.json','utf8'));for(const file of manifest.files)assert.equal(digest(await readFile(root+'/'+file.path)),file.sha256);snapshots.push(manifest);}
const cores=roots.map(root=>startNative(root+'/idealis-core'));
const input={source:await readFile('evals/fixtures/tcommit/TCommit.tla','utf8'),config:await readFile('evals/fixtures/tcommit/TCommit.cfg','utf8')};
const driver=await readFile('.artifacts/0066-confirm.ts');
const records=[];
try{
 const fixtures=[];
 for(const irVersion of[1,2]){
  const compiled=await cores[0].request({version:1,request:{kind:'compile-tla',...input,irVersion}});
  assert.equal(compiled.kind,'compiled');assert.equal(compiled.program.version,irVersion);
  const command={version:1,request:{kind:'check-safety',program:compiled.program,bounds}};
  const expected=await cores[0].request(command);
  assert.equal(expected.kind,'safety');assert.equal(expected.result.status,'complete');assert.equal(expected.result.states,6816);assert.equal(expected.result.generated,53513);
  fixtures.push({irVersion,command,expected});
 }
 const routes=[0,1].flatMap(side=>fixtures.map(fixture=>({side,fixture})));
 await save('manifest.json',{plan:'0066-domain-offset',snapshots,input,bounds,warmup:1,samples:12,driverSha256:digest(driver),scope:'Reused native JSONL IR check API only; compile, assertion, disk and process startup excluded',fixtures});
 for(let iteration=0;iteration<13;iteration++){
  const rotated=routes.map((_,index)=>routes[(index+iteration)%routes.length]);
  for(const route of iteration%2?rotated.reverse():rotated){
   const started=performance.now();const response=await cores[route.side].request(route.fixture.command);const wallMs=performance.now()-started;
   assert.deepEqual(response,route.fixture.expected);
   records.push({iteration,warmup:iteration===0,side:route.side,irVersion:route.fixture.irVersion,wallMs,response});
  }
 }
 assert.equal(digest(await readFile('.artifacts/0066-confirm.ts')),digest(driver));
 const summary=routes.map(route=>{const values=records.filter(record=>!record.warmup&&record.side===route.side&&record.irVersion===route.fixture.irVersion).map(record=>record.wallMs).sort((left,right)=>left-right);assert.equal(values.length,12);return{side:route.side,irVersion:route.fixture.irVersion,minMs:values[0],medianMs:(values[5]+values[6])/2,maxMs:values[11]};});
 await save('result.json',{status:'complete',records,summary});
 await appendFile('evals/runs/ledger.jsonl',JSON.stringify({runId:'0066-tcommit-confirmation',kind:'domain-probe-confirmation',status:'complete'})+'\n');
 console.log(summary);
}catch(error){await save('error.json',{status:'error',error:String(error),records});throw error;}finally{for(const core of cores)await core.close();}
