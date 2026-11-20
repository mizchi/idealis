# 0053: TLCの24不一致を解消する

0052で残った24件は、TLCの標準module検索pathと反例の整数表記を修正して解消した。[集計と旧結果との照合](../runs/0053-resolution/result.json)を保存した。元の155入力、モデルsource/config、native/Wasmの応答はすべて同一で、native/Wasm/TLCのbinary hashも変わっていない。仕様や期待値を変えて一致にした結果ではない。

## 検査と再生

| 実行                                                       | ケース | 一致 | 既知の拒否 | 不一致 | 反例再生 |
| ---------------------------------------------------------- | -----: | ---: | ---------: | -----: | -------: |
| [development](../runs/0053-tlc-development-02/result.json) |    155 |  153 |          2 |      0 |      105 |
| [confirmation](../runs/0053-tlc-confirmation/result.json)  |     32 |   32 |          0 |      0 |       19 |
| [保存結果のreplay](../runs/0053-tlc-replay/result.json)    |    155 |  153 |          2 |      0 |      105 |

2件は従来から明示したTLCの文字列literal制限による拒否。正常な意味論の一致へ数えていない。confirmationは既存の別seedによる32グラフで、同じfixtureを使う回帰確認。未知の実装バグ検出力を測定した結果ではない。replayでは保存した入力とreportが完全一致し、source・binary・標準moduleのdigestも検証した。

## 不足していた参照module

24件の元仕様はApalacheをimportするが、TLCのjarだけではそのmoduleを発見できなかった。公式[v0.56.1のApalache.tla](https://raw.githubusercontent.com/apalache-mc/apalache/v0.56.1/src/tla/Apalache.tla)を、既存fold対照と同じSHA-256 `a9115b404e72c58fd085f81bc59cc918f8adac49feabfb57f8e285bc7ab5cfac`へ固定した。手元のApalache 0.63.0 jar内の同名moduleもこのdigestと一致した。

`just tlc-modules`で検証したsourceを.artifactsへ準備する。TLCの[SimpleFilenameToStream](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/util/SimpleFilenameToStream.java)の`TLA-Library` system propertyで検索pathを渡し、元モデル・witness・tautology時の追加検査で共有する。現在directoryの明示moduleが優先するTLCの規則も維持した。標準module原本はgitへ追加せず、URL・path・digestをmanifestへ保存する。検査中はnetwork取得を行わない。

module不足は`input-error`から`infrastructure-error`へ分類する。モデルの評価エラーと同じ理由で一致した扱いにしない。欠落・改変されたcacheは検査前に拒否し、既存cacheを黙って置き換えない。

## 参照を直した後に見つかった整数境界

[最初の実行](../runs/0053-tlc-development/result.json)では元のモデル判定はすべて揃ったが、3件のwitnessが失敗した。int32の最小値を`-2147483648`と出力すると、TLCの[SpecProcessor](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/tool/impl/SpecProcessor.java)が正の整数token `2147483648`を拒否する。

この値だけを`(-2147483647 - 1)`へ出力する。[Naturals.Minus](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/module/Naturals.java)とIntegers.Minusの実装を確認し、値を変えずにTLCで表現できることを検査した。list・set・mapのkey/valueへ再帰的に適用し、元仕様の判定と保存したtraceの値は維持する。境界値を含む実TLCテストで失敗するRedを確認してから修正し、3件とも反例再生まで一致した。最初のrunは編集していない。

## 再発防止

6本の分類テスト、2本のcache検証、2本の実TLCテストを通した。空白を含む検索path、ローカルmodule優先、空sequenceでcallbackを評価しないこと、module欠落、入れ子の整数境界を確認する。CIには固定moduleの準備と、分類・cacheの8テストを追加した。実TLCの2テストは`just tlc-diff-test <java> <jar>`で実行する。

共通のcollections/temporal 49テスト、just check/browser、runtime-build/test、evaluate/replay-evaluationも通った。mutationは従来109件を再利用し104 killed・5 survivedで、新しいrendererのmutation scoreではない。crateの対応範囲は変わらず、Jevの35比較とoffline/stagedの鮮度を確認した。

[再実行手順](../tlc-differential/README.md)を参照。公開artifactのdigest一覧は[resolutionのindex](../runs/0053-resolution/published-artifacts.json)へ保存し、TLCの一時statesファイルを除いている。
