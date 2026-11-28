import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  writeFile,
} from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { monotone, recordNodes } from "../evals/experiments/symmetry/cases.js";
import { compileTlaSafety } from "../packages/idealis/src/tla-safety.js";

const directory = resolve(process.argv[2] ?? ".artifacts/packages");
const version = JSON.parse(
  await readFile("packages/idealis/package.json", "utf8"),
).version;
const tarball = `mizchi-idealis-${version}.tgz`;
const crate = `idealis-core-${version}.crate`;
const rtlcCrate = `rtlc-${version}.crate`;
const rtlaCrate = `rtla-${version}.crate`;
const compilerCrate = `rtla-compiler-${version}.crate`;
const rusmtCrate = `rusmt-${version}.crate`;
const ralloyCrate = `ralloy-${version}.crate`;
const cliCrate = `idealis-cli-${version}.crate`;
const connectCrate = `idealis-connect-${version}.crate`;
const oracleCrate = `roracle-${version}.crate`;
const consumer = await mkdtemp(join(tmpdir(), "idealis-consumer-"));
await writeFile(
  join(consumer, "package.json"),
  JSON.stringify({ name: "idealis-consumer", private: true, type: "module" }),
);
execFileSync(
  "pnpm",
  [
    "add",
    join(directory, tarball),
    "--offline",
    "--store-dir",
    join(consumer, ".pnpm-store"),
    "--ignore-scripts",
    "--lockfile=false",
    "--config.auto-install-peers=false",
    "--config.node-linker=hoisted",
  ],
  { cwd: consumer, stdio: "pipe", encoding: "utf8" },
);
const nodeTypes = await realpath("node_modules/@types/node");
const nodeRequire = createRequire(join(nodeTypes, "package.json"));
await mkdir(join(consumer, "node_modules/@types"), { recursive: true });
await cp(nodeTypes, join(consumer, "node_modules/@types/node"), {
  recursive: true,
});
await cp(
  dirname(nodeRequire.resolve("undici-types/package.json")),
  join(consumer, "node_modules/undici-types"),
  { recursive: true },
);
const artifact = {
  format: "modelt.finite-model",
  version: 1,
  name: "package-smoke",
  initial: [{ key: "0", observation: { type: "int", value: "0" } }],
  states: [
    {
      key: "0",
      violations: [],
      transitions: [
        {
          action: { type: "string", value: "tick" },
          target: "1",
          observation: { type: "int", value: "1" },
        },
      ],
    },
    { key: "1", violations: [], transitions: [] },
  ],
};
const history = {
  format: "modelt.portable-trace",
  version: 1,
  model: "package-smoke",
  initial: { type: "int", value: "0" },
  steps: [
    {
      action: { type: "string", value: "tick" },
      observation: { type: "int", value: "1" },
    },
  ],
};
await writeFile(join(consumer, "model.json"), JSON.stringify(artifact));
await writeFile(join(consumer, "history.json"), JSON.stringify(history));
const sourceInput = {
  source:
    "---- MODULE PackageSafety ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nSafe == x < 3\n====",
  config: "INIT Init\nNEXT Next\nINVARIANT Safe",
};
const safety = compileTlaSafety(sourceInput);
await writeFile(join(consumer, "source.json"), JSON.stringify(sourceInput));
await writeFile(
  join(consumer, "reductions.json"),
  JSON.stringify([recordNodes(3), monotone(3)]),
);
await writeFile(
  join(consumer, "safety-request.json"),
  JSON.stringify({
    version: 1,
    request: {
      kind: "check-safety",
      program: safety,
      bounds: { maxDepth: 4, maxStates: 5, maxWork: 100 },
    },
  }),
);
await writeFile(
  join(consumer, "request.json"),
  JSON.stringify({
    version: 1,
    request: { kind: "check-finite", model: artifact, traces: [history] },
  }),
);
await writeFile(
  join(consumer, "consumer.mjs"),
  `import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createTestingWasm} from '@mizchi/idealis/testing';
import {createAlloyWasm,AlloyError} from '@mizchi/idealis/alloy';
import {compileTlaSource,checkTlaSource,createPortableWasm,checkFiniteHistories,decodePortableTrace,loadFiniteModel,checkSafety,checkSafetyCoverage,prepareSafetyProgram} from '@mizchi/idealis/portable';
import {createHistoryRecorder} from '@mizchi/idealis/history';
import {parseFiniteModel} from '@mizchi/idealis/finite';
import {createSimulationHost} from '@mizchi/idealis/host';
import {planSchedule,runSchedule} from '@mizchi/idealis/scheduling';
import {createRtlcBackend,runBackend,parseBackendResult} from '@mizchi/idealis/backend';
const testing=await createTestingWasm(await readFile(new URL(import.meta.resolve('@mizchi/idealis/testing.wasm'))));
const schedules=await testing.schedule({id:'consumer-actors',version:'1',actors:[{id:'left',operations:['read','write']},{id:'right',operations:['read','write']}]});assert.equal(schedules.maxLength,4);assert.equal(Object.keys(schedules.states).length,9);
const testingPlan={generator:{id:'consumer',version:'1',initial:['s'],states:{s:[{action:'good',target:'s'},{action:'bad',target:'s'}]},minLength:0,maxLength:8},seed:0x36a1,cases:32,maxAttempts:64,maxShrinkChecks:64,maxWork:10000};
const predicate=testCase=>testCase.actions.includes('bad')?{status:'mismatch',signature:'bad'}:{status:'matched'};
const testingResult=await testing.property(testingPlan,predicate);assert.equal(testingResult.status,'failed');assert.deepEqual(testingResult.counterexample.actions,['bad']);assert.equal(testingResult.shrink.status,'minimal');
assert.equal((await testing.replay(testingResult.counterexample,predicate,'bad')).status,'failed');assert.equal((await testing.property({...testingPlan,maxWork:0},predicate)).status,'inconclusive');
assert.equal((await testing.compare({kind:'allowed',values:[1]},[{id:'candidate',outcome:{kind:'value',value:2}}])).status,'mismatch');
const alloy=await createAlloyWasm(await readFile(new URL(import.meta.resolve('@mizchi/idealis/ralloy.wasm'))));
const alloyInput={source:'sig N { edge: set N } fact { no iden & edge } assert Safe { no iden & ^edge } check Safe for 2 but exactly 2 N, 0 Int',command:0,maxWork:100000};
const generated=alloy.analyze(alloyInput);assert.equal(generated.status,'counterexample');assert.equal(alloy.checkInstance(alloyInput,generated.instance).status,'mismatch');
assert.equal(alloy.checkInstance(alloyInput,{signatures:{N:['a','b']},fields:{edge:[['a','b']]}}).status,'matched');
assert.equal(alloy.checkInstance({...alloyInput,maxWork:0},generated.instance).status,'unknown');
assert.throws(()=>alloy.checkInstance(alloyInput,{signatures:{N:['a','b']},fields:{}}),AlloyError);
const core=await createPortableWasm(await readFile(new URL(import.meta.resolve('@mizchi/idealis/core.wasm'))));
import {createCheckCache,createMemoryCheckCacheStore,runCachedCheck} from '@mizchi/idealis/check-cache';
import {createHttpCheckCacheStore} from '@mizchi/idealis/check-cache/http';
import {createSqliteCheckCacheStore,createCheckCacheHttpHandler} from '@mizchi/idealis/check-cache/node';
void createMemoryCheckCacheStore;void createHttpCheckCacheStore;void createCheckCacheHttpHandler;
const cacheStore=createSqliteCheckCacheStore(':memory:');
const cache=createCheckCache(cacheStore);
const cacheBackend=createRtlcBackend(core,{version:'consumer-cache'});
const cacheJob={buildDigest:'c'.repeat(64),backend:cacheBackend.descriptor,request:{format:'modelt.backend-request',version:1,scope:{modelId:'consumer',propertyId:'Safe',semantics:'tlc-1.7.4-int32',assumptions:[]},query:{kind:'explicit',bounds:{maxDepth:8,maxStates:100,maxWork:10000}},input:compileTlaSource(core,{source:"---- MODULE CacheConsumer ----\\nVARIABLE x\\nInit == x=0\\nNext == UNCHANGED x\\nSafe == x=0\\n====",config:"INIT Init\\nNEXT Next\\nINVARIANT Safe"})}};
assert.equal((await runCachedCheck(cache,cacheJob,cacheBackend,{owner:'consumer',runId:'cold',leaseMs:100,now:()=>0})).kind,'executed');
assert.equal((await runCachedCheck(cache,cacheJob,cacheBackend,{owner:'consumer',runId:'warm',leaseMs:100,now:()=>1})).kind,'cached');
import {createSafetyExpansionCache,checkSafetyIncremental,mergeSafetyExpansions} from '@mizchi/idealis/portable';
const expansionOptions={enabled:true,buildDigest:'d'.repeat(64),maxEntries:100};
const expanded=checkSafetyIncremental(core,cacheJob.request.input,{maxDepth:8,maxStates:100,maxWork:10000},{...expansionOptions,cache:createSafetyExpansionCache()});
const reused=checkSafetyIncremental(core,cacheJob.request.input,{maxDepth:8,maxStates:100,maxWork:10000},{...expansionOptions,cache:JSON.parse(JSON.stringify(expanded.cache))});
assert.equal(reused.result.status,'complete');assert.equal(reused.stats.hits,1);assert.equal(reused.stats.misses,0);
assert.equal(Object.keys(mergeSafetyExpansions(core,expanded.cache,reused.cache,100).entries).length,1);
cacheStore.close();

const parserModule=await WebAssembly.compile(await readFile(new URL(import.meta.resolve('@mizchi/idealis/rtla.wasm'))));
assert.deepEqual(WebAssembly.Module.imports(parserModule),[]);
const parser=(await WebAssembly.instantiate(parserModule,{})).exports;
assert.equal(parser.rtla_abi_version(),1);
const parserInput=new TextEncoder().encode(JSON.stringify({kind:'expression',source:"x'"}));
const parserPointer=parser.rtla_input_resize(parserInput.length);
new Uint8Array(parser.memory.buffer,parserPointer,parserInput.length).set(parserInput);
parser.rtla_dispatch();
const parsed=JSON.parse(new TextDecoder().decode(new Uint8Array(parser.memory.buffer,parser.rtla_output_ptr(),parser.rtla_output_len())));
assert.equal(parsed.expression.kind,'Prime');assert.equal(parsed.expression.expression.name,'x');
const model=parseFiniteModel(JSON.parse(await readFile('model.json','utf8')));
const trace=decodePortableTrace(await readFile('history.json','utf8'));
assert.equal(checkFiniteHistories(core,model,[trace])[0].status,'matched');
const loaded=loadFiniteModel(core,model);
const recorded=createHistoryRecorder(model.name,trace.initial);
recorded.invoke('tick',trace.steps[0].action);recorded.returned('tick',trace.steps[0].observation);
assert.equal(loaded.checkHistory(recorded.snapshot(),100).status,'linearizable');loaded.release();
assert.equal(createSimulationHost().now(),0);
const safety=JSON.parse(await readFile('safety-request.json','utf8')).request;
const backendResult=await runBackend(createRtlcBackend(core,{version:'0.1.0-alpha.0'}),{format:'modelt.backend-request',version:1,scope:{modelId:'package-safety-v1',propertyId:'Safe-v1',semantics:'tlc-1.7.4-int32',assumptions:[]},query:{kind:'explicit',bounds:safety.bounds},input:safety.program});
assert.equal(backendResult.outcome.kind,'counterexample');assert.equal(backendResult.outcome.witness.kind,'behavior');
assert.deepEqual(parseBackendResult(JSON.parse(JSON.stringify(backendResult))),backendResult);
assert.equal(checkSafety(core,safety.program,safety.bounds).violation.trace.length,4);
assert.deepEqual(checkSafetyCoverage(core,safety.program,safety.bounds).result,checkSafety(core,safety.program,safety.bounds));
const source=JSON.parse(await readFile('source.json','utf8'));
const sourceProgram=compileTlaSource(core,source);assert.equal(sourceProgram.name,'PackageSafety');assert.equal(checkTlaSource(core,source,safety.bounds).violation.trace.length,4);
const sharedProgram=compileTlaSource(core,{...source,irVersion:2});assert.equal(sharedProgram.version,2);assert.equal(checkSafety(core,sharedProgram,safety.bounds).violation.trace.length,4);
for(const program of [safety.program,sharedProgram]){const prepared=prepareSafetyProgram(core,program);const expected=checkSafety(core,program,safety.bounds);assert.deepEqual(prepared.check(safety.bounds),expected);assert.deepEqual(prepared.checkBatch([{bounds:safety.bounds},{bounds:safety.bounds}]),[{kind:'safety',result:expected},{kind:'safety',result:expected}]);prepared.release();assert.throws(()=>prepared.check(safety.bounds),{code:'protocol'});}
const reductions=JSON.parse(await readFile('reductions.json','utf8'));
for(const [index,input] of reductions.entries()){const program=compileTlaSource(core,input);const result=checkSafety(core,program,{maxDepth:100,maxStates:10000,maxWork:10000000},index===0?{symmetryActions:true}:{partialOrder:true});assert.equal(result.status,'complete');assert.equal(result.states,index===0?56:7);assert.equal(result.generated,index===0?253:8);}
const planned=planSchedule([{id:'safety',estimatedMs:1,memoryBytes:1}],[{id:'serial',workers:1,startupMs:0,costScale:1,fixedMemoryBytes:0}],{maxWorkers:1,durationMs:10000,memoryBytes:1024},{policy:'cost',batchTargetMs:4,maxBatchSize:16});
const scheduled=await runSchedule(planned,[{execute:async ids=>ids.map(id=>({id,result:checkSafety(core,safety.program,safety.bounds)}))}]);
assert.equal(scheduled.executionStatus,'complete');assert.equal(scheduled.completed[0].result.status,'invariant-violation');
for(const name of ['z3-solver','tla-ts','fast-check'])assert.throws(()=>import.meta.resolve(name));
console.log('packaged TS/Wasm consumer matched without solvers');
`,
);
const node = execFileSync(process.execPath, [join(consumer, "consumer.mjs")], {
  cwd: consumer,
})
  .toString()
  .trim();
