# roracle

Cargoの依存名とRustのimportは`roracle`。リポジトリ内の配置は`crates/roracle`。

Rustの`#[test]`から複数実装を同じオラクルへ照合し、同じ登録で速度を比較する試作ライブラリ。任意の入力型と`PartialEq`の観測型を使える。TLA+やSMTの検査器、テストランナー、外部プロセスの実行機能は内蔵しない。

## テストへの組み込み

```rust
use roracle::{Observation, OracleGroup};

#[test]
fn sum_matches_formula() -> Result<(), roracle::Error> {
    let mut group = OracleGroup::new("sum");
    group
        .case("ten", 10_u64)
        .reference("formula", |input| Observation::value(input * (input + 1) / 2))
        .test_function("iterator", |input| {
            Observation::value((0..=*input).sum::<u64>())
        });
    group.check()?.assert_matched();
    Ok(())
}
```

[`#[test]`と`Result`](https://doc.rust-lang.org/reference/attributes/testing.html)を使い、入力・関数を登録する形は[CriterionのBenchmarkGroup](https://docs.rs/criterion/latest/criterion/struct.BenchmarkGroup.html)を参考にした。CriterionのAPI互換、統計検定、proc macroは提供しない。各公開report型は、保存と集計を呼び出し側で行うために公開している。

`reference`は1つの独立した基準。`Value`との等値か、`Allowed`の許容値集合への包含を調べる。対象実装は単一の`Value`を返す。`Allowed`が空なら`invalid`、対象実装が`Allowed`を返しても`invalid`になる。`unknown`・`invalid`・`error`は、両者が同じ理由でも一致にしない。複数の結果の集約優先順位は`error > mismatch > unknown > invalid > matched`。空のsuite、重複ID、不足した登録は設定エラーとして返す。

入力と観測には所有したimmutableなスナップショットを使う。型に内包された共有可変状態までは検査できない。SUTの初期化、観測の射影、実装間の表現の正規化はcallback側が担当する。callbackの`Result`は`Observation::from(result)`で実行エラーとして保存できる。panicは通常のRustテストと同じように伝播する。

## 保存と再生

`CheckReport<I, O>`は入力・reference・各実装の観測・判定を保持する。`I`と`O`がserdeに対応していればJSONなどへ保存できる。

```rust,ignore
let saved = serde_json::to_string(&group.check()?)?;
let recorded = serde_json::from_str(&saved)?;
group.replay(&recorded)?.assert_matched();
```

`replay`は保存入力で対象実装だけを実行し、保存referenceへ照合する。reference callbackは呼ばない。同じgroup/reference/実装IDを登録する。case未登録なら保存caseを使い、登録済みなら入力も含めて一致を要求する。保存判定は再計算し、件数やID、集約statusの不整合を拒否する。保存データの真正性、ソース・バイナリのhash、モデル版の固定はホストの責任。

## 比較ベンチマーク

```rust,ignore
use roracle::BenchConfig;

let report = group.bench(BenchConfig {
    baseline: "iterator".into(),
    samples: 20,
    warmup: 3,
    iterations: 10,
    ..BenchConfig::default()
})?;
report.assert_matched();
```

正しさのreferenceと、速度比のbaselineは別に指定する。referenceはpreflightでcaseごとに1回実行し、計測しない。対象実装のpreflightが一致した場合だけwarmupと測定へ進む。全呼び出しの出力を、時計を止めた後に保存referenceへ照合する。途中で不一致や未確定結果が出れば、全caseの速度比を取り消し、部分sampleと失敗箇所を返す。

測るのはcallback呼び出しと出力構築。入力登録・出力の比較・破棄は時計の外に置く。SUT初期化をcallback内で行えば、その費用も含む。[Criterionのtiming loop](https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html)とは計測境界が異なるため、比較時は`scope`へ条件を記録する。

sampleごとに開始実装を回し、隣のsampleで順序を逆にする。reportには実行順と、固定iterationsの合計nsを保存する。mean/median/min/maxは1呼び出し当たりのns。速度比は`baselineのmedian / 対象のmedian`で、対象が速いほど1を超える。信頼区間や有意差は出さない。`max_evaluations`はreference・preflight・warmupを含む予定呼び出し数の上限で、壁時計のtimeoutではない。

nativeの`bench`は`Instant`を使う。Wasmは`bench_with_clock(config, clock)`へ単調増加するns時計を渡す。0時間・逆行・累積時間overflowは`unknown`にし、速度比を返さない。JSON ABIは観測batchの照合を扱う。benchmarkのcallbackとclockのJS bindingは提供しない。実行例は[comparison.rs](examples/comparison.rs)、外部実装比較は[評価アダプタ](../../evals/experiments/oracle-comparison)を参照。

## 対応範囲

[`idealis-connect`](../idealis-connect)は有限生成器・feedback・縮小を管理する。このcrateは固定入力に対する実装登録・照合・測定・reportを担当する。生成したcaseを`case`へ登録して併用できる。自動PBT、縮小、並列実行、プロセス隔離、任意言語のbindingはこのcrateには含まない。[対応チェックリスト](SUPPORT.md)を参照。

## 観測batchのJSON境界

`dispatch(&[u8])`はJSON Valueを、`dispatch_json(&[u8])`はUTF-8のJSON bytesを返す。bytes APIは中間Valueを作らずに全reportをserializeする。JSONの内容は同じで、objectのkey順は保証しない。native JSONL executable `roracle`は、ホストが収集済みの観測を同じOracleGroupへ通す。version 1の入力例は[observations.rs](examples/observations.rs)を参照。`group`・`reference`・`implementations`、caseごとの`id`・`input`・`reference`・実装順の`actual`を渡す。出力は`{kind:"checked", report}`または`{kind:"error", message}`。不正な入力をmatchedのreportにしない。

入力は16 MiB、4096 cases、256 implementationsまで。caseと実装は空にできず、IDの重複と観測数のずれを拒否する。JSONの構文と入れ子の制限もserde_jsonに従う。観測は`kind`とvariantに必要なfieldだけを受理する。公開Observationのserdeと同じ`[kind, payload]`形式も受理する。wire専用visitorでpayloadを一度だけ読み、公開ObservationのDeserialize実装は変えない。不正なvariantに対するmessageの詳細は旧版と変わる場合がある。この境界は収集済み観測の照合であり、実装関数の実行や計測は行わない。

Wasmは`oracle_abi_version`、`oracle_input_resize`、`oracle_dispatch`、`oracle_output_ptr/len`とmemoryをexportする。inputへUTF-8 JSONを書き、dispatch後にoutputをコピーする。過大なresizeは0を返して前のinputを消す。bufferは次のresize/dispatchで無効になり、runtimeへの呼び出しは直列にする。JS bindingはworkspaceの[eval adapter](../../evals/experiments/workspace-oracle/README.md)へ置き、npm公開APIには追加していない。

`dispatch_value(serde_json::Value)`も同じRequestとOracleGroupを使う。文字列を返すホストは `dispatch_value_to_json` を使うと、検査済みの応答を直接 JSON に書き出せる。どちらもcanonical serializationで16 MiBを検査し、深すぎる入れ子は再帰的にdropせず拒否する。文字列表現や重複fieldを保持する必要がある場合はbytes APIを使う。

## RustからTS型を生成する

開発用の`typescript` featureで`typescript::definitions()`を有効にする。`just oracle-types`はrequest/reportのTS宣言を生成し、`just oracle-types-check`は保存した宣言との一致を要求する。通常のbuildではts-rsを有効にしない。serdeの未知field拒否など、実行時の制約はRust側に残る。ts-rsが出す`deny_unknown_fields`未対応の警告は、TS型だけではこの制約を表せないことを示す。

[napi-rsの実験binding](../../evals/experiments/roracle-native/README.md)も同じ生成型を参照する。bindingはこのcrateの依存に含めず、既存のWasm ABIを維持する。型生成の初回対象はroracleで、他crateの全DTOにはまだ広げていない。

## JSON境界のCriterionベンチ

`just oracle-criterion`で単独・32 cases・入れ子256要素・unknownの4入力を測る。Valueとbytesの2出力を分け、入力の構築と出力の破棄は時計の外へ置く。parse・照合・全reportの生成は時計内に含む。Criterionは開発用依存で、配布libraryの実行時依存には含まない。

変更前後は同じbenchの実行ファイルを保存し、buildやtestと同時に測定しない。[0054の比較](../../evals/iterations/0054-oracle-bench-tuning.md)と[workspace手順](../../evals/experiments/workspace-oracle/README.md)を参照。

[0055の比較](../../evals/iterations/0055-oracle-hotspots.md)は、JSON観測の読み直しとJS snapshotの中間配列を減らした結果を記録する。native/Wasmの境界、Nodeの全照合、IPC込みの往復は別々に測る。
