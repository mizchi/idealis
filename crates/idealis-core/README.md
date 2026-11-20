# idealis-core

SafetyProgramのprepare/check/batch/releaseをnative/Wasmの共通protocolで提供する。batchは1〜256件の独立予算・結果・errorを順序を保って扱い、handleはEngine内でのみ有効。[APIと所有の契約](../../docs/prepared-safety.md)を参照。

`check-safety-coverage`は逐次のSafetyProgram検査へcoverageを付ける。応答は`{kind:"safety-coverage",report:{result,coverage}}`。通常の`check-safety`の結果形は変えない。branchのattempts/生成・新規state/workとunattributedWorkは[rtlcの契約](../rtlc/README.md#アクションのcoverage)に従う。

[対応範囲・非対応のチェックリスト](SUPPORT.md)を参照。

Rust native / Wasm core for finite state models, conformance checking, trace replay, and failure-preserving sequence shrinking.

The `compile-tla` and `check-tla` requests accept an optional `modules` object mapping module names to source strings. The compiler resolves the explicit bundle without filesystem or network access. Omitting `modules` preserves the existing single source request. See [module resolution and configuration overrides](../rtla-compiler/README.md#モジュールと設定置換) for the supported fragment.

`SafetyProgram::check()` is re-exported from the independent `rtlc` crate. It evaluates a versioned transition program and performs BFS, invariant/deadlock checking, temporal/fairness checking, and counterexample reconstruction inside Rust. It generates successors during exploration rather than accepting a precomputed graph. The IR supports a finite TLA+ fragment including integers, booleans, sets, functions, records and sequences; it is not a complete TLA+ interpreter. `compile_tla(source, config)` connects the `rtla` parser and `rtla-compiler` to produce that IR in native or Wasm. The checker itself lives in `rtlc` and has no parser or solver dependency. The existing TypeScript `compileTlaSafety` frontend remains a comparison path. See [the supported fragment and limits](https://github.com/mizchi/idealis/blob/main/docs/rust-safety.md).

`Session::new()` and `Session::replay()` return effects. The caller performs model evaluation or I/O and returns responses with the pending effect ID. `FiniteModel` loads a versioned, observation-bearing transition table without a model-language runtime. `FiniteModel::check_history()` validates saved observations against that table. No threads, clocks or host imports are required by the Wasm library.

The checker implementation and its Criterion benchmarks live in [rtlc](../rtlc). `idealis-core` retains the finite-model, conformance, history, host, shrink, JSONL and Wasm adapters. `Value` and `CoreError` are shared by re-export, with the existing wire format preserved. `Value` implements `Drop` to release arbitrarily deep values with bounded stack, so its fields cannot be moved out by pattern; match by reference and take fields with `std::mem::take` or `std::mem::replace`.

See the [repository](https://github.com/mizchi/idealis) for TypeScript bindings, the shared value format, examples, and evaluations. The `idealis-core` binary accepts the same versioned JSON commands over stdin/stdout, one JSON object per line.

The release is experimental. A matched finite history covers the supplied actions and observations; it is not a refinement proof or a liveness guarantee.

`compile_tla_with_options(source, config, CompileOptions { irVersion: IrVersion::V1 })` selects v1. Omitted `irVersion` retains v1. Explicit v2 shares expression definitions and supports marked recursion, module linking and configuration overrides. Calls and argument demands consume work; values are not memoized. Recursive evaluation exhaustion returns bounded/evaluation. Marked recursive assignment actions use shared action definitions in explicit v2; nonrecursive action operators that need a caller frame can also be shared. Native/Wasm requests also accept `syntaxVersion: "1.7.4" | "1.8.0"` for the Rust parser's precedence table. The request envelope and Wasm ABI remain at version 1.

`check-safety-incremental` optionally reuses complete per-state Next expansions from a portable snapshot. It rechecks current initial states, properties and constraints. `merge-safety-expansions` combines snapshots with matching namespaces and rejects conflicting entries. The implementation lives in `rtlc`; JSONL and Wasm expose the same sequential checker. See [the cache contract and proof scope](../../docs/safety-expansions.md).

Expansion caching defaults to disabled. Set `enabled: true` to use it. An omitted or false flag delegates to the ordinary checker, leaves the cache unchanged, and returns zero cache counters.

明示v2のaction/temporal helperは共有callまたはanonymous frameでcallerの遅延引数を保持する。binder・EXCEPT oldと要求時のprimeを分離し、入口と引数要求へ各1 workを課金する。sourceの対応範囲はcompilerが判断する。再帰expressionとaction定義は明示v2で扱う。Rust frontendが生成する再帰actionの範囲は[呼出しと制限](../../docs/recursive-actions.md)を参照。

明示IR v2のsource level検査・遅延関数観測・action/temporal共有はrtla-compilerとrtlcへ委譲する。[範囲と検査方法](../../docs/frontend-semantics.md)を参照。
