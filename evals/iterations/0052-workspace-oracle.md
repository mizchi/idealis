# 0052: workspaceの差分検査とチューニング

idealis-oracleをworkspaceの23ファイル・99比較箇所へ接続した。TSのNode adapterからWasmのRust OracleGroupを実行し、parser/compiler、core、SMT、Alloy、PBT、TS/Rust/GoのSUTと非同期履歴の差分を共通化した。[移行一覧](../workspace-oracle/migrations.json)と[実行手順](../workspace-oracle/README.md)を参照。

## 維持した比較と反例

snapshotはundefined・bigint・NaN・負のzeroを区別する。plain objectのkey順は無視する。対象外のobjectや循環は黙って変換せず拒否する。18種類の値の全組み合わせで、従来のNode isDeepStrictEqualと判定が一致した。不一致はNodeのAssertionErrorへreportを添え、既存ランナーへ伝える。Rust側は収集済みJSON batchのdispatch、native JSONLとWasm ABI v1を提供する。ホストから実装関数を呼ぶbenchmarkのJSON bindingは追加していない。

正規化や期待status、Z3のSATモデル検証、TLCの分類と強制trace検証は維持した。正常なモデル検査の完了と、unknownや意図した拒否応答のprotocol一致を区別する。対応していない入力を、比較器の導入によって成功へ変換しない。既存の保存runは書き換えていない。

[compilerの239ケース](../runs/0052-compiler-02/result.json)は237受理・2拒否だった。移行したparser、portable/native/Wasm、SMT/Z3、Alloy、testingのNode回帰48本が通った。crates/oracleのtyped 14本とJSON 2本、process adapterの4本、Chromiumのtyped経路と実JSON ABIの2本も通った。

[3言語の回帰](../runs/0052-cross-02/result.json)は136 cases、816正常対照、3264比較で言語間差分0。TS/Rust/Goの観測digestも同じだった。保存した5故障を3言語で再現した。[非同期の回帰](../runs/0052-async-02/result.json)と再生は182保存故障、42 concurrent historiesの126比較を確認した。既存seedの回帰再使用であり、新しい未知バグ検出や確認用seedによる手法の優位性は主張しない。

## TLCで残った24件を成功にしない

[155ケースのTLC実行](../runs/0052-tlc-development/result.json)は129一致・24不一致だった。24件はApalacheをimportする仕様で、参照TLCがApalache.tlaを発見できずinput-errorになった。この実行を完全一致や全TLA+互換の証拠にしない。foldの参照環境を整える作業は残る。

c268b8eeの旧比較器を同じ観測とraw logへ適用し、[155件の差分判定が同一](../runs/0052-legacy-parity/result.json)だったことを確認した。native/Wasmの等値判定も従来のNode比較と一致した。差分をフィルタして0件にする修正はしていない。比較器の移行による追加の差分は0件で、24不一致は保存したまま。

## 変更前後のチューニング手順

snapshotは実行ファイル、source、request、受理条件、hash、revisionと環境を保存する。対象はrtla、idealis-core、idealis-smt、ralloy、idealis-testing、idealis-oracleの6境界。coreはrtla-compilerとrtlcを含む。ケースに未知・予算切れが返れば受理条件が拒否する。入力と受理条件の異なるsnapshotも比較しない。

[同一版の正常対照](../runs/0052-workspace-bench-02/result.json)は13 casesをcold/reusedへ分けた12 groupsで、計1222回を照合した。各条件7 samples・1 warmup・3 iterations。各呼出の応答全体をbaseline専用のreferenceへ比較し、速度baselineもbaselineを指定した。coldは毎回processを起動し、reusedは3つの独立したJSONL processを使う。起動・IPC・JSONを含む値で、checker kernelの時間とは解釈しない。

[保存オラクルの再生](../runs/0052-workspace-replay-02/result.json)も12 groupsで一致した。reference callbackを実行せず、保存入力と観測へ52回のtarget呼び出しを照合した。raw sampleと順序は各groupのbench.json、[before](../runs/0052-workspace-bench-02/before-snapshot.json)・[after](../runs/0052-workspace-bench-02/after-snapshot.json)にsource/binaryのhashを残した。同一binaryの正常対照であり、高速化の実験ではない。最初の実行も保存した。親directoryが未作成のsnapshot保存をRedで確認してmkdir処理を追加し、最終sourceで02のsnapshot・比較・再生と各回帰を再実行した。出力directoryの修正で、checkerのアルゴリズムや測定条件は変えていない。

人工故障の実processテストは、観測値の改変、応答前のexit、応答後のexit、不正JSON、途中drift、unknown、存在しないprogramを検出した。とくに最後の正常JSONの後にexit 7で終了するケースは、最初の実装で見逃すRedを確認し、終了状態と余分なstdoutを検査して全速度比を無効にするよう修正した。バイナリ・sourceの改変、request差異、不正manifestも測定前に拒否する。これは人工故障による基盤の検証。

## bridgeの追加費用

従来のNode strict equalityと、新しいsnapshot/Wasm比較を、同じ常駐Node processのJSONL runnerで測った。実parser/coreの応答を入力に使い、7 samples・1 warmup・3 iterations、中央値を記録した。

| 入力               | 従来のNode比較 | snapshot/Wasm比較 |       差分 |
| ------------------ | -------------: | ----------------: | ---------: |
| rtlaの応答         |     1140.60 µs |        2028.29 µs | +887.69 µs |
| idealis-coreの応答 |       65.58 µs |         100.24 µs |  +34.65 µs |

[全sampleと範囲](../runs/0052-bridge-cost-02/measured/bench.json)と[保存オラクルへの再生](../runs/0052-bridge-cost-replay-02/result.json)を保存した。IPCとJSONを含む境界全体の値で、Wasm単体の関数時間ではない。JSON木の符号化とreport生成の追加費用があり、等値比較自体の高速化としては採用しない。今回はstatus・診断・保存再生の共通化に採用する。hot loopのkernel tuningには既存Criterion/profileを使い、変更前後の応答と境界費用をこのrunnerで検査する。

## 運用と再実行

just check/browser、oracle-test、cross-test/async-testと各eval/replay、runtime-build/test、evaluate/replay-evaluation、8 crateのpackageと単独consumerを確認した。packageの単独oracle consumerはtyped APIと新しいdispatchの両方を実行する。mutationは既存109件を再利用し104 killed・5 survived。Rustの新しいmutation scoreではない。Jevは35比較で矛盾・未評価0件。stagedとofflineの鮮度も確認する。

provenanceはoracleのsource・workspace adapter・実Wasm binaryのhashを含めるよう更新した。旧runの再実行は記録したsourceへ戻して行う。新しいsnapshotとrunは新しいdirectoryへ保存し、測定中は他のbuild/test/solver/profileを並走させなかった。

```sh
just workspace-oracle-test
just workspace-snapshot .artifacts/before
# 対象を修正し、release buildと回帰を行う
just workspace-snapshot .artifacts/after
just workspace-bench .artifacts/before .artifacts/after evals/runs/new-tuning
just workspace-replay evals/runs/new-tuning evals/runs/new-tuning-replay
```

TS adapterはworkspaceのeval用で、公開npm APIには追加していない。Goの既存EOF入力CLIは常駐JSONLではないため、3言語の既存runnerまたはcoldのprocess adapterで使う。汎用runnerの並列計測と自動timeoutは追加していない。

追記: [0053](0053-tlc-standard-modules.md)で参照環境と整数境界のtrace表記を修正し、24不一致を解消した。本iterationの過去runと判定は保持している。
