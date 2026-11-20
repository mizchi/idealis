# 0071: 再帰する代入actionと親frame共有

基準は23131b74。[先行計画](../plans/0071-recursive-actions.md)に従い、Rust frontendの明示IR v2から再帰するInit/Nextの代入actionを生成する。省略時のv1と旧TS frontendの受理範囲は維持する。

## 呼出しの契約

共有definition表へactionのbranch配列を登録し、Step callから実行する。expression/actionのkind、arity、capture、phase、slotと再帰SCCを検査する。calleeが生成した候補でcallerの残りstepsを実行し、backtrackで代入を戻す。引数とcaptureはcallerのbinder・EXCEPT oldを保持し、要求時のcurrent/next/primeで評価する。

TLCの[Tool.java](https://github.com/tlaplus/tlaplus/blob/d8fb231de4e4b5d02c1b2ed1ac759c086f694021/tlatools/org.lamport.tlatools/src/tlc2/tool/impl/Tool.java)で、Init/Nextのoperator context、遅延引数とconjunctionの継続を読んだ。再帰operator自身のformalをUNCHANGED対象にする形は、[ModuleNode.java](https://github.com/tlaplus/tlaplus/blob/d8fb231de4e4b5d02c1b2ed1ac759c086f694021/tlatools/org.lamport.tlatools/src/tla2sany/semantic/ModuleNode.java)と実行の両方でSANYの拒否を確認した。外側formalの有効なcaptureとは分ける。

## 独立した照合

固定26件と、先に固定したseed 1071/2071の生成24件を実行した。生成は尾再帰、相互再帰、LET捕捉、高階特殊化の4種類で、各入力に正常版と意図したinvariant故障を用意した。

| 経路                                           | 結果                                                       |
| ---------------------------------------------- | ---------------------------------------------------------- |
| native/Wasmのcompile・check完全応答            | 50件で一致                                                 |
| TLCの判定・状態数・生成数、違反の強制trace再生 | 47 matched、不一致0                                        |
| recursive formalの不正なUNCHANGED              | 1 rejected。TLCも入力拒否                                  |
| 非停止と再帰するゼロ引数Next                   | 2 bounded/evaluation。TLCの一致へ数えない                  |
| Chromium                                       | 6テスト成功。追加ケースは23 matched・1 rejected・2 bounded |

固定入力は分岐、存在量化、caller継続、prime、EXCEPT old、ENABLED、Init、INSTANCE、action property、複数引数も含む。work 1の切断と再帰128段の切断は成功へ読み替えない。準備済みprogramの再利用、4 workerの探索、未代入の次状態も別テストで検査する。生成故障の検出は、未知の実バグ発見ではない。

## 測定と採用

初期実装Aと親frame共有Bを、A B B A、B A A Bの順で各4回測った。各runは20 samples、warmup 500 ms、measurement 1秒。Apple M5の非専有環境で、同じtoolchain・入力・ベンチを使う。表は各runの平均の中央値、単位µs。compile、fresh check、prepared checkを分け、sampling中は測定しない。

| case           |  fresh A | fresh B | prepared A | prepared B | prepared短縮 |
| -------------- | -------: | ------: | ---------: | ---------: | -----------: |
| inline-control |    2.029 |   2.058 |      0.597 |      0.604 |        -1.2% |
| tail-12        |   23.206 |  21.619 |     14.767 |     14.727 |         0.3% |
| branching      |   15.236 |  14.164 |      4.671 |      4.680 |        -0.2% |
| captured       |   18.810 |  16.223 |      9.179 |      7.475 |        18.6% |
| target-capture |   18.462 |  16.065 |      9.779 |      8.086 |        17.3% |
| multi-argument | 1051.493 | 883.347 |   1036.806 |    808.600 |        22.0% |
| accumulator    |   23.641 |  23.626 |      3.672 |      3.624 |         1.3% |

sample profileでは初期実装の親frame複製が引数ごとに発生していた。変更後はcallごとに一つのimmutableな親frameを共有する。Vec<Argument>::cloneのsample weightはinclusive 103から45となった。総sample weightも異なるため、これをwall時間の比とは扱わない。値のmemoizationは追加せず、7入力のSafetyResultとwork値は完全一致した。

複数引数のprepared checkは約1.28倍となり、捕捉変数の2件でも短縮が再現したため採用する。最初のABBAだけでは一部ケースに退行が見えたため、順を逆転した測定も残す。外れ値を削除しない。compile測定は対照として保存し、runtime変更によるcompiler高速化は主張しない。JVM、IPC、起動を含むTLCとの速度比較ではない。

## 再現と失敗記録

[保存索引](../runs/0071-recursive-actions/archives.json)に入力、TLCのraw出力、source archive、binary hash、Criterion全run、profile、Red/Greenログを保存する。元のsource snapshotに不足していた変更のないrtla manifestsとCargoの別bench targetは、基準revisionとの同一性を検証した別archiveで補う。元の記録は変更しない。`rebuild.ts SNAPSHOT RUN`は保存ソースと補完からofflineで再ビルドし、7件の結果を元の実行ファイルと比較する。ビルド場所によるbinary hashの一致は要求しない。

初回TLC対照は不正formalの期待と`x=<<...>>`のtokenizationで失敗した。入力と期待を修正し、終了する固定24件と最終50件を新しいrunで比較した。257状態のringに深さ100では足りず、測定前に深さ1000へ変更した。初回のsource再ビルドは別benchファイルの不足で失敗し、その後の変更後版はディスク容量不足で失敗した。両方のログを保存する。解析テストの最初の2回はwire境界追加に対する固定登録表の更新漏れで失敗し、対応する5境界と項目名を同期した。

## 制約と回帰

再帰actionの生成はRust frontendの明示v2に限る。非再帰actionの自動共有、temporal template共有、全SANYのlevel検査、任意のaction構文は追加しない。再帰128段・既存の値や式の上限を維持する。[対応範囲](../../docs/recursive-actions.md)を参照。

全体の回帰とsource不変検査は[verification](../runs/0071-verification/result.json)へ記録する。Jevの対応表照合はliveとofflineで検査し、キーがない場合のskipと区別する。

全21項目のverificationは成功し、検証中のsourceは不変だった。その後のレビューで、TS decoderが`bodyKind: ["expression"]`を文字列へ変換して受理する拒否漏れを見つけた。Redを確認して直接の文字列比較へ修正し、native/Wasmの51テストが通った。最終変更のlint・解析・配布consumerと対応表の検査は[追加verification](../runs/0071-decoder-verification/result.json)へ記録する。

実行ファイルは[別の保存索引](../runs/0071-recursive-actions/executables.json)にも保存する。macOS arm64用であり、展開後に実行権限を戻す。最終検証と失敗ログの追加分は[完了時の索引](../runs/0071-recursive-actions/archives-completion.json)を参照。
