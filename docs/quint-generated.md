# Quintから生成したTLA+を実行する

Quint 0.33.1がApalache 0.63.0経由で生成したTLA+を、書き換えずにRust frontendとrtlcで検査する。6モデルの正常なinvariantと違反するinvariant、計12件をnative/WasmとTLCへ渡し、判定・出力される状態数と生成数を照合した。6件の反例も、生成元のTLA+をimportする別モジュールでTLCへ再生した。[入力と生成物](../fixtures/quint-generated)、[評価0044](../evals/iterations/0044-quint-generated.md)を参照。

| Quintの入力  | 確認した経路                           | 正常版の状態数 / 生成数 |
| ------------ | -------------------------------------- | ----------------------- |
| Counter      | 有限counter、条件式、次状態の代入      | 4 / 5                   |
| SetFold      | 整数集合のfold、LET内のoperator        | 1 / 2                   |
| SequenceFold | 順序に依存する左fold                   | 1 / 2                   |
| Choice       | 有限集合からの非決定選択               | 3 / 10                  |
| Match        | sum type、Variants、OTHERのないCASE    | 3 / 4                   |
| NestedFold   | sequenceと集合の入れ子fold、束縛の捕捉 | 1 / 2                   |

通常の回帰は保存した生成物を使い、Quint・Apalache・JVMを起動しない。省略v1・明示v1・明示v2を検査し、コンパイルされたversionも確認する。native/Wasmの38テストは、sourceの直接検査とコンパイル済みIRの検査、TLCの判定・計数、正常/違反の対照を扱う。実Chromiumでも3通りのversionで同じ12入力を実行する。v1とv2のwork量が同じとは仮定しない。

```sh
just quint-test
just browser
```

同じ6生成物をCriterionでも測れる。crate内のbench用fixtureは生成TLA+とTLCオラクルから作り、回帰で原本との一致を確認する。解析（moduleとconfig）、解析済みASTのcompile、コンパイル済みIRのcheck、parse+compile+checkを分け、compile以降はv1/v2で比較する。各測定の前に正常版の完全探索・状態数・生成数をTLCの値と照合する。合計42測定で、結果の破棄は各区間に含める。JVM起動、Quint生成、JSONL輸送は含めない。

```sh
just quint-bench .artifacts/quint-bench-01
just quint-bench-replay .artifacts/quint-bench-01
```

新しいrunにsource/toolchain/CPU、コマンド、IR bytes・定義数・work、Criterionの20 sampleと中央値・95%区間を保存する。runの上書きを拒否し、再生はdigestと測定の欠落を検査する。CIのcargo all-targetsはbenchをsmoke実行し、共有runnerの時間を合否の閾値にしない。[計画](../evals/plans/quint-sharing-regression.md)を参照。

[初回の測定](../evals/iterations/quint-sharing-regression.md)では、この小さい6モデルの全体時間はv1が有利だった。v2の全体は約1.06〜1.67倍かかり、共有IRの効果を一般的な高速化とは扱わない。

TLCを含めて再実行するには、Java、TLC jar、Apalache 0.63.0のjarと、新しいrunのパスを指定する。TLCは同じ生成物と設定を使う。12設定を各v1/v2のnative/Wasmと比較し、各versionの反例をTLCへ再生する。Apalache jarは標準モジュールの読み取りに使い、SMT求解はしない。

```sh
just quint-eval /path/to/java /path/to/tla2tools.jar /path/to/apalache.jar evals/runs/my-quint-run
just quint-replay /path/to/java /path/to/tla2tools.jar /path/to/apalache.jar evals/runs/my-quint-run evals/runs/my-quint-replay
```

TLA+を再生成する場合はApalache serverを起動し、別プロセスから新しい出力先へ生成する。`quint-generate`は`pnpm dlx @informalsystems/quint@0.33.1`を使う。既存のrunとfixtureを上書きしない。

```sh
java -Xmx512m -jar /path/to/apalache.jar server --port=18822
just quint-generate .artifacts/quint-regenerated localhost:18822 /path/to/apalache.jar
```

保存するTLA+は`quint compile --target=tlaplus`のstdoutそのもの。0.33.1では`--out`にQuintのJSONが書かれ、TLA+はstdoutへ出るため、生成器は`--out`を使わない。コマンド・stderr・入力と生成物のSHA-256をmanifestへ残す。[QuintのCLI資料](https://quint.sh/docs/quint)を参照。

対応はこの有限fragmentの範囲に限る。`:=`はroot moduleが直接`EXTENDS Apalache`を指定した場合だけ、同モジュールの定義どおり等号として扱う。`EXTENDS TLC`は受理するが、任意のTLC operatorを実装するものではない。

VariantsはApalache 0.63.0の関数表現に合わせ、`Variant(tag, value)`、`VariantTag(value)`、`VariantGetUnsafe(tag, value)`を扱う。`VariantTag`はdomainが一つの文字列keyである関数に限定し、空・複数key・非文字列keyを拒否する。一般のCHOOSE、Variantsの他のoperator、古いVariantsモジュールの別表現への互換性は含めない。集合foldの文字列・recordと一部の関数順の制限は[compilerの対応範囲](../crates/rtla-compiler/README.md)を参照。

Quint言語を直接解釈するAPI、Quint ConnectのDriver/API、ITFのimportは提供しない。今回確認したのは生成TLA+の実行であり、Quint全体やApalache全体への互換性、形式モデルと実装の適合性の証明を意味しない。
