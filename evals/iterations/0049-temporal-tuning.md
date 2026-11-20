# 0049: 時間論理の訪問済み式を最適化する

## 判断と残る課題

時間論理の展開に限定した最適化として採用する。256式以下のpoolでは訪問済み式を`[u64; 4]`へ保持し、257式以上では従来のHashSetを使う。展開順、pendingの判定、workのtick、SCCと反例の構築を変えていない。新しい依存や公開APIは追加していない。

交互測定では、量化WFのcheckが1.39倍、量化SFが1.35倍になった。ただし41本のうち8本には、meanの集約で3%以上の後退が残った。全ワークロードの性能改善とはしない。対照の後退が実装・実行ファイルの配置・共有機の負荷のどれによるかは切り分けていない。次の調査では、専用機または負荷を固定した環境でこれらを再検査する。

## profileで選んだ変更

[計画](../plans/0049-temporal-tuning.md)を実装前に記録した。TLC revision 5a47802のTBPar/TBGraphも読み、今回の変更は式の集合への挿入・所属の内部表現だけに限定した。TLCの探索方式を移植する変更ではない。

[変更前](../runs/0049-profile-before/result.json)と[変更後](../runs/0049-profile-after/result.json)で、3入力を各10,000回checkした。nativeのsamply 1,000 Hzで記録し、prepared IRを反復する。parse/compileは反復前に1回行い、結果の比較も各回行う。profileのsample数を実行時間の測定値へ読み替えない。Cargoのcompiler crateのrelease設定を使い、productionのidealis-coreのLTO設定とは区別する。

| 入力             | 全sample before → after | HashSet挿入のinclusive sample | Expansion::cloneのinclusive sample | work（各回、両実装同値） |
| ---------------- | ----------------------: | ----------------------------: | ---------------------------------: | -----------------------: |
| forall WF        |               664 → 522 |                       139 → 0 |                            76 → 49 |                    1,032 |
| 間欠的有効化のWF |           8,613 → 6,737 |                     1,182 → 5 |                        1,133 → 760 |                   14,500 |
| 間欠的有効化のSF |          11,245 → 7,668 |                     1,646 → 7 |                        1,319 → 870 |                   16,961 |

挿入の行は、HashMap<usize, ()>のinsertとしてsymbolicateされたsample。SCC側などの集合操作も含む。inclusiveの行同士は重なるため加算しない。全結果もbefore/afterで一致した。[beforeの集計](../runs/0049-profile-before-summary/result.json)と[afterの集計](../runs/0049-profile-after-summary/result.json)は保存profileのdigestを検査して再集計した。

小さい式集合ではHashSetのhash・rehashと分岐clone時の確保を減らせる。大きなpoolでは、分岐ごとに全pool分のbitmapを確保しないようHashSetを残した。大きいモデルでの高速化やメモリの厳密な上限は保証しない。

## 交互測定の結果

[初回compiler 27本](../runs/0049-criterion-comparison/result.json)と[既存14本の対照](../runs/0049-controls-comparison/result.json)を保存した。初回afterは未変更のコンパイル処理まで大きくぶれたため、結果を捨てずに再測定条件をplanへ追加した。

[再測定](../runs/0049-alternating/result.json)では、Aをgit archiveした0e9212ac、Bを候補とした。3 crateのsource.tarと4つの実行ファイルのhashを記録し、全buildを測定前に済ませた。各benchmarkをA/B/B/Aの順で測り、2回のmeanの算術平均を各実装の値とした。各回20 sample、warmup 0.5 s、measurement 1 s。個別の95%区間は保存するが、下表の集約値にはpooled CIの意味を付けない。計測中はこの作業のbuild/test/profileを並走させていない。専用機ではない。