await cp(
  "tests/package/pattern-consumer.mts",
  join(consumer, "pattern-consumer.mts"),
);
await cp(
  "examples/model-patterns/semaphore/weighted-fifo.model",
  join(consumer, "weighted-fifo.model"),
);
await cp("crates/rcheck/target/debug/rcheck", join(consumer, "rcheck"));
const patterns = execFileSync(
  process.execPath,
  ["pattern-consumer.mts", "./rcheck"],
  { cwd: consumer, encoding: "utf8" },
).trim();
await writeFile(
  join(consumer, "consumer.ts"),
  `import type {Model} from '@mizchi/idealis';
import {createCheckCache,type CheckCacheJob,type CachedCheckSchedule} from '@mizchi/idealis/check-cache';
import {createHttpCheckCacheStore} from '@mizchi/idealis/check-cache/http';
import {createSqliteCheckCacheStore} from '@mizchi/idealis/check-cache/node';
const cacheType:(input:CheckCacheJob)=>void=()=>{};const scheduleType:(input:CachedCheckSchedule)=>void=()=>{};
void createCheckCache;void createHttpCheckCacheStore;void createSqliteCheckCacheStore;void cacheType;void scheduleType;

import {createTestingWasm,type TestingCase,type TestingSchedule} from '@mizchi/idealis/testing';
const schedule:TestingSchedule={id:'typecheck',version:'1',actors:[{id:'actor',operations:['step']}]};void schedule;
const testingCase:TestingCase={format:'idealis.test-case',version:1,rng:'xorshift32-v1',generatorId:'g',generatorVersion:'1',seed:1,index:0,actions:[]};void testingCase;void createTestingWasm;
import {createAlloyWasm,type AlloyInstance} from '@mizchi/idealis/alloy';
const observed:AlloyInstance={signatures:{N:[]},fields:{edge:[]}};void observed;void createAlloyWasm;
import {compileFiniteModel,createFiniteModel} from '@mizchi/idealis/finite';
import {createSimulationHost} from '@mizchi/idealis/host';
import {createPortableWasm,compileTlaSource,checkTlaSource,checkSafetyCoverage,prepareSafetyProgram,type PreparedSafetyProgram,type SafetyCheck,type SafetyBatchResult,type SafetyCoverageReport} from '@mizchi/idealis/portable';
import {createSafetyExpansionCache,checkSafetyIncremental,mergeSafetyExpansions,type SafetyExpansionCache} from '@mizchi/idealis/portable';
const expansionTypes:SafetyExpansionCache=createSafetyExpansionCache();void expansionTypes;void checkSafetyIncremental;void mergeSafetyExpansions;
import {planSchedule} from '@mizchi/idealis/scheduling';
import {createHistoryRecorder,checkConcurrentHistory} from '@mizchi/idealis/history';
import {createRtlcBackend,runBackend,parseBackendResult,type BackendResult} from '@mizchi/idealis/backend';
const model:Model<number,'tick',number>={name:'typed',initial:()=>[{state:0,observation:0}],actions:()=>['tick'],step:()=>[{state:0,observation:0}],key:String};
const compiled=compileFiniteModel(model,{maxDepth:1,maxStates:1});
if(compiled.model){createFiniteModel(compiled.model);checkConcurrentHistory(compiled.model,createHistoryRecorder(model.name,{type:'int',value:'0'}).snapshot(),100);}
createSimulationHost({seed:1});
void createPortableWasm;void compileTlaSource;void checkTlaSource;void checkSafetyCoverage;
const preparedTypes=(prepared:PreparedSafetyProgram,jobs:readonly SafetyCheck[]):readonly SafetyBatchResult[]=>prepared.checkBatch(jobs);void preparedTypes;void prepareSafetyProgram;
const coverageCount=(report:SafetyCoverageReport)=>report.coverage.next.reduce((total,row)=>total+row.generated,0);void coverageCount;
void planSchedule;
const backendTypes:(value:unknown)=>BackendResult=parseBackendResult;
void backendTypes;void createRtlcBackend;void runBackend;
`,
);
execFileSync(
  process.execPath,
  [
    resolve("node_modules/typescript/bin/tsc"),
    "--noEmit",
    "--strict",
    "--target",
    "ES2023",
    "--module",
    "NodeNext",
    "--types",
    "node",
    "consumer.ts",
    "pattern-consumer.mts",
  ],
  { cwd: consumer, stdio: "pipe" },
);
const cli = join(consumer, "node_modules/.bin/idealis");
const checked = spawnSync(
  cli,
  ["check", join(consumer, "model.json"), join(consumer, "history.json")],
  { encoding: "utf8" },
);
assert.equal(checked.status, 0, checked.stderr);
assert.equal(JSON.parse(checked.stdout).status, "matched");
await writeFile(
  join(consumer, "history.json"),
  JSON.stringify({
    ...history,
    steps: [{ ...history.steps[0], observation: { type: "int", value: "2" } }],
  }),
);
assert.equal(
  spawnSync(cli, [
    "check",
    join(consumer, "model.json"),
    join(consumer, "history.json"),
  ]).status,
  1,
);
await writeFile(
  join(consumer, "history.json"),
  JSON.stringify({
    ...history,
    steps: [
      { ...history.steps[0], action: { type: "string", value: "unknown" } },
    ],
  }),
);
assert.equal(
  spawnSync(cli, [
    "check",
    join(consumer, "model.json"),
    join(consumer, "history.json"),
  ]).status,
  2,
);
await writeFile(join(consumer, "PackageSafety.tla"), sourceInput.source);
await writeFile(join(consumer, "PackageSafety.cfg"), sourceInput.config);
for (const [args, status, diagnostic] of [
  [["tlc", "-workers", "1", "PackageSafety"], 12, "Safe"],
  [
    ["tla", join(consumer, "PackageSafety.tla")],
    0,
    "Semantic processing of module PackageSafety",
  ],
] as const) {
  const output = spawnSync(cli, [...args], { cwd: consumer, encoding: "utf8" });
  assert.equal(output.status, status, output.stdout + output.stderr);
  assert.ok(output.stdout.includes(diagnostic), output.stdout);
}
execFileSync("tar", ["-xzf", join(directory, crate), "-C", consumer]);
execFileSync("tar", ["-xzf", join(directory, rtlcCrate), "-C", consumer]);
execFileSync("tar", ["-xzf", join(directory, rtlaCrate), "-C", consumer]);
execFileSync("tar", ["-xzf", join(directory, compilerCrate), "-C", consumer]);
for (const archive of [
  crate,
  rtlcCrate,
  rtlaCrate,
  compilerCrate,
  rusmtCrate,
  ralloyCrate,
  connectCrate,
  cliCrate,
  oracleCrate,
]) {
  const entries = execFileSync("tar", ["-tzf", join(directory, archive)])
    .toString()
    .split("\n");
  assert.ok(
    entries.some((entry) => entry.endsWith("/SUPPORT.md")),
    `${archive}: missing capability checklist`,
  );
}
await writeFile(
  join(consumer, "Cargo.toml"),
  `[package]\nname="idealis-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nidealis-core={path="idealis-core-${version}"}\nrtlc={path="rtlc-${version}"}\nserde_json="1.0"\n[patch.crates-io]\nrtlc={path="rtlc-${version}"}\nrtla={path="rtla-${version}"}\nrtla-compiler={path="rtla-compiler-${version}"}\n[[bin]]\nname="consumer"\npath="consumer.rs"\n`,
);
await writeFile(
  join(consumer, "consumer.rs"),
  `use idealis_core::{Engine,Message,Status};
fn main(){let source:serde_json::Value=serde_json::from_slice(&std::fs::read("source.json").unwrap()).unwrap();let program=idealis_core::compile_tla(source["source"].as_str().unwrap(),source["config"].as_str().unwrap()).unwrap();let checked=program.check(idealis_core::SafetyBounds{max_depth:10,max_states:100,max_work:10000,max_temporal_nodes:None}).unwrap();assert_eq!(checked.violation.unwrap().trace.len(),4);let bytes=std::fs::read("request.json").unwrap();let response=Engine::default().dispatch(&bytes);match response{Message::Results{results}=>assert_eq!(results[0].status,Status::Matched),_=>panic!("unexpected result")};let bytes=std::fs::read("safety-request.json").unwrap();match Engine::default().dispatch(&bytes){Message::Safety{result}=>{assert_eq!(result.status,"invariant-violation");assert_eq!(result.violation.unwrap().trace.len(),4)},_=>panic!("missing safety result")};let command: idealis_core::Command = serde_json::from_slice(&bytes).unwrap(); if let idealis_core::Request::CheckSafety { program, bounds, options } = command.request { let program: rtlc::SafetyProgram = program; assert_eq!(program.check_with_options(bounds, options.unwrap_or_default()).unwrap().status, "invariant-violation"); } else { panic!("missing typed program") }; let inputs:serde_json::Value=serde_json::from_slice(&std::fs::read("reductions.json").unwrap()).unwrap();for(index,input)in inputs.as_array().unwrap().iter().enumerate(){let program=idealis_core::compile_tla(input["source"].as_str().unwrap(),input["config"].as_str().unwrap()).unwrap();let result=program.check_with_options(rtlc::SafetyBounds{max_depth:100,max_states:10000,max_work:10000000,max_temporal_nodes:None},rtlc::SafetyOptions{symmetry_actions:index==0,partial_order:index==1,..Default::default()}).unwrap();assert_eq!(result.states,if index==0{56}else{7});assert_eq!(result.generated,if index==0{253}else{8});}let shared=idealis_core::compile_tla_with_options(source["source"].as_str().unwrap(),source["config"].as_str().unwrap(),idealis_core::CompileOptions{irVersion:idealis_core::IrVersion::V2}).unwrap();assert_eq!(shared.version,2);assert_eq!(shared.check(rtlc::SafetyBounds{max_depth:10,max_states:100,max_work:10000,max_temporal_nodes:None}).unwrap().violation.unwrap().trace.len(),4);println!("packaged Rust consumer matched");}
`,
);
const rust = execFileSync(
  "cargo",
  [
    "run",
    "--offline",
    "--quiet",
    "--manifest-path",
    join(consumer, "Cargo.toml"),
  ],
  { cwd: consumer, stdio: ["ignore", "pipe", "pipe"] },
)
  .toString()
  .trim();
