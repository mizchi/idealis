# idealis

[日本語](README.md) · English

idealis is a portable toolkit for checking properties of a formal model and testing whether an implementation behaves as the model allows.

**Development status.** Version `0.1.0-alpha.0` is a prototype. The APIs, supported language fragments, and integration between backends are still evolving. The packages have yet to be published to npm or crates.io.

The design combines SMT for values and constraints with TLA+ for state transitions and concurrency. The goal is to connect model checking, oracle generation, and conformance testing to the test tools people already use in TypeScript, Rust, and Go. [Project roadmap](docs/vision.md)

## Compatibility and API stability

`rtla`, `rtla-compiler`, and `rtlc` aim to run more existing TLA+ models and TLC configurations unchanged and to make parsing, compilation, and checking faster. Similarly, `rusmt` aims to support more SMT-LIB inputs and improve solver performance. Compatibility concerns which inputs a component accepts and whether their meaning and results agree with the reference implementation. Each crate documents the subset it supports today. [Crate support checklists](docs/crate-checklists.md)

The `idealis tla` and `idealis tlc` commands from `idealis-cli` prioritize compatibility with the SANY and TLC options and exit codes they support. Higher-level wrappers such as `idealis-connect` and the TypeScript SDK have less stable APIs. The higher the level of abstraction, the more likely an alpha upgrade is to require changes to calling code, such as function arguments or driver types. [Supported CLI behavior](crates/idealis-cli/README.md#tla-ファイルを検査する)

## Intended workflow

1. Specify the states, actions, invariants, and environmental assumptions. Check the properties of the model itself.
2. Generate input sequences and the permitted observations from the model and its constraints.
3. Drive the implementation in its own language. Vary failures and execution order, then check whether the specification allows the observed history.
4. Shrink and save a failing case. Replay the same counterexample in CI after changing the specification or implementation.

As in Quint Connect, a driver connects the model to the implementation. The specification supplies the expected observations. For a nondeterministic model, the checker keeps all candidate states consistent with the observations so far. [Quint Connect](https://github.com/quint-co/quint-connect)

In the longer term, language-specific frontends could extract invariants, preconditions, and postconditions from annotated code and pass verification conditions to a common backend, following Why3. Today, `Model.invariants` checks model states. Source annotation extraction and static verification of arbitrary code remain future work. [Why3](https://why3.org/)

## Initial language support

| Language   | Test environment              | Available today                                                                               |
| ---------- | ----------------------------- | --------------------------------------------------------------------------------------------- |
| TypeScript | Vitest, `node:test`, browsers | Model/Driver API, Rust/Wasm core, property-based and differential testing, replay, Z3 adapter |
| Rust       | `cargo test`                  | Native core, standalone crates, property-based and differential testing, saved-case replay    |
| Go         | `testing`                     | CGO-free Wasm adapter for finite models, implementation observations, and histories           |

Models, values, observations, counterexamples, and checking semantics are intended to be shared. Each language's driver operates the implementation, and its frontend is responsible for reading annotations. API coverage varies by language.

## Implementation status

| Area                                            | Current support                                                                                                              |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Model conformance                               | Finite models, drivers, shared oracles, history checking, and replay. Examples in TS, Rust, and Go.                          |
| Generation, shrinking, failures, and scheduling | Property-based and differential tests, saved-case replay, explicit clocks, communication, persistence, and actor order.      |
| TLA+ model checking                             | Rust/Wasm parser, compiler, and checker, plus a tla-ts path. Both cover subsets of TLA+.                                     |
| SMT                                             | The public TS adapter uses Z3 Wasm. The standalone `rusmt` prototype solves a subset on native and Wasm targets.             |
| TLA+ and SMT interoperability                   | A common backend contract exists. Value and expression conversion and an integrated checking path are planned.               |
| Embedded invariants                             | Model-state invariants are checked. Extracting source annotations and generating verification conditions remain future work. |

Run `idealis tlc -workers 1 -config Spec.cfg Spec.tla` or `idealis tla Spec.tla` without Java. The CLI in the `@mizchi/idealis` npm package is named `idealis`. The Rust JSONL entry point is `idealis-cli`. Quint Connect's driver design and Why3's frontend/backend separation inform the design. Their languages and APIs have their own contracts. [CLI options and exit codes](crates/idealis-cli/README.md#tla-ファイルを検査する) · [Migration and saved formats](docs/migration.md)

## Libraries and their roles

Each crate tracks its supported behavior in a checklist that is synchronized before commits. TypeScript request and report types for `roracle` are generated from Rust. Its default backend is Wasm. A Node napi-rs binding is available as a comparison experiment. Other crates do not yet have equivalent generated types or native bindings. [Crate checklists](docs/crate-checklists.md) · [Native binding experiment](evals/experiments/roracle-native/README.md)

| Library                                          | Input                                                              | Responsibility                                                                                                                                        |
| ------------------------------------------------ | ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`rtla`](crates/rtla)                            | TLA+ source and TLC configuration                                  | Parses input into an AST.                                                                                                                             |
| [`rtla-compiler`](crates/rtla-compiler)          | `rtla` AST                                                         | Resolves names, expands constants and operators, and lowers supported finite models to IR.                                                            |
| [`rtlc`](crates/rtlc)                            | Model-checking IR                                                  | Enumerates states, checks invariants, deadlocks, temporal properties and fairness, and returns counterexamples. Supports native parallel exploration. |
| [`idealis-core`](crates/idealis-core)            | TLA+ source, IR, finite models, observations, and histories        | Connects frontend and checker, checks conformance and histories, and exposes replay over native JSONL and Wasm.                                       |
| [`rusmt`](crates/rusmt) (formerly `idealis-smt`) | Supported SMT-LIB expressions                                      | Solves arbitrary-precision integer difference constraints and Boolean formulas. Returns a SAT model, UNSAT, or unknown when the work budget expires.  |
| [`ralloy`](crates/ralloy)                        | Static Alloy source and finite scope                               | Parses signatures, binary relations, facts, and assertions. Enumerates finite structures to produce instances or counterexamples.                     |
| [`idealis-cli`](crates/idealis-cli)              | JSONL requests and implementation observations                     | Runs `idealis-connect` sessions and returns generated sequences and comparison results.                                                               |
| [`idealis-connect`](crates/idealis-connect)      | Finite generators, saved cases, feedback, and observations         | Manages generation, shrinking, differential testing, and replay in Rust. Asks language-specific drivers to evaluate cases.                            |
| [`roracle`](crates/roracle)                      | Fixed inputs, independent references, and implementation callbacks | Reuses registrations for differential tests, saved-oracle replay, and correctness-checked benchmarks.                                                 |
| [`@mizchi/idealis`](packages/idealis)            | TS models, drivers, backend requests and responses                 | Oracle generation, conformance, differential and property-based testing, shrinking, scheduling, and CLI.                                              |
| [Go adapter](adapters/go)                        | Core Wasm module and Go models, implementations, and histories     | Runs conformance and replay against the Rust core without CGO.                                                                                        |

In the Rust TLA+ pipeline, `rtla` parses the source, `rtla-compiler` lowers the AST to IR, and `rtlc` explores and checks it. `idealis-core` exposes the pipeline to native and Wasm callers. TypeScript uses the Rust frontend API. Go uses the core for finite-model and history checking. Both `rtla` and `rtlc` can be used independently of `idealis-core`.

`rtla` parses the pinned SANY operator table and Unicode aliases, LAMBDA/RECURSIVE, parameterized and LET-local INSTANCE, nested modules, general selectors, and structured proofs. `syntaxVersion` selects 1.7.4 or 1.8.0 precedence. The Rust compiler specializes higher-order operators. Explicit IR v2 executes recursive expressions and assignment actions under a work budget. Parsing a proof, checking a finite model, and proving a THEOREM are distinct operations. [Parser checklist](crates/rtla/SUPPORT.md) · [SANY/TLC comparisons](evals/iterations/0070-tla-coverage.md) · [Frontend contract](docs/rust-frontend.md)

The Rust frontend checks source-level constraints even in unused definitions. With explicit IR v2, selection and DOMAIN evaluate finite functions lazily through aliases and actual arguments. Nonrecursive actions that need a frame and temporal helpers can share bodies. The TS frontend also shares nonrecursive actions. Full SANY name, type, and proof checking and TLC-compatible lazy evaluation at every observation point are outside the supported subset. [Frontend semantics and reproduction steps](docs/frontend-semantics.md)

The following Criterion measurements compare sharing on the same seven inputs with equal logical work on an Apple M5. Times are in microseconds, using the median of four run means.

| Repeated body     | Compile before → after | Speedup | Prepared check before → after | Speedup |
| ----------------- | ---------------------: | ------: | ----------------------------: | ------: |
| Action, depth 6   |     3181.000 → 220.402 |  14.43× |              108.445 → 90.971 |   1.19× |
| Temporal, depth 5 |     2548.725 → 795.399 |   3.20× |             306.134 → 257.879 |   1.19× |

These synthetic models exercise repeated bodies. The measurements exclude startup, IPC, and TLC. [All 21 measurements and profile](evals/iterations/0072-frontend-semantics.md)

Six Quint models translated to TLA+ through Apalache are checked on native, Wasm, and TLC with twelve passing or violating configurations. Coverage includes `:=`, folds, finite choice, Variants, and CASE. [Quint-generated TLA+](docs/quint-generated.md)

`rusmt` is independent of this exploration pipeline. The public SMT adapter currently uses Z3 Wasm. Integrating `rusmt` into that backend remains future work.

## Coverage and integration

TLA+, SMT-LIB, and Alloy support is limited to documented subsets. Model-checking results apply to the supplied model, assumptions, and search bounds. Conformance results apply to the action sequences and observations exercised. [Verification scope](#verification-scope)

The Rust/Wasm frontend handles quantified fairness over finite domains, SelectSeq, SortSeq, and Assert. The compiler checklist records callback and search restrictions. TLA+ can also run through tla-ts. The optional SMT adapter uses `z3-solver`. The TS `Driver` connects specifications to implementations in the style of Quint Connect. [Compiler checklist](crates/rtla-compiler/SUPPORT.md)

The standalone `rusmt` crate solves integer difference constraints and Boolean formulas on native and Wasm targets without Z3 or another idealis crate. It supports constant multiplication, integer `ite`, and assertions, push/pop, and temporary Boolean assumptions through Rust/JSON. SMT-LIB scripts can use push/pop and multiple checks when declarations are fixed at the start. Before the crate was split out, native runs on small TTL, amount, and scheduler cases were about 3.0–3.4× faster than reused Z3. Some Boolean UNSAT cases regressed or exhausted the budget. [SMT checklist](docs/smt-checklist.md) · [SMT-LIB subset](docs/smt-subset.md) · [All 17 inputs](evals/iterations/0031-smt-reuse.md)

`ralloy` checks static signatures, binary fields, relational operations, quantifiers, and run/check within a finite scope. Native and import-free Wasm use the same search engine without a JVM or Z3. Alloy Analyzer's SAT translation, inheritance, temporal features, `var`, and integer arithmetic remain unsupported. `@mizchi/idealis/alloy` loads the Wasm module to explore a model or compare relational snapshots. Integration with the general backend contract and CLI is planned. [Alloy guide](docs/alloy.md) · [Evaluation](evals/experiments/alloy-subset/README.md)

`idealis-connect` provides finite-state-machine generation, shrinking, differential checking, and saved-case replay. TS uses the same Wasm through `@mizchi/idealis/testing` with Vitest, `node:test`, or browser drivers. It can generate schedules for explicitly supplied actors. Its guarantees do not extend to fast-check strategy/seed compatibility, automatic instrumentation of arbitrary threads, or proofs over all executions. [Actor scheduling](docs/testing.md#actorの操作順を制御する)

With `roracle`, a Rust `#[test]` registers cases and implementations once for differential checking and comparative benchmarks. It replays saved oracles and detects changes in results during measurement. Pair it with `idealis-connect` for generation and shrinking.

Development fuzzing checks the testing protocol and saved cases under ASan and replays the corpus on native and Wasm. The evaluation records reachability, state-corruption controls, budgets, and adoption criteria. [Structured fuzzing](evals/experiments/structured-fuzz/README.md) · [Evaluation 0039](evals/iterations/0039-structured-fuzz.md)

The SAT/CNF experiment compared Varisat 0.2.2 on native and Wasm against independent exhaustive enumeration and checked it in a real browser. It remains a private evaluation crate while budget guarantees are unresolved. [Evaluation 0037](evals/iterations/0037-sat-foundation.md)

## Running the toolkit

Use Node.js 24+, pnpm 12+, and just. To develop the Rust/Wasm backends, install Rust and the `wasm32-unknown-unknown` target. Building the npm TLA+ CLI also uses `wasm32-wasip1`. Consumers of the packaged Wasm can use it directly.

```sh
just install
just check
just demo
just mutation
rustup target add wasm32-unknown-unknown wasm32-wasip1
just wasm-test
```

Run tasks from the pnpm workspace root. `just build-sdk` builds the TypeScript SDK and CLI. `just package-prepack` builds the CLI with its Wasm assets. `just package` creates npm, Cargo, and Wasm archives that work without a required solver and tests them from a separate consumer. The guide covers sharing finite models and observations between TS and Rust, shrinking failures, and injecting clocks, communication, and persistence. [Package](packages/idealis) · [Model guide](docs/using-models.md) · [Releases and CI](docs/releasing.md)

`just check` runs Vite+ formatting and lint checks, TypeScript type checking, Vitest, and `node:test`. `vite.config.ts` specifies test targets and style rules. Use `just lint`, `just fmt`, and `just vitest` for individual tasks. `just wasm-test` exercises the real Wasm ABI, verdicts, replay, recovery, and agreement with native results in Node. Both runners register shared behavior tests. `just mutation` mutates TS sources with `@mizchi/mutator` and runs the cases in Vitest.

`just eval-loop` runs the evaluation loop and `just replay-loop` reproduces saved failures. The initial broader comparison uses `just evaluate` and `just replay-evaluation`. Hypotheses, conditions, runs, decisions, and follow-up questions live under evals. Its experimental explorers and history checkers are private. [Evaluation guide](evals/README.md)

`./budget` chooses worker counts and search depths given time and memory limits. `./sharding` partitions and aggregates independent tasks. Rust/Wasm kernels for these experiments are separate from the public API. [Runtime and cloud sharding](evals/experiments/runtime/README.md) · [Measurements](evals/iterations/0003-runtime.md)

`./scheduling` places independent checks and dispatches them to persistent workers. Its API is independent of Node and solvers. The Node and browser TLA adapters here are evaluation prototypes. [Scheduling guide](docs/scheduling.md) · [TLA checks with 1/2/4/8 workers](evals/iterations/0017-parallel-safety.md)

## Sharing operator bodies in IR v2

Pass `irVersion: 2` to `compileTlaSource(core, { source, config, irVersion: 2 })` or the TS frontend's `compileTlaSafety({ source, config, irVersion: 2 })` to share operator definitions. The default is v1. Arguments retain the caller's lexical environment and evaluate on demand with the current/next/prime context. Calls and argument requests consume work. Unused arguments stay unevaluated.

Rust v2 supports module linking, configuration substitutions, and shared recursive assignment actions in Init/Next. SYMMETRY is unsupported in v2. The Rust and TS frontends accept different fragments. On the public TCommit model with RM=8, Rust v1/v2 and TLC agreed on 6,816 distinct states and 53,513 generated states. Tests also cover six Quint translations with twelve normal/fault configurations. These are finite-scope comparisons. [IR contract and restrictions](docs/safety-ir-v2.md)

| Fixed sharing chain              |                        v1 | Explicit v2 |
| -------------------------------- | ------------------------: | ----------: |
| IR bytes at depth 10             |                   330,878 |       4,707 |
| Compile request at depth 10 (ms) |                    35.534 |       0.359 |
| IR bytes at depth 60             | Rejected at nesting limit |      24,557 |

Compile time is the median request/IR response through a persistent native JSONL process. Sharing avoids inlining unselected branches. Evaluation itself can still require exponential work. Cold TCommit time rose from 114.85 ms before integration to 124.05 ms with v1 and 129.32 ms with v2. [Costs, profile, and decision](evals/iterations/0062-p0-checker.md)

## Repeated checks on one SafetyProgram

`prepareSafetyProgram(core, program)` reuses validated IR and constant indexes. Pass different search budgets or options to `check` and `checkBatch`. Each job has independent search state and work accounting. A batch returns 1–256 results or errors in input order. Call `release()` when finished. Rust exposes the same contract through `program.prepare()`.

For 100 Counter queries, native v1 API time including preparation and release fell from 3.883 ms one-shot to 0.643 ms batched, or 2.162 to 0.256 ms when varying the budget. Gains were smaller on the search-heavy TCommit model, and the batch figures do not apply to a single query. Earlier evaluations used “prepared search” to mean a one-shot check of precompiled TLA+ IR. [API and budgets](docs/prepared-safety.md) · [Cold timings, RSS, and regressions](evals/iterations/0064-prepared-safety.md)

## SYMMETRY for node-indexed functions

The Rust frontend accepts `Symm == Permutations(Nodes)` for one set of untyped model values. For Int/Bool functions with domain Nodes, it groups values by node, retaining correlations across functions and concrete counterexamples. A three-node, three-value counter shrank from 27 states to 10. With seven nodes and six values it shrank from 279,936 to 792. Median sequential native time including startup fell from 673.5 to 36.5 ms (18.5×), and prepared search from 633.2 to 3.89 ms (163×).

The user must establish that the declared symmetry preserves Init, Next, and the invariants. This reduction supports safety checks and rejects combinations such as node-reference payloads and temporal/fairness checks. Optimization decisions use native Rust measurements, while Wasm tests check portability and agreement. [Configuration and conditions](docs/symmetry.md) · [Evaluation 0060](evals/iterations/0060-restricted-symmetry.md)

Iteration 0061 added flat Int/Bool records per node and subsets of nodes. `symmetryActions` expands one representative for equivalent node rows. Native search can expand a frontier of at least 64 states in parallel. For record N5, prepared search took 20.69 ms without reduction, 2.12 ms with symmetry, and 1.60 ms with representative expansion. For record N12, representative expansion took 92.6 ms with one worker and 80.9 ms with eight. The report includes reruns and substantial variation between repetitions.

An opt-in partial-order reduction (POR) handles independent operations that each set a Boolean flag once. On a synthetic ten-flag model it reduced 4,096 states to 14 and prepared search from 3.74 to 0.0105 ms. The reduced graph does not guarantee the original graph's shortest counterexample. Direct execution by instruction ID regressed the counter by about 40% and was dropped. [Options and restrictions](docs/reductions.md) · [Evaluation 0061](evals/iterations/0061-symmetry-next.md)

Including startup and the compiled-IR JSON boundary, record N12 took 99.3 ms with one worker and 84.6 ms with eight. Median peak RSS rose from 12.8 to 15.2 MiB. This includes the Node measurement driver and timing wrapper but excludes Node host startup and the TLA+ frontend. Wall time for the small POR model fell from 6.89 to 3.70 ms.

Iteration 0065 stored node rows contiguously and avoided copying the current state's Option array. In 21 alternating Criterion comparisons of fixed binaries, SYMMETRY checks took about 8–17% less time. Record N12 representative expansion fell from 85.57 to 71.80 ms with one worker and from 73.01 to 64.41 ms with eight. Reduction conditions, generated counts, and logical work stayed the same. [Conditions, controls, and decision](evals/iterations/0065-symmetry-allocation.md)

Iteration 0067 skipped schema reconstruction only for the same internal value as the first validated record. Across 21 paired comparisons, six symmetry record cases took about 6.5–9% less time. N12 IR checks fell from 73.40 to 67.95 ms with one worker and from 63.89 to 59.49 ms with eight. Persistent API timings including source compilation and IR checking were 77.52 to 75.51 ms and 67.22 to 63.64 ms respectively. Distinct records still undergo validation, logical work is preserved, and caches stay within one arena. [Samples and decision](evals/iterations/0067-record-schema.md)

Further tuning of shape caches and borrowed entries awaits a profile showing they dominate cost. Small-integer compression needs a real model that exceeds memory limits with existing 0/1 packing. Worker tuning needs evidence of waiting or merge costs on broad frontiers. [Remaining candidates and criteria](https://github.com/mizchi/idealis/issues/63)

## Preparing Data constants

Iteration 0059 saved the first successful evaluation of a Data constant in a slot to avoid hashing the entire value on each use. All runs below exhaustively explored the same channel model of about 2.1 million states. Figures include startup and are median seconds from three runs after one warmup.

| Execution           | Before |  After | Before/after |
| ------------------- | -----: | -----: | -----------: |
| Native, 1 worker    | 14.075 | 13.463 |         1.05 |
| Native, 8 workers   |  5.887 |  5.432 |         1.08 |
| Node Wasm, 1 worker | 21.191 | 19.308 |         1.10 |

The native eight-worker ranges overlapped, with the order reversing on the final repetition. The native function model was nearly flat. Channel Wasm remained about 18% slower than tla-ts (16.374 s) in the same run. Typed instructions and prechecking visited states during parallel exploration both showed regressions and were dropped. [Nine paths, fourteen component comparisons, and decisions](evals/iterations/0059-prepared-execution.md)

## Hashing packed functions

Iteration 0058 exhaustively explored a 0/1 integer function with N=20 on Apple M5 arm64, Node 24.22.2, and tla-ts 0.0.10. Times include startup and the common TS frontend and are median seconds from three runs after one warmup. Every path found 1,048,576 distinct states and generated 20,971,521.

| Execution           | Before | After | Before/after |
| ------------------- | -----: | ----: | -----------: |
| Native, 1 worker    |  3.377 | 3.191 |         1.06 |
| Native, 8 workers   |  2.399 | 2.280 |         1.05 |
| Node Wasm, 1 worker |  5.017 | 4.548 |         1.10 |

On this function model, native with eight workers was 1.84× faster than TLC with eight, and Node Wasm was 1.12× faster than tla-ts. Reference timings varied substantially on channels. Sequential ring time in Criterion grew by about 8%. [Samples, controls, and compatibility checks](evals/iterations/0058-packed-hash.md)

## Function updates in iteration 0057

The following complete explorations ran under the same conditions in 0057. Times include startup and the common TS frontend, in median seconds from three runs after one warmup. All paths agreed on distinct and generated counts.

| Model                | tla-ts 1 |  TLC 1 | TLC 8 | idealis Wasm 1 | idealis native 1 | idealis native 8 |
| -------------------- | -------: | -----: | ----: | -------------: | ---------------: | ---------------: |
| 0/1 integer function |    5.648 | 13.487 | 4.391 |          5.595 |            3.910 |            2.609 |
| Channels             |   15.829 | 23.260 | 6.341 |         20.541 |           13.670 |            5.531 |

Relative to the pre-change binary, parallel native channel exploration sped up by 1.11×. Sequential and Wasm changes were small, and the function model regressed by about 1–2%. Channel Wasm was about 30% slower than tla-ts. Runs used an Apple M5 arm64, Node 24.22.2, tla-ts 0.0.10, and pinned arm64 Java/TLC on a shared host. Wasm ran in Node. The 0056 domain sharing and bit-packing measurements and the 0022 four-model table came from separate runs. [0057 samples and checks](evals/iterations/0057-probe-updates.md) · [Evaluation 0056](evals/iterations/0056-compact-functions.md)

`./check-cache` reuses complete exploration results for identical inputs using a browser-compatible TS core with SQLite/HTTP adapters. A Lean model specifies reuse conditions, ownership, and generations. Finite differential tests and real-browser tests exercise the implementation. Rust/Wasm optionally reuses per-state Next evaluations and rechecks the current Init, properties, and constraints. Enable this state cache with `enabled: true`. Lean is a development dependency. Distributed BFS and queue checkpoint recovery remain future work. [Check cache](docs/check-cache.md) · [State expansion reuse](docs/safety-expansions.md)

## Model-checker performance

Iteration 0062 ran each complete exploration in a fresh process on identical TLA+ source and configuration. Figures are median seconds from three runs after one warmup on Apple M5 arm64, 32 GiB, Node 24.22.2, tla-ts 0.0.10, and tla2tools v1.7.4 / Temurin 21.0.13+9. Native and Wasm times include Node startup, the common TS frontend, and IR v1 transfer.

| Model            | Distinct states | tla-ts 1 worker | TLC 1 worker | TLC 8 workers | idealis Wasm 1 worker | idealis native 1 worker | idealis native 8 workers |
| ---------------- | --------------: | --------------: | -----------: | ------------: | --------------------: | ----------------------: | -----------------------: |
| Counters         |       5,308,416 |           6.490 |        6.913 |         7.922 |                 4.195 |                   3.218 |                    2.968 |
| Boolean function |       1,048,576 |           5.424 |       12.781 |         4.325 |                 4.613 |                   3.485 |                    2.440 |
| Channels         |       2,097,152 |          15.728 |       23.247 |         6.285 |                19.977 |                  13.809 |                    5.497 |
| Width-one ring   |       1,000,000 |           0.407 |        0.926 |         1.474 |                 0.255 |                   0.182 |                    0.181 |

Wasm ran in Node. Worker counts are requested maxima. The width-one native model never starts its pool. Only single-worker tla-ts was measured. Both TLC and Node used 8 GiB heap settings, which do not impose a common RSS limit. The host was shared, and these results apply to the finite fragments measured.

Requesting eight native workers yielded speedups over eight-worker TLC of 2.67× on counters, 1.77× on the function, and 1.14× on channels. Compared with the version before shared IR integration, native function runs were about 8% slower with one worker and 6% slower with eight. Eight-worker native channels improved, though the baseline varied substantially. Channel Wasm was about 27% slower than tla-ts. Persistent APIs, IR v2, and the older 0022 runs are reported separately. [Samples, ranges, and profiles](evals/iterations/0062-p0-checker.md) · [0022 comparison](evals/iterations/0022-readme-comparison.md)

Iteration 0063 tuned Data constant slot lookup. TCommit (RM=8) native IR requests fell from 143.77 to 113.24 ms in v1 and from 140.55 to 118.32 ms in v2. Including startup, times fell from 138.70 to 118.61 ms and 143.87 to 125.93 ms respectively. These are medians of three samples after one warmup on a shared M5, including IR decoding and preparation. Some of the fourteen kernels regressed by a few percent. [Hash change, rejected inlining, and samples](evals/iterations/0063-evaluator-tuning.md)

Iteration 0066 used a candidate index for Ref-domain lookup after a full key match and fell back to linear search on a miss. Twelve TCommit IR requests per fixed binary showed a decrease from 105.01 to 97.61 ms in native v1 and 109.61 to 102.55 ms in v2. The initial three samples and cold results showed about 3–4% reductions. The regular fourteen benchmarks included regressions of up to about 6%. [Conditions and controls](evals/iterations/0066-domain-offset.md)

## Differential checks and benchmarks from the same registrations

`roracle` ran the same small models on TLC, tla-ts, and idealis. Median cold source-to-result timings use seven samples after one warmup, all with one worker. They include process startup, transport, parsing, and raw log writes.

| Model         | idealis native (ms) | tla-ts (ms) | TLC (ms) |
| ------------- | ------------------: | ----------: | -------: |
| linear-ring   |               4.505 |      71.607 |  584.650 |
| function-cube |               2.675 |      78.875 |  593.217 |

All three agreed on completion and distinct/generated counts for the 1,000-state ring and the 8-bit, 256-state function cube. Startup weighs heavily for such small models. The evaluation also covers Rust prepared check/coverage comparisons, drift detection during measurement, saved-oracle replay, and the first failure. [Evaluation 0051](evals/iterations/0051-oracle-library.md) · [Rust tests and Criterion-style groups](crates/roracle/README.md)

The workspace migrated 99 comparisons in 23 files to one Rust/Wasm comparator. The tuning procedure covers before/after binary and source snapshots, cold and reused processes, and saved-oracle replay. All 1,222 comparisons across twelve control groups agreed. The report also measures JSON bridge overhead. Fixing the TLC reference environment resolved 24 earlier discrepancies, with zero discrepancies in 155 checks, 32 confirmations, and 155 saved replays. [Tuning procedure](evals/experiments/workspace-oracle/README.md) · [Evaluation 0052](evals/iterations/0052-workspace-oracle.md) · [Evaluation 0053](evals/iterations/0053-tlc-standard-modules.md)

## Rust core before and after optimization

Following the six-path baseline in 0022, six optimizations were measured by alternating the original and modified native/Wasm binaries. Figures are median seconds including startup from three runs after one warmup. Ratios are before/after.

| Model            | Native 1, before → after (s) | Ratio | Native up to 8, before → after (s) | Ratio | Wasm 1, before → after (s) | Ratio |
| ---------------- | ---------------------------: | ----: | ---------------------------------: | ----: | -------------------------: | ----: |
| Counters         |                3.114 → 3.061 | 1.02× |                      3.781 → 2.996 | 1.26× |              3.944 → 4.822 | 0.82× |
| Boolean function |                9.609 → 7.221 | 1.33× |                      5.407 → 3.942 | 1.37× |            19.809 → 10.585 | 1.87× |
| Channels         |              22.260 → 17.838 | 1.25× |                     13.500 → 8.829 | 1.53× |            38.771 → 24.964 | 1.55× |
| Width-one ring   |                0.213 → 0.215 | 0.99× |                      0.223 → 0.217 | 1.03× |              0.268 → 0.286 | 0.94× |

The counter with up to eight workers ran sequentially because its scalar operations were cheap. Wasm counters and ring regressed. Native RSS sampled every 200 ms fell by about 40% for the Boolean function and 35% for channels. Run `just criterion-test` for a smoke test or `just criterion-save <stage> <new-run-path>` for all fourteen benchmarks. [Criterion results and reproduction](evals/iterations/0023-criterion-safety.md)

## CPU profile-guided tuning

After 0023, scalar leaf evaluation and integer-key function lookup were adjusted. The table shows a fresh alternating comparison on ordinary Wasm with the profiler removed, including startup.

| Model          |  Before → after | Before/after |
| -------------- | --------------: | -----------: |
| Counters       | 4.476 → 4.136 s |        1.08× |
| Width-one ring |    272 → 255 ms |        1.07× |

Native Criterion counter time fell from 12.285 to 9.266 ms, and function time from 11.071 to 9.416 ms. The integer-index change alone sometimes regressed channels with up to eight workers. Reproduce with `just profile-test` and `just profile-criterion <stage> <new-run-path>`. [Profiles, component timings, samples, and replay](evals/iterations/0024-profile-tuning.md)

## SMT script parsing

Criterion measured native `rusmt` parsing on Apple M5. Medians are from twenty samples and exclude solving, JSONL, and Wasm. [All fourteen cases, variability, and rerun conditions](evals/iterations/0042-smt-tuning.md)

| Script                                  |   Before |    After | Speedup |
| --------------------------------------- | -------: | -------: | ------: |
| 64 checks, 32-variable integer chain    | 4.701 ms | 1.183 ms |    4.0× |
| 32 pairs of scope operations, 64 checks | 415.2 µs | 239.0 µs |    1.7× |
| Integer ite, 64 checks                  | 212.7 µs |  31.8 µs |    6.7× |

## Comparing a model with an implementation

The TS/Rust shared core in `./portable` uses the same Rust source for native and Wasm conformance checks and trace replay. The Go adapter uses the same Wasm without CGO. `just portable-build` and `just portable-test` exercise Rust and host integration. `just go-test` checks Go comparisons and a standalone consumer. [Values, effect protocol, and examples](docs/portable-core.md) · [Go adapter](docs/go-adapter.md)

`just cross-test` drives normal and faulty TS, Rust, and Go implementations directly using the same finite model and saved trace. [Shared oracle](docs/shared-oracle.md)

`./history` and `loadFiniteModel` support asynchronous histories and model reuse. `just async-test` covers incomplete operations and unknown results, indexed handles, dependency-aware shrinking, and exploration feedback. [Asynchronous histories](docs/async-history.md)

`compileTlaSafety` and `checkSafety` use IR for finite models with integers, sets, functions, records, sequences, temporal properties, and weak/strong fairness. State generation, BFS, SCC checks, and counterexample reconstruction run inside native Rust or Wasm. For models outside the Rust subset, the existing tla-ts path is available within its own supported scope. [Rust safety backend](docs/rust-safety.md)

`rtlc` owns IR evaluation, search, temporal/fairness checks, and native parallelism. `idealis-core` owns implementation conformance, histories, hosting, shrinking, and the Wasm/JSONL adapters. `just rtlc-test` and `just rtlc-example` exercise standalone use. `just package` checks packaging and external consumers. [Checker crate](crates/rtlc)

`rtla` parses source and `rtla-compiler` lowers the AST to IR. `compileTlaSource` and `checkTlaSource` run the Rust frontend in a browser without tla-ts. `compileTlaSafety` retains the tla-ts frontend for comparisons. Use `just compiler-test` and `just compiler-eval <new-run>` to check the compiler. `just package` also checks the nine crates and packaged Wasm assets. [Parser](crates/rtla) · [Compiler](crates/rtla-compiler) · [Rust frontend](docs/rust-frontend.md)

The Apalache pilot records startup-inclusive time and counterexamples for six matching conditions. SMT-based bounded checking and `rtlc` state enumeration offer different guarantees. The timings apply to these small models. [Apalache comparison](docs/apalache-comparison.md)

`@mizchi/idealis/backend` distinguishes exhaustive exploration, bounded absence of a counterexample, and inductiveness. Results identify the model, property, semantics, assumptions, and backend version. An empty initial set, resource exhaustion, or an unknown result stays visible. A native/Wasm bridge to `rtlc` is available. A general Apalache adapter and SMT encoding remain future work. [Backend contract](docs/backend-contract.md) · [Saved-response checks](evals/iterations/0028-backend-contract.md)

```ts
import { checkConformance, checkModel, checkTrace } from "@mizchi/idealis";
import { counterModel, createCounter } from "./examples/durable-counter.js";

const result = await checkConformance(counterModel, createCounter(), [
  { type: "add", id: "request-1", amount: 2 },
  { type: "crash" },
  { type: "restart" },
  { type: "add", id: "request-1", amount: 2 },
]);
if (result.status !== "matched") throw new Error(JSON.stringify(result));

const oracle = checkModel(counterModel, { maxDepth: 4, maxStates: 500 });
for (const trace of oracle.traces) {
  const replay = await checkTrace(trace, createCounter());
  if (replay.status !== "matched") throw new Error(JSON.stringify(replay));
}
```

The example pairs an independent immutable reference model with a mutable implementation. It represents failures with `crash` and `restart` and network events with `send`, `deliver`, and `drop`. Choosing a delivery index reproduces delay or reordering. These events simulate failures at the driver boundary.

`Model.initial()` and `Model.step()` return allowed states and observations. `Driver.reset()` and `Driver.step()` operate the implementation and return its observations. For a nondeterministic model, the checker narrows candidate states against the entire observed history. Driver exceptions propagate to the caller, and `dispose()` runs at the end.

## Property-based and differential testing

```ts
import assert from "node:assert/strict";
import { array, constantFrom } from "fast-check";
import { checkProperty } from "@mizchi/idealis/property";
import { checkDifferential } from "@mizchi/idealis";
import {
  counterModel,
  createCounter,
  type CounterAction,
} from "./examples/durable-counter.js";

const actions = array(
  constantFrom<CounterAction>(
    { type: "add", id: "a", amount: 1 },
    { type: "crash" },
    { type: "restart" },
  ),
  { maxLength: 24 },
);

const result = await checkProperty(counterModel, createCounter, actions, {
  seed: 42,
  numRuns: 100,
});
assert.equal(result.status, "passed");

const diff = await checkDifferential(createCounter(), createCounter(), [
  { type: "add", id: "a", amount: 1 },
]);
assert.equal(diff.status, "matched");
```

This code works in a Vitest `test()` or `node:test`. With the faulty `persist: false` implementation, the failure shrinks to `add → crash → restart`. To reproduce, use the reported `seed` and `path` with the same arbitrary and fast-check version. Save the action sequence itself for long-term replay. When every case is discarded because of invalid actions, the result is `exhausted`.

The demo writes `.artifacts/oracle.json`, `tla-oracle.json`, and `counterexample.json`. `decodeTrace` requires type guards for actions and observations. Traces use versioned JSON. Adapters explicitly encode bigint, Set, and Map.

## Adapters and portability

| Source                             | Responsibility                                                                   | Runtime dependency         |
| ---------------------------------- | -------------------------------------------------------------------------------- | -------------------------- |
| `packages/idealis/src/index.ts`    | Finite-model BFS, composition, conformance, differential comparisons, and traces | Standalone TS              |
| `packages/idealis/src/property.ts` | Generation, shrinking, and seed/path replay                                      | fast-check                 |
| `packages/idealis/src/tla.ts`      | Reads TLA+ checker results and converts finite counterexamples                   | tla-ts                     |
| `packages/idealis/src/smt.ts`      | Reads SMT-LIB SAT/UNSAT/UNKNOWN results and integer solutions                    | Caller-initialized Z3 Wasm |

Importing the core leaves solvers unloaded. Regular CI runs 23 shared contracts through the Node test runner and Vitest using Node WebAssembly, and checks the browser bundle. `just browser` and the manual workflow exercise Web Worker transport and cross-origin isolation in a browser. Browser Z3 requires serving `z3-built.js` and its Wasm asset, SharedArrayBuffer, and COOP/COEP headers. [Browser compatibility workflow](.github/workflows/browser.yml)

`tla-ts@0.0.10` contains Node/Bun references. The browser prototype configures Vite+ with `resolve.alias: { "node:zlib": "fflate" }`, `build.rolldownOptions.external: ["bun:jsc"]`, and `define: { global: "globalThis" }`. The Bun-only dynamic import is unreachable in the browser. TLA+ uses the default single worker. [Browser build and serving example](tests/browser.spec.ts)

Pass the result of Z3's `init()` to `createSmtOracle(api)`. In Node, call `killThreads(api.em)` afterwards. Integer solutions are returned as decimal strings to preserve arbitrary precision. The API takes SMT-LIB assertions. Symbolic execution of arbitrary TS functions is outside its scope.

## Verification scope

- `complete` means that the reachable graph closed under the supplied `actions` and a collision-free `key`. A cutoff gives `bounded`, and no initial states gives `empty`. The TS core checks state invariants. Liveness and fairness require a backend that supports them.
- Generated traces are representative witnesses for reachable states and transitions. Two histories ending in the same model state can leave different hidden states in the implementation.
- Conformance checks cover the supplied actions, observations, and abstraction. `invalid` means the model could not take an action. `mismatch` means the observation fell outside the allowed set. Neither finite trace coverage nor a match establishes a general refinement or linearizability proof.
- `composeModels` explores interleavings of independent actions and the product of explicit `both` actions. Users can specify invariants across model boundaries. Assume-guarantee reasoning is outside its scope.
- The TLA+ adapter creates no witness on success. Lasso counterexamples retain the original checker result because they cannot be converted to finite oracles. Matching types and integer semantics to Z3 requires explicit conversion.

[Architecture](docs/architecture.md) · [Testing methods](docs/testing-research.md) · [Validation records](docs/validation.md)

Recursive action calls now share the parent frame previously copied per argument. In Criterion, the prepared check with several arguments fell from 1,036.8 to 808.6 µs (1.28×). The two captured-variable cases took about 17–19% less time. Single-argument and branching cases were nearly unchanged. The baseline was the original implementation. [Recursive actions](docs/recursive-actions.md) · [Conditions and results](evals/iterations/0071-recursive-actions.md)
