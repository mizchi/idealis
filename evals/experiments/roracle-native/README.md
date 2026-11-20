# roracleのnative binding比較

Rustのrequest/report型からTS宣言を生成し、同じroracleをWasmとnapi-rsで呼ぶ。native bindingは未公開の実験で、npm配布には含めない。既定のworkspace oracleはWasmを使う。

## 型を生成する

```sh
just oracle-types
just oracle-types-check
just oracle-native-test
```

`crates/roracle`の`typescript` featureで、`packages/idealis/src/generated/roracle.ts`を生成する。通常のRust/Wasmではtypegenを有効にしない。napi-rsの生成する`generated/index.d.ts`も同じ`OracleRequest`を参照する。CIは両生成物の鮮度と型エラーの対照を確認する。

TSの構造型は実行時の入力検査ではない。RustのserdeとOracleGroupがversion、必須field、未知field、空suite、重複ID、件数を検査する。生成型は推奨するobject形式を表し、既存のwire visitorが受理するlegacy sequence形式を追加のTS APIとして公開しない。

## Nodeで明示選択する

```sh
just oracle-native-test
IDEALIS_ORACLE_BACKEND=native-json node --test dist/tests/workspace-oracle/contract-test.js
IDEALIS_ORACLE_BACKEND=native-object node --test dist/tests/workspace-oracle/contract-test.js
```

`native-json`はJSON文字列をnapi-rsへ渡す。`native-object`はplainなJSON dataを渡し、`serde_json::Value`に変換する。両者とも出力はJSON文字列と共通decoderを使い、`__proto__`などをJSのsetterへ書き込まない。object入力にはcloneと検査の費用があり、JSON処理をすべて省く経路ではない。

object入力は循環、accessor、疎なarray、追加property、非列挙field、symbol、不正Unicode、非有限数、負のzeroを拒否する。任意のJS値には先に既存の`snapshot`を使う。Proxyの隔離や敵対的なJSコードのsandboxは提供しない。RustのValue境界はcanonical serializationで16 MiBとparser相当の入れ子制限を検査する。文字列表現を保持する必要がある入力はbytes経路を使う。

環境変数は初回のoracle生成前に設定する。native binaryが見つからない場合はエラーにし、別backendへfallbackしない。別OS向けのprebuilt binaryやloaderの配布は行わない。

## 保存して測定・再生する

```sh
just oracle-native-test
# buildとtestが終わってから単独で測る
just oracle-native-eval evals/runs/my-native-oracle
just oracle-native-replay evals/runs/my-native-oracle evals/runs/my-native-oracle-replay
```

[測定前の計画](../../plans/native-oracle.md)に従い、6入力・各30 samplesと別processのcoldを測る。warmはrequestのcodec、照合、report構築、共通decoderを含み、snapshotとSUTは含まない。各呼出の全reportを時計の外で照合する。coldの`initAndFirstUs`はbinaryの読み込み・requireまたはcompile/instance生成・初回requestを含む。`processMs`はNode起動とmodule importも含む。

runには入力、期待report、sampleごとの合計nsと件数、順序、source、binary、hashと環境を保存する。replayは同じplatform・arch・Node版で全reportとhashを検査し、統計を再計算する。異なる環境の速度再現性や、保存dataの真正性は保証しない。runは新しいdirectoryへ作り、上書きしない。

初回の[測定結果](../../iterations/native-oracle.md)ではnativeの既定化を見送った。`idealis-core`全体のDTO生成、探索器のnative binding、callbackやclockのbindingは今回の対象に含めない。

## object経路の費用を分ける

```sh
just oracle-native-test
just oracle-object-phases .artifacts/my-object-phases
just oracle-object-phases .artifacts/my-object-profile profile object-request nested-256
```

区間測定は入力検査・native呼出・stringify・decoder・全requestを分ける。JSONのnative呼出だけは事前生成した文字列を使う。profileの時間を高速化の比較に使わない。[調査結果とWITの設計候補](../../iterations/native-object-profile.md)を参照。

前後比較は凍結した変更前のJSとnative binaryを使う。保存証拠のarchiveに`before` snapshotと二つの候補を含めた。`before/manifest.json`の`files`へ相対pathとSHA-256を記録し、snapshot内に`dist/evals/experiments/roracle-native/client.js`とその依存、および`roracle-native.node`を置く。native binaryのhashが現在のbuildと異なる場合は、JSだけの比較として受理しない。

```sh
just oracle-object-compare .artifacts/saved-before .artifacts/my-object-comparison
just oracle-object-compare .artifacts/saved-before .artifacts/my-object-control --control
just oracle-object-replay .artifacts/saved-before .artifacts/my-object-comparison .artifacts/my-object-replay
```

比較は元の6入力と共有参照のない6入力を使い、全reportとJSON経路の後退を確認する。保存runは編集せず、新しいdirectoryへ再生する。hashは改変検知に使い、archiveの真正性は保証しない。

今回の候補は次の手順で再生できる。native artifactと同じdarwin arm64、同じNode版を使う。

続く[WIT/型付きpayloadの試作](../roracle-wit/README.md)では、汎用Valueを省いてもJS packingが大きく、JSON経路より遅かった。既定化せず、結果と再生を保存している。

```sh
mkdir -p .artifacts/saved-object-profile
tar -xzf evals/runs/native-object-profile/evidence.tar.gz -C .artifacts/saved-object-profile
just oracle-object-replay .artifacts/saved-object-profile/before .artifacts/saved-object-profile/comparison-02 .artifacts/my-object-replay
```