const rtlcConsumer = join(consumer, "rtlc-only");
await mkdir(rtlcConsumer);
await writeFile(
  join(rtlcConsumer, "Cargo.toml"),
  `[package]\nname="rtlc-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nrtlc={path="../rtlc-${version}"}\nserde_json="1.0"\n[[bin]]\nname="consumer"\npath="main.rs"\n`,
);
await writeFile(
  join(rtlcConsumer, "main.rs"),
  `fn main(){let bytes=std::fs::read("safety-request.json").unwrap();let request:serde_json::Value=serde_json::from_slice(&bytes).unwrap();let program:rtlc::SafetyProgram=serde_json::from_value(request["request"]["program"].clone()).unwrap();let bounds:rtlc::SafetyBounds=serde_json::from_value(request["request"]["bounds"].clone()).unwrap();let result=program.check(bounds).unwrap();let report:rtlc::SafetyCoverageReport=program.check_with_coverage(bounds).unwrap();assert_eq!(report.result,result);assert!(!report.coverage.initial.is_empty());assert_eq!(result.status,"invariant-violation");assert_eq!(result.violation.unwrap().trace.len(),4);for version in [1,2]{let mut next=program.clone();next.version=version;if version==2{next.definitions=Some(vec![]);}let prepared:rtlc::PreparedSafetyProgram=next.prepare().unwrap();assert_eq!(prepared.check(bounds).unwrap().violation.unwrap().trace.len(),4);}println!("standalone rtlc consumer checked");}`,
);
const rtlc = execFileSync(
  "cargo",
  [
    "run",
    "--offline",
    "--quiet",
    "--manifest-path",
    join(rtlcConsumer, "Cargo.toml"),
  ],
  {
    cwd: consumer,
    env: { ...process.env, CARGO_TARGET_DIR: join(consumer, "target") },
    stdio: "pipe",
  },
)
  .toString()
  .trim();
