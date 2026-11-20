# 0061: symmetry拡張と探索の削減

[計画](../plans/0061-symmetry-next.md)に従い、ringの診断、record/集合、代表展開、並列化、命令ID、PORの順に実装・評価した。基準は62eac486。Rust nativeのprepared探索を主指標にし、Wasmは移植性と挙動を照合する。起動込みの速度比をpreparedの値から主張しない。

## ringの再測定

[再測定](../runs/0061-ring-diagnosis-fixed/result.json)は凍結した0060-before/finalを8回、ABBAとBAABで実行した。Criterionは20 samples、warmup 0.5秒、measurement 1秒。通常ringの平均所要時間増は1-worker要求で約0.7%、8-worker要求で約3.2%だった。0060の約11%増は再現しなかった。原因を特定したとは扱わず、初回の変動も残す。ringの8-worker要求は既存の逐次fallback。

最初の[driverの失敗](../runs/0061-ring-diagnosis/manifest.json)はCriterion出力directoryのslashをunderscoreと取り違えた。最初の測定出力を残し、修正して別runを作った。

## recordとnode集合

node関数のpayloadへflatなInt/Bool recordを追加した。各関数内でfield名・型を揃え、node参照とnested値を拒否する。node部分集合は所属Boolを同じnode行へ加える。record全体のcanonical identityと集合所属を行へまとめ、変数間の相関を維持する。

TLCのTool.getSymmetryPerms、RecordValue.permute、SetEnumValue.permute、FcnRcdValue.permuteを参照した。TLC cloneはd8fb231de4e4b5d02c1b2ed1ac759c086f694021。置換で変わらないrecord payloadと、nodeとともに動く集合所属を分ける。宣言のequivarianceは従来と同じ利用者の前提で、自動証明とは扱わない。

[段階1のCriterion](../runs/0061-stage1-criterion/result.json)は、同じbinaryのplain/symmetryを2回、2回目は順序を反転して測った。各回20 samples、warmup 0.5秒、measurement 1秒。表は2回のmean推定値の算術平均で、pooled confidence intervalではない。

| モデル           | plain ms | symmetry ms | 前/後 |
| ---------------- | -------: | ----------: | ----: |
| record N5、6値   |   20.500 |       2.033 |  10.1 |
| 2つのnode集合 N5 |    3.267 |       0.390 |  8.38 |

recordは7,776→252状態、2集合は1,024→56状態。[TLC27件](../runs/0061-stage1-tlc/result.json)は判定・利用可能な計数が一致し、反例は元のNextで再生した。最初の集合fixtureはIFの括弧が不足して評価エラーになった。sourceを直して通常探索の対照も確認した。新しい実対象の未知バグを発見した結果ではない。

## node操作の代表化

SafetyOptionsを追加し、既定では全nodeを展開する。symmetry_actionsを指定した場合だけ、先頭の単一node bindingを同じ現在行の代表へ絞る。Init・複数binding・tuple bindingは通常展開、全群以外のdomainは全候補へ戻す。worker展開でも同じ代表選択を使う。generatedとworkは削減後の実行に対する値。

| モデル        | 状態縮約のみ ms | 代表展開 ms | 前/後 |
| ------------- | --------------: | ----------: | ----: |
| counter N5/K3 |          0.1724 |      0.1269 |  1.36 |
| counter N7/K5 |          3.6574 |      2.5300 |  1.45 |
| 2 Bool関数 N5 |          0.3203 |      0.2304 |  1.39 |
| record N5     |          1.9107 |      1.5375 |  1.24 |
| 2集合 N5      |          0.3794 |      0.2796 |  1.36 |

[段階2のCriterion](../runs/0061-stage2-criterion/result.json)も同じsample条件・逆順の2回。[TLC27件](../runs/0061-stage2-tlc-fixed/result.json)では生成数を比較対象から外すpolicyを明示し、判定・状態数・反例長と具体反例を照合した。最初の比較器は生成数をnullにしてreferenceをinfrastructure-errorに分類した。[失敗run](../runs/0061-stage2-tlc/error.json)を残し、raw referenceを保って比較policyだけを指定する形へ修正した。通常の比較では生成数の不一致も引き続き失敗にする。

## symmetryと並列frontier

