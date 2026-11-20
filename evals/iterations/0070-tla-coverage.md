# 0070: portable TLA+ frontendの追加対応

基準は328aeeef。[先行計画](../plans/0070-tla-coverage.md)に沿い、構文解析と有限モデルの実行を別に検査した。proofは構造の保持が対象で、証明の検証は行わない。

## 実装した範囲

rtlaは固定Operators.javaの101 canonical operatorと有効alias、Unicodeの構造aliasを解析する。label引数、一般selector、引数付き・LET内INSTANCE、nested module、LAMBDA、RECURSIVE、ASSUME/PROVEとproof stepを型付きASTに保持する。大きい整数はInteger.textへ原文を保持する。

syntaxVersionは1.7.4/1.8.0の優先順位表を選ぶ。文法とUnicodeは共通で、旧SANYのCASE/submoduleの不具合や廃止SetExcept文法を再現しない。省略時は従来の優先順位を保つ。

Rust compilerは名前・LAMBDAの高階operatorを静的に特殊化し、selectorとmodule scopeを解決する。v2にもmodule連結・設定置換とSelectSeq/SortSeq/Assertを追加した。再帰expressionは明示v2で実行し、強連結成分のphase依存を固定点で検査する。非停止はbounded/evaluation、work枯渇はbounded/workになる。

引数なしLETの自由な`@`は宣言側、引数あり定義は適用側の環境に従う。異なるEXCEPTへ入る引数なし定義はv2の遅延引数へ変換する。v1と旧TS frontendの該当組合せは、誤った値を返す代わりに理由を示して拒否する。特殊化・selectorの反例には公開operator名を使う。

## 独立した比較

| 検査                    | 入力・比較                                                                      | 結果                                        |
| ----------------------- | ------------------------------------------------------------------------------- | ------------------------------------------- |
| SANY grammar            | 公式355・prefix対照13・全aliasの適用/定義/宣言/参照・境界を含む1,558入力 × 二版 | 3,116観測。記録した世代差501件以外の不一致0 |
| native / Wasm parser    | 同じ入力とsyntaxVersion                                                         | 完全なJSON応答が一致                        |
| SANY構文木              | 両方が受理した2,521組                                                           | 正規化後の構造が一致                        |
| native / Wasm / TLC実行 | 対応例・不正宣言・反例をIR v1/v2で比較                                          | 105件中87 matched・18 rejected、不一致0     |

jarのSHA、corpus revision、入力、native/Wasm応答、SANYの生の木とprocess出力は[保存索引](../runs/0070-tla-coverage/archives.json)にある。1.7.4の空ファイルで起きる参照側例外1件はinfrastructureとして分け、syntax一致へ数えない。参照を正順・逆順のbatchで実行し、状態の持ち越しによる差がないことも検査した。

構文木の比較はalias、括弧、位置情報、同時binderの表現を正規化する。moduleの項目は型ごとの配列へ揃え、operatorの結合、引数順、束縛pattern、selector、proof階層を比較する。未知のnodeは比較失敗となる。位置情報や全SANYの意味解析を検証したという主張ではない。

実行比較はnative/Wasmのcompile・check応答、TLCの判定・状態/生成数と、違反の強制trace再生を検査する。不正入力の拒否はmatchedと別に数える。各対応例には小さいwork予算も与え、打ち切りを成功にしない。

## 修正中に見つけた境界

- `F(\\X)`だけでなく、空白を入れたASCIIの`\\X`・`\\times`もSANYはoperator引数として拒否する。Unicodeの`×`の構文受理とは区別した。
- まとめたbinderのsource位置が同じでも、変数名ごとに捕捉の識別情報を分ける必要があった。
- nested moduleの外側の定数を、内側のWITH対象として扱ってはいけなかった。
- 引数付きINSTANCEでは、暗黙再帰関数をmodule引数の付加前に正規化する必要があった。
- LETの`@`を一律に宣言側または適用側へ固定する案は不正だった。TLCの引数なし/引数あり対照を追加した。

失敗した生成・比較・Red/Greenのログは保存索引に残す。初回Criterionは測定前のmanifestがなく、ガードが拒否したためinvalid-provenanceとした。最終の測定とは混ぜない。

## Criterion

7つの固定caseでparse、v2 compile、コンパイル済みprogramのcheckを分けた。20 samples、warmup 500 ms、measurement 1秒。[最終測定の索引](../runs/0070-tla-coverage/archives-completion.json)を参照。起動・JSON・JVMの費用を含む速度比較ではなく、機能ごとの費用を追跡するためのkernel benchmarkである。入力とsource hashを測定前に固定し、測定中の変更を拒否する。

単位は平均µs。CPUや測定条件を揃えた旧版との比較ではないため、改善率は主張しない。

| case               | parse | compile v2 | prepared check |
| ------------------ | ----: | ---------: | -------------: |
| named              | 25.92 |     105.57 |          10.22 |
| label              | 24.73 |      67.07 |           5.12 |
| factorial          | 23.32 |      79.17 |          20.45 |
| instance-parameter | 23.86 |     134.27 |           7.67 |
| select-lambda      | 19.23 |      53.97 |           6.99 |
| fold-recursive     | 28.63 |      67.23 |          20.41 |
| except-zero-arity  | 35.04 |     134.87 |          12.41 |

## 残る制約

再帰する代入actionの生成、stateへ格納する一般のoperator値、全SANYの型・level検査、proofの証明検証は対象外。暗黙再帰関数の直接keyとDOMAINを最適化するが、関数値全体の構築はeagerであり、任意のaliasを介した完全なlazy関数値ではない。既存の有限値・式・評価深さの上限も維持する。詳細は[parser](../../crates/rtla/SUPPORT.md)、[compiler](../../crates/rtla-compiler/SUPPORT.md)、[checker](../../crates/rtlc/SUPPORT.md)のチェックリストに記録する。

## 終了時の検査

実行比較の最終runは57入力のIR variantと9故障対照の105件。native/Wasmの完全応答、TLCの判定・計数、違反traceが一致した87件と、不正宣言を双方が拒否した18件を分けて記録する。[追加の保存索引](../runs/0070-tla-coverage/archives-completion.json)に最終実行・Criterion・Jev初回の指摘を保存した。JevがINSTANCEの実装抜粋だけではLET内の対応を確認できなかったため、既存のmodule/INSTANCE挙動テストを比較contextへ加えた。

全体の回帰結果とsource不変検査は[最終verification](../runs/0070-verification-final/result.json)へ記録する。旧runの失敗は削除せず、修正後のrunと分ける。

初回の[全チェック](../runs/0070-verification/result.json)はソース不変のまま失敗した。原因は追加前の拒否期待、Jev生成中のmanifest整形、Goのstdin読み取り停止だった。旧期待はTLCの保存ログと追加の10ケースで確認し、成功・反例・再帰のboundedを検査するテストへ更新した。修正後はanalysis 4,419件、safety 1,185件が通った。Goの同じ188,380 bytesの入力は個別実行と3言語検査の再実行で通り、停止した初回は失敗として残す。追加の再生では10本のTLA programと改善ループの10 witnessが再現した。最終verificationはJevの完了後にソースを固定して行う。