const metadata = JSON.parse(
  execFileSync(
    "cargo",
    [
      "metadata",
      "--offline",
      "--format-version",
      "1",
      "--manifest-path",
      join(rtlcConsumer, "Cargo.toml"),
    ],
    { cwd: consumer, stdio: "pipe" },
  ).toString(),
);
assert.ok(
  metadata.packages.some((entry: { name: string }) => entry.name === "rtlc"),
);
assert.ok(
  !metadata.packages.some(
    (entry: { name: string }) => entry.name === "idealis-core",
  ),
  "rtlc must remain independent of the idealis host",
);
const parserConsumer = join(consumer, "rtla-only");
await mkdir(parserConsumer);
await writeFile(
  join(parserConsumer, "Cargo.toml"),
  `[package]\nname="rtla-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nrtla={path="../rtla-${version}"}\n[[bin]]\nname="consumer"\npath="main.rs"\n`,
);
await writeFile(
  join(parserConsumer, "main.rs"),
  `fn main(){let module=rtla::parse_module("---- MODULE Consumer ----\\nVARIABLE x\\nInit == x = 0\\n====").unwrap();assert_eq!(module.name,"Consumer");assert_eq!(module.definitions[0].name,"Init");let config=rtla::parse_config("INIT Init").unwrap();assert_eq!(config.init.as_deref(),Some("Init"));println!("standalone rtla consumer parsed");}`,
);
const rtla = execFileSync(
  "cargo",
  [
    "run",
    "--offline",
    "--quiet",
    "--manifest-path",
    join(parserConsumer, "Cargo.toml"),
  ],
  {
    cwd: consumer,
    env: { ...process.env, CARGO_TARGET_DIR: join(consumer, "target") },
    stdio: "pipe",
  },
)
  .toString()
  .trim();