rootへの値移送後に逐次にkeyを照合する。symmetry時の閾値は64状態で、未満は逐次へ戻す。代表展開と併用し、work予算は既存のworker予約を使う。小さいquotientだけで並列効果を結論しないため、計画へN12 recordを追加してから測った。

| record N12、6,188状態 | 1 worker ms | 8 workers ms | 前/後 |
| --------------------- | ----------: | -----------: | ----: |
| 全node展開            |     180.101 |      173.567 |  1.04 |
| 代表展開              |      87.100 |       74.292 |  1.17 |

[段階3のCriterion](../runs/0061-stage3-criterion/result.json)は同じbinary・同じsample条件・逆順の2回。代表展開の52,417生成と全展開の148,513生成を区別する。全展開の差は小さく、任意のモデルで8 workerが速いとは結論しない。対応する形状に限定して採用する。

## 命令IDの試作は不採用

式ごとのHashMap検索を使わず、Vec添字の命令列、stack、条件jumpでscalar branchを実行した。未対応branchは既存評価器へ戻した。型確認の順序、短絡、overflow、代入の復元、workと通常の応答はテストで一致した。

| モデル          | 通常 ms | 命令列 ms |
| --------------- | ------: | --------: |
| counter-product |   9.664 |    13.482 |
| linear-ring     |  1.1242 |    1.1306 |

[修正後の測定](../runs/0061-stage4-criterion-fixed/result.json)は同じbinary・sample条件・逆順の2回。counterは約40%後退し、ringも有用な短縮がないため撤去した。[試作source](../runs/0061-stage4-criterion-fixed/trial-source.tar.gz)とRed/Green・型確認のlogを保存した。最初のrunはringをmaxDepth=1000で実行し、boundedで停止した。[出力](../runs/0061-stage4-criterion/0/process.log)を残し、既存benchと同じ100000へ直して全ケースを取り直した。

## 単調な独立flagに限定したPOR

他のbranchとinvariantが参照せず、他のbranchが戻せないBool flagをFALSE→TRUEにする操作だけを先に実行する。選択のたびに未設定flagが減ることを循環回避の条件にする。未知の式・依存・型不正は通常展開へ戻す。SYMMETRYとの併用は拒否する。temporal/fairness・constraint・initial/action propertyは通常探索。POR時はnativeも逐次実行する。

[段階5のCriterion](../runs/0061-stage5-criterion/result.json)ではN10の独立flagと4状態のvisible counterが4,096状態・24,577生成から14状態・15生成へ減った。prepared探索は3.7373→0.010524 ms、約355倍。同じbinary・sample条件・逆順の2回による人工モデルの結果で、一般PORの速度や保証へ広げない。

[TLC10件](../runs/0061-stage5-tlc/result.json)で判定を比較した。生成数・状態数・元のグラフの最短反例長は比較対象外と明示し、具体反例は元のNextで再生した。可視flag、依存guard、循環、別操作からの参照はfallbackの対照にした。PORの反例は有効でも、通常BFSより長くなる場合がある。

## 検証と残る範囲

record・集合、代表展開、並列展開、限定PORを採用する。命令列は不採用。一般のSYMMETRY・POR、peer参照、nested record、複数群、時間論理との削減併用は未対応。[利用方法と契約](../../docs/reductions.md)、[rtlc checklist](../../crates/rtlc/SUPPORT.md)へ境界を記録する。

TypeScriptの通常buildが、0060でraw evidenceへ保存した.tsをコンパイルして失敗する問題も確認した。tsconfigからevals/runsを除外し、保存済みsourceを変更せずにbuildを復旧した。package consumerはRequest::CheckSafetyの新しいoptions fieldに追従し、配布したRust/TS/Wasmでも実際のrecord代表展開とPORを検査する。

## 最終版の確認と計測範囲

[最終21本](../runs/0061-final-criterion/result.json)はrecord/集合の最初の反復だけ数倍遅くなった。例えばrecord N12の逐次・全展開は1,123.9/208.9 msだった。この平均から一般的な性能を結論せず、[追加計画](../plans/0061-native-resources.md)へ条件を記録してから、同じ凍結binary・sample条件で対象を絞って取り直した。初回の全sampleを残す。

