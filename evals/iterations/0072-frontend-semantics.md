# 0072: source level、遅延関数、共有template

基準は3fec1611f。[先行計画](../plans/0072-frontend-semantics.md)に従い、Rust frontendの意味検査、alias/actualを通る遅延関数、非再帰action/temporalの共有を実装した。対応する有限fragmentは[契約](../../docs/frontend-semantics.md)と各crateのSUPPORTへ記録する。

## sourceと実行の対照

ローカルのtlaplus/tlaplus（d8fb231de4e4b5d02c1b2ed1ac759c086f694021）で、[BuiltInOperators.java](https://github.com/tlaplus/tlaplus/blob/d8fb231de4e4b5d02c1b2ed1ac759c086f694021/tlatools/org.lamport.tlatools/src/tla2sany/semantic/BuiltInOperators.java)のoperand上限とlevel weightsを読んだ。OpApplNodeのactual検査、ModuleNodeの再帰operator制約も対照にした。ToolとFcnLambdaValueではkeepLazy、宣言時state snapshot、選択と全値の実体化を調べた。

source levelの解析は、actualの受理上限と戻り値への依存を分ける。未使用定義、formal/capture/LET、再帰の制約を固定点で伝播する。5000定義の前向きchainも既存回帰で検査する。全SANYの名前・型・proof検査は追加しない。

遅延関数はv2の選択とDOMAINでalias/actual・IF・record/tupleを通過する。current/next/locals/old/prime/frameを所有するsnapshotから要求keyを評価する。等値や状態保存の完全観測は有限domainを実体化し、要求したエラーを隠さない。EXCEPT target、fold、membershipなどすべての観測位置でのTLC lazy互換は対象外。値cacheとwireのlazy tagは追加しない。

非再帰actionとtemporalは、frameが必要な呼出しだけを共有する。零引数でframe不要の呼出しはinlineを維持する。body kind、phase、arity、captureと循環を検査し、PROPERTYのatom/always分類と旧展開上限を保つ。TSも非再帰actionを共有し、temporalの自動共有はRustが行う。

## 独立した照合

固定57件と、修正条件で先に固定したseed 1172/2172の生成24件を実行した。固定入力にはlevelの正常/不正対照、関数の値をinvariantで確認する13件、未要求のエラー/非停止、要求時のエラー、snapshot、共有action/temporalを含む。

| 経路                                          | 結果                                                                      |
| --------------------------------------------- | ------------------------------------------------------------------------- |
| native/Wasmのcompile/check完全応答            | 81件一致                                                                  |
| TLCの判定・状態数・生成数、違反の強制反例再生 | 64 matched、不一致0                                                       |
| 不正なsource level                            | 12 rejected。TLCも入力拒否                                                |
| 要求した関数値のエラー                        | 4 model errors。TLCも評価エラー                                           |
| 非停止する要求key                             | 1 bounded/evaluation。TLC一致へ数えない                                   |
| Chromium                                      | 7テスト成功。追加81件は64 matched・12 rejected・4 model errors・1 bounded |

検査結果を返す各ケースにはwork 1でのbounded/work対照を置く。意図した故障はinvariant 12件とtemporal 1件で、未知の実バグ発見とは数えない。判定比較は既存のidealis-oracle経路を使い、モデルと実装で遷移を共有しない。native/Wasmのsource.zip、source/binary hashとdriverをruntime-3/execution-3に保存した。

## 条件修正と失敗

最初の単体対照68件では54 pass・14 failだった。正常版のliveness入力にfairnessがなく、stutteringでも性質が成り立つという前提が誤っていた。加えて、固定TLCはstate predicateをtemporal helperのactualにした入力を未対応として拒否した。このTLC実行はstuttering反例を確認した実行ではない。

入力を定数actualへ変更し、必要なliveness対照にWFを加えた。生成対照はstutteringでも真になるstate invariantと定数actualを使う。旧条件とseed 1072/2072はconditions-1.zipへ保存した。両seedは既に観測したため、条件修正後の確認には再利用せず、[別plan](../plans/0072-frontend-properties-2.json)の1172/2172を使った。修正後68件の独立runを残し、その後に値invariantの13対照を追加した。

解析回帰の初回2 runには113件・85件の失敗があった。共有表現でも仮想的なaction展開上限を守る処理、PROPERTY抽出後の未到達temporal定義の除去、独立reach解析でのcall追跡と構造正規化を修正した。temporal callは検証後にframeへ展開して旧表現と比較し、TLCとの実行比較にはこの正規化を使わない。

遅延した関数選択は一時的な全関数のinternを省くため、既存の小さい対照2件のworkが13→12、11→10になる。歴史的なgoldenやrunは変更しない。次の共有化比較では、この遅延化を既にbaselineへ含め、workが変わらないことを別に確認した。

## Criterionと採用

共有化前Aと共有化後Bの実行ファイルを固定し、A B B A、B A A Bの順で各4回測った。各runは20 samples、warmup 500 ms、measurement 1秒。Apple M5の非専有環境で、同じtoolchain・入力・benchを使う。表は各runの平均の中央値、単位µs。起動、IPC、JVMの費用は含めない。

baselineはlevel検査と遅延関数を含む。最初のsource snapshotは固定TLCで扱えないstate actualを含んでいたため、最初のtiming前に入力だけを修正した。保存source.zipから別の作業領域へ展開し、修正fixtureだけを重ねてbefore-sharing-2を再ビルドした。旧snapshotも残す。summaryはsource差分、toolchain/environmentとbench・fixture・lockfileの同一性を検査する。単一ファイルの変更だけを測った比較ではない。

| case              |      compile A → B |       fresh A → B |    prepared A → B | prepared倍率 |
| ----------------- | -----------------: | ----------------: | ----------------: | -----------: |
| action-repeat-4   |  695.181 → 144.358 |   43.647 → 30.284 |   23.570 → 22.033 |        1.07× |
| action-repeat-6   | 3181.000 → 220.402 | 164.949 → 106.057 |  108.445 → 90.971 |        1.19× |
| action-capture    |   127.575 → 94.508 |   22.401 → 12.964 |    12.380 → 7.489 |        1.65× |
| temporal-repeat-3 |  498.554 → 185.834 |   74.992 → 62.459 |   60.947 → 52.772 |        1.15× |
| temporal-repeat-5 | 2548.725 → 795.399 | 313.462 → 249.927 | 306.134 → 257.879 |        1.19× |
| scalar-control    |    21.231 → 19.307 |     2.015 → 1.740 |     0.714 → 0.676 |        1.06× |
| lazy-control      |  222.020 → 192.941 |   43.853 → 42.129 |     3.898 → 3.827 |        1.02× |

7入力の完全なSafetyResultとworkはA/Bで一致した。反復actionとtemporal helperのcompile/fresh/preparedに短縮が見られるため共有化を採用する。小さい対照も全sampleを残し、外れ値を削除しない。反復bodyを持つ人工入力の結果を一般的な倍率やTLCとの速度差へ拡張しない。

sampling profileはCriterionと重ねずに実行した。最初の1000反復は85/87 samplesと少なかったため、そのrunを残して100000反復でも診断した。samplesは8320→8206、eval_referenceのinclusive weightは1160→1156、Argument vector cloneは738→652だった。探索と評価の仕事が残ることを示す診断で、runtimeの改善全体をこの差へ帰属させない。profileのsample weightをwall時間の比として扱わない。

## 保存と回帰

[保存索引](../runs/0072-frontend-semantics/archives.json)に、入力、source.zip、binary hash、TLC raw出力、失敗とRed/Greenログ、Criterion全run、profileを保存する。生成したbuild source/target cacheと実行ファイルのコピーは除外する。benchmark sourceはsource.zipから再ビルドできる。

[全体verification](../runs/0072-verification/result.json)はlint・型・単体・解析・Rust・Wasm/Chromium・mutation・評価再生・配布consumerと対応表を検査する。Jevのlive比較、offline再生、stagedの鮮度検査を区別する。

最初の全体verificationは21項目中18項目が成功した。safety回帰には遅延化前のwork期待値が4件残っていたため、実行結果に対応する12/10へ更新し、その直前の予算ではbounded/workになる対照も加えた。対応表の2検査は、文書とリンク先のテストを修正した後の記録が古いため失敗した。どちらも元runを成功へ書き換えない。

全体run中の非Markdown sourceは不変だった。修正後の[追試](../runs/0072-final-verification/result.json)では、元の18成功項目を残し、Rust crate/SDK sourceがそのrunと同じhashであることを先に検査する。work境界、lint・型・単体、mutation、対応表の自己テストとoffline、safety評価・再生、改善ループ・再生を再検査する。

追試の9項目はすべて成功し、検証中のsourceは不変だった。解析回帰は4,571件、修正後のsafety回帰は1,185件が成功した。Jevはliveの8 crateで矛盾0・未評価0、offlineでも全比較に回答があり、APIキー未設定時のskipとは区別している。