const parserMetadata = JSON.parse(
  execFileSync(
    "cargo",
    [
      "metadata",
      "--offline",
      "--format-version",
      "1",
      "--manifest-path",
      join(parserConsumer, "Cargo.toml"),
    ],
    { cwd: consumer, stdio: "pipe" },
  ).toString(),
);
assert.ok(
  !parserMetadata.packages.some(
    (entry: { name: string }) =>
      entry.name === "idealis-core" || entry.name === "rtlc",
  ),
);
const smtConsumer = await mkdtemp(join(tmpdir(), "rusmt-consumer-"));
execFileSync("tar", ["-xzf", join(directory, rusmtCrate), "-C", smtConsumer]);
await writeFile(
  join(smtConsumer, "Cargo.toml"),
  `[package]\nname="rusmt-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nrusmt={path="rusmt-${version}"}\n[[bin]]\nname="consumer"\npath="main.rs"\n`,
);
await writeFile(
  join(smtConsumer, "main.rs"),
  await readFile(join(smtConsumer, `rusmt-${version}`, "examples", "ttl.rs")),
);
const rusmt = execFileSync("cargo", ["run", "--offline", "--quiet"], {
  cwd: smtConsumer,
  stdio: "pipe",
})
  .toString()
  .trim();
assert.equal(rusmt, "standalone rusmt consumer checked");
const smtMetadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--offline", "--format-version", "1"], {
    cwd: smtConsumer,
    stdio: "pipe",
  }).toString(),
);
assert.ok(
  smtMetadata.packages.some(
    (entry: { name: string }) => entry.name === "rusmt",
  ),
);
assert.ok(
  !smtMetadata.packages.some((entry: { name: string }) =>
    ["idealis-core", "rtlc", "rtla", "rtla-compiler", "z3", "z3-sys"].includes(
      entry.name,
    ),
  ),
  "rusmt must remain independent of idealis hosts, TLA frontends and Z3",
);
const alloyConsumer = await mkdtemp(join(tmpdir(), "ralloy-consumer-"));
execFileSync("tar", [
  "-xzf",
  join(directory, ralloyCrate),
  "-C",
  alloyConsumer,
]);
await writeFile(
  join(alloyConsumer, "Cargo.toml"),
  `[package]\nname="ralloy-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nralloy={path="ralloy-${version}"}\n[[bin]]\nname="consumer"\npath="main.rs"\n`,
);
await writeFile(
  join(alloyConsumer, "main.rs"),
  await readFile(
    join(alloyConsumer, `ralloy-${version}`, "examples", "graph.rs"),
  ),
);
const ralloy = execFileSync("cargo", ["run", "--offline", "--quiet"], {
  cwd: alloyConsumer,
  stdio: "pipe",
})
  .toString()
  .trim();