| 最終版の確認測定      | 全展開 ms | 代表展開 ms |
| --------------------- | --------: | ----------: |
| record N5、逐次       |     2.116 |       1.600 |
| 2集合 N5、逐次        |    0.4106 |      0.2841 |
| record N12、1 worker  |   195.820 |      92.594 |
| record N12、8 workers |   179.297 |      80.946 |

[確認run](../runs/0061-confirmation-criterion/result.json)は2回のmean推定値の平均。record N12の代表展開は92.887/92.301 msと83.303/78.589 ms（1/8 worker）。最初の大きな変動は再現しなかったが、原因を特定したとは扱わない。

[通常14本](../runs/0061-final-controls/result.json)のABBAではcounterの8-worker要求が9.507→11.206 ms、約18%増になった。変更後の2回は9.580/12.831 msで、片方が遅かった。[counterの8反復](../runs/0061-counter-confirmation/result.json)では1-worker要求が約1.2%増、8-worker要求は約0.5%減で、18%増は再現しなかった。[ringの8反復](../runs/0061-ring-confirmation/result.json)は1-worker要求が約4.1%減、8-worker要求が約3.8%増。変更前にも遅い反復があり、改善や後退の原因は未特定として残す。counter/ringのworker要求はこの入力では逐次fallback。

## 起動とRSS

[native resource run](../runs/0061-native-resources/result.json)は同じ最終Rust CLIのfresh processを1 warmup後3回測った。表はwallとmaximum RSSをそれぞれ独立に取った中央値。macOSのtime -lが返すbyteをMiBへ換算した。wallにはNode計測driverのJSON serialization・spawnSyncとtime wrapperを含む。Node hostの起動とTLA+ frontendは含まない。メモリ上限や専有ホストでの測定ではない。

| 入力と経路                    | wall ms | peak RSS MiB |
| ----------------------------- | ------: | -----------: |
| counter N7/K5 plain           |  629.92 |        65.33 |
| counter N7/K5 symmetry        |    8.00 |         3.53 |
| counter N7/K5 actions         |    6.69 |         3.20 |
| record N5 plain               |   24.87 |         4.98 |
| record N5 symmetry            |    5.51 |         2.84 |
| record N5 actions             |    5.47 |         2.80 |
| record N12 actions、1 worker  |   99.27 |        12.81 |
| record N12 actions、8 workers |   84.59 |        15.17 |
| monotone N10 plain            |    6.89 |         3.58 |
| monotone N10 POR              |    3.70 |         2.31 |

小さいモデルでは起動・JSON境界の費用が残る。PORのpreparedでの数百倍をfresh processの倍率へ使わない。record N12の8 workerはこの測定でwallを減らす一方、ピークRSSが増える。

## 最終回帰

[旧版照合](../runs/0061-final-parity/result.json)は172入力・1,720コマンドのnative/Wasm応答全体が一致した。[TLC既存155件](../runs/0061-final-existing-tlc/result.json)は153一致・想定拒否2件・不一致0。[symmetry27件](../runs/0061-final-tlc/result.json)、[代表展開27件](../runs/0061-final-actions-tlc/result.json)、[POR10件](../runs/0061-final-por-tlc/result.json)も不一致0で、具体反例を元のNextへ再生した。既存seedの回帰であり、新しい独立した確認実験とは数えない。

check、portable-test、browser、safety-test、runtime-build/test、evaluate/replay-evaluation、criterion-test、packageと追加のRust/Node contract検査を通した。ブラウザでもrecord・集合・代表展開・PORを検査する。共有テストのmutationはconformance/counterの既存2対象で95.4%、104 killed・5 survived・133 compile error、109件を実行して再利用0。Rustの削減や今回のAPIのmutation scoreではない。

SUPPORT/Jevは8 crate・41比較でlive/offlineの矛盾・未判定0。fixtureが256 KBを超えてJevのsource上限に達したため、小モデル15件とscaling/POR 6件のJSONへ分割した。結合した値は元の21件と完全一致し、検査対象から除外していない。過大なcontextはflat recordの検証を独立比較へ分けた。最終版のfmt/clippyと単独consumerも通過した。

最終版の検証結果、source/binary hashと途中の失敗は[検証記録](../runs/0061-verification/result.json)へ保存する。測定した各段階と最終版のsourceは区別する。