| prepared IRのcheck      | before平均 |  after平均 |  速度比 | 時間の短縮 |
| ----------------------- | ---------: | ---------: | ------: | ---------: |
| 既存のtemporal-fair     |  10.401 µs |   7.342 µs | 1.417倍 |      29.4% |
| forall WF               |  63.084 µs |  45.399 µs | 1.390倍 |      28.0% |
| 間欠的有効化のforall SF | 928.580 µs | 689.142 µs | 1.347倍 |      25.8% |

どの対象も2回のafterで改善した。ASTからのcompile、parser、IPC、process起動の高速化は主張しない。TLCとの速度比でもない。

次の8件は、再測定でもpoint estimateが3%以上後退した。都合の悪いsampleを捨てたりmedianへ切り替えたりしていない。

| 対照                             | before平均（µs） | after平均（µs） | 時間の増加 |
| -------------------------------- | ---------------: | --------------: | ---------: |
| check/power-min                  |            0.444 |           0.539 |      21.5% |
| compile/action-constraint        |           10.566 |          11.040 |       4.5% |
| operators-compile/assert-true    |           10.345 |          10.842 |       4.8% |
| operators-compile/sort-ascending |           24.066 |          24.989 |       3.8% |
| membership/parallel-channels     |          882.761 |         911.413 |       3.2% |
| search/function-cube/1           |        9,934.020 |      10,616.345 |       6.9% |
| search/linear-ring/1             |        1,102.933 |       1,137.005 |       3.1% |
| search/parallel-channels/8       |        7,006.233 |       7,614.902 |       8.7% |

power-minのafterは0.453 µsと0.625 µsで、後者の95%区間は0.504-0.790 µsだった。この揺れを含めて平均した。3%のflagは統計的有意差の判定ではない。対象checkの改善とprofile上の費用減少を採用根拠とするが、対照の後退も課題として残す。

## 結果・予算・反例の一致

変更前のproduction native/Wasmを別pathへ保存した。[変更前の対照](../runs/0049-parity-control/result.json)、[候補](../runs/0049-parity-after/result.json)、[最終確認](../runs/0049-parity-final/result.json)で、156ケース・各10要求の完全な応答を4つのengineへ照合した。compile結果、通常のcheckと小さいwork/state/depth/temporal予算を含む。work、状態・生成数、反例、エラーのcode/messageも一致した。時間の短縮とwork予算の削減を混同しない。

16/17/64/65要素の時間論理forallでword境界と256式の切り替えの両側を検査した。正常版と活性違反、work切れをbeforeでも通し、候補でも維持した。空domain、未実行branch、不正IRは既存corpusとRust回帰で検査する。

[52ケースのTLC比較](../runs/0049-tlc/result.json)と[再実行](../runs/0049-tlc-replay/result.json)も一致し、4つの活性反例を元のSPECIFICATIONへ戻して再生した。Chromiumで同じRust/Wasmを実行した。Wasmでの速度測定は今回行っていない。

## 回帰と証拠

crateのfmt/clippy/test、just check/browser、portable-build/test、既存の安全性テスト213件、runtime-build/test、evaluate/replay、npmと7 crateの単独consumerを通した。mutationは109件の保存結果を再利用した結果で、新しいRust最適化のmutation評価ではない。

SUPPORT.mdとJevを同期し、30比較で矛盾・未評価は0件だった。stagedのオフライン検査も通した。通常のCriterion4 runは計344 artifact、交互測定は990 artifactのdigestを確認した。交互測定の41集約値を個別meanから再計算した。raw profile・log・不利な結果も残している。

```sh
just temporal-profile evals/runs/new-temporal-profile
just temporal-criterion new-temporal evals/runs/new-temporal-criterion
just temporal-controls new-controls evals/runs/new-temporal-controls
just temporal-alternating evals/runs/new-temporal-alternating
just temporal-parity .artifacts/0049-before evals/runs/new-temporal-parity
```

parityのbefore binaryは`.artifacts/0049-before`へこの作業開始時に保存したもので、repositoryへ実行ファイル自体は含めない。再実験時はbaselineのnative/Wasmとmanifestを先に用意する。交互測定用のbaselineはsource.tarから構築する。