assert.equal(ralloy, "standalone ralloy consumer checked");
const alloyMetadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--offline", "--format-version", "1"], {
    cwd: alloyConsumer,
    stdio: "pipe",
  }).toString(),
);
assert.ok(
  alloyMetadata.packages.some(
    (entry: { name: string }) => entry.name === "ralloy",
  ),
);
assert.ok(
  !alloyMetadata.packages.some((entry: { name: string }) =>
    [
      "idealis-core",
      "rusmt",
      "rtla",
      "rtla-compiler",
      "rtlc",
      "z3",
      "z3-sys",
    ].includes(entry.name),
  ),
);
const alloyModule = await WebAssembly.compile(
  new Uint8Array(await readFile(join(directory, "ralloy.wasm"))).buffer,
);
assert.deepEqual(WebAssembly.Module.imports(alloyModule), []);
const testingConsumer = await mkdtemp(
  join(tmpdir(), "idealis-connect-consumer-"),
);
execFileSync("tar", [
  "-xzf",
  join(directory, connectCrate),
  "-C",
  testingConsumer,
]);
await writeFile(
  join(testingConsumer, "Cargo.toml"),
  `[package]\nname="testing-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nidealis-connect={path="idealis-connect-${version}"}\n[[bin]]\nname="consumer"\npath="main.rs"\n`,
);
await writeFile(
  join(testingConsumer, "main.rs"),
  await readFile(
    join(
      testingConsumer,
      `idealis-connect-${version}`,
      "examples",
      "counter.rs",
    ),
  ),
);
execFileSync("tar", ["-xzf", join(directory, cliCrate), "-C", testingConsumer]);
const cliManifest = join(
  testingConsumer,
  `idealis-cli-${version}`,
  "Cargo.toml",
);
execFileSync(
  "cargo",
  [
    "test",
    "--offline",
    "--manifest-path",
    cliManifest,
    ...["idealis-connect", "rtla", "rtlc", "rtla-compiler"].flatMap((name) => [
      "--config",
      `patch.crates-io.${name}.path=${JSON.stringify(join(name === "idealis-connect" ? testingConsumer : consumer, `${name}-${version}`))}`,
    ]),
  ],
  { cwd: testingConsumer, stdio: "pipe" },
);
const testing = execFileSync("cargo", ["run", "--offline", "--quiet"], {
  cwd: testingConsumer,
  stdio: "pipe",
})
  .toString()
  .trim();
assert.equal(testing, "standalone idealis-connect consumer checked");
await writeFile(
  join(testingConsumer, "main.rs"),
  await readFile(
    join(
      testingConsumer,
      `idealis-connect-${version}`,
      "examples",
      "schedule.rs",
    ),
  ),
);
const testingSchedule = execFileSync("cargo", ["run", "--offline", "--quiet"], {
  cwd: testingConsumer,
  stdio: "pipe",
})
  .toString()
  .trim();
assert.equal(testingSchedule, "standalone schedule generator checked");
const testingMetadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--offline", "--format-version", "1"], {
    cwd: testingConsumer,
    stdio: "pipe",
  }).toString(),
);
assert.ok(
  !testingMetadata.packages.some((entry: { name: string }) =>
    [
      "rtla",
      "rtlc",
      "rtla-compiler",
      "idealis-core",
      "rusmt",
      "ralloy",
      "loom",
      "z3",
      "z3-sys",
    ].includes(entry.name),
  ),
);
assert.deepEqual(
  WebAssembly.Module.imports(
    await WebAssembly.compile(
      new Uint8Array(await readFile(join(directory, "idealis-connect.wasm")))
        .buffer,
    ),
  ),
  [],
);
const oracleConsumer = await mkdtemp(join(tmpdir(), "roracle-consumer-"));
execFileSync("tar", [
  "-xzf",
  join(directory, oracleCrate),
  "-C",
  oracleConsumer,
]);
await writeFile(
  join(oracleConsumer, "Cargo.toml"),
  `[package]\nname="oracle-consumer"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nroracle={path="roracle-${version}"}\nserde_json="1.0"\n[[bin]]\nname="consumer"\npath="main.rs"\n`,
);
await writeFile(
  join(oracleConsumer, "main.rs"),
  await readFile(
    join(oracleConsumer, `roracle-${version}`, "examples", "comparison.rs"),
  ),
);
const oracle = execFileSync("cargo", ["run", "--offline", "--quiet"], {
  cwd: oracleConsumer,
  stdio: "pipe",
})
  .toString()
  .trim();
assert.equal(oracle, "standalone roracle consumer checked");
await writeFile(
  join(oracleConsumer, "main.rs"),
  await readFile(
    join(oracleConsumer, `roracle-${version}`, "examples", "observations.rs"),
  ),
);
const oracleJson = execFileSync("cargo", ["run", "--offline", "--quiet"], {
  cwd: oracleConsumer,
  stdio: "pipe",
})
  .toString()
  .trim();
assert.equal(oracleJson, "standalone oracle JSON consumer checked");
const oracleMetadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--offline", "--format-version", "1"], {
    cwd: oracleConsumer,
    stdio: "pipe",
  }).toString(),
);
assert.ok(
  !oracleMetadata.packages.some((entry: { name: string }) =>
    [
      "rtla",
      "rtlc",
      "rtla-compiler",
      "idealis-core",
      "idealis-connect",
      "z3",
      "z3-sys",
    ].includes(entry.name),
  ),
);
const result = {
  status: "complete",
  oracle,
  oracleJson,
  oracleDependencies: oracleMetadata.packages
    .map((entry: { name: string }) => entry.name)
    .sort(),
  node,
  patterns,
  typecheck: "passed without solver declarations",
  rust,
  rtlc,
  rtla,
  rusmt,
  ralloy,
  testing,
  testingSchedule,
  testingDependencies: testingMetadata.packages
    .map((entry: { name: string }) => entry.name)
    .sort(),
  ralloyDependencies: alloyMetadata.packages
    .map((entry: { name: string }) => entry.name)
    .sort(),
  rusmtDependencies: smtMetadata.packages
    .map((entry: { name: string }) => entry.name)
    .sort(),
  parserWasm: "packaged rtla Wasm parsed without solvers",
  cliExitCodes: [0, 1, 2],
  consumer,
  artifacts: [
    tarball,
    crate,
    rtlcCrate,
    rtlaCrate,
    compilerCrate,
    rusmtCrate,
    ralloyCrate,
    connectCrate,
    cliCrate,
    oracleCrate,
  ],
};
await writeFile(
  join(directory, "consumer-result.json"),
  `${JSON.stringify(result, null, 2)}\n`,
);
console.log(JSON.stringify(result));
