# operator共有のTLC対照

2026-11-17にTLC v1.7.4、revision `5a47802b5c391f59ecdd44117981f4ff8c0656ba`で取得した19ログ。[計画0048](../../plans/0048-operator-sharing.md)の4 familyを17入力の独立cohortとして扱い、宣言順で拒否された先行2入力も別に保持する。既存compiler corpusへ追加しない。

jarのSHA-256は`936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88`。Apalache 0.56.1の`tla2sany/StandardModules/Apalache.tla`は`a9115b404e72c58fd085f81bc59cc918f8adac49feabfb57f8e285bc7ab5cfac`。両方を使用前と継続時に検証した。Apalache原本はリポジトリに含めず、必要な2入力のscratchにだけ配置した。実行環境、source/config/log digest、終了コード、評価エラーの文言と位置は[expected.json](expected.json)に記録する。期待値はTLCログだけから作り、idealisの出力から補わない。

各入力を隔離したcase directoryに`Differential.tla` / `Differential.cfg`として保存し、scratch内のtmpをJVMの一時ディレクトリにする。JVMは1つずつ、1 worker、256 MiB、60秒のprocess timeoutで実行した。

```sh
timeout 60 java -Xmx256m -Djava.io.tmpdir="$TLC_TMP" -cp "$TLC_JAR" tlc2.TLC -workers 1 -nowarning Differential.tla
```

stdoutとstderrは同じストリームへ取得順に保存した。[CASE fixture](../tlc-case/README.md)と同じ置換だけを次の順に適用した。パスはPythonの`str.replace`、残りは`re.sub`による。

| パターン | 置換後 |
| --- | --- |
| `str(temporary_directory)` | `<tlc-tmp>` |
| `str(case_directory)` | `<capture>` |
| `\[pid: \d+\]` | `[pid: <pid>]` |
| `\bon \d+ cores\b` | `on <cores> cores` |
| `\([^\n]*, (MSBDiskFPSet, DiskStateQueue)\)` | `(<host>, \1)` |
| `\b\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\b` | `<time>` |

メッセージ、位置、計数、seed、fingerprintは変更しない。継続時に全19ログをscratchのraw logからこの規則で再変換し、保存bytesと完全一致することを確認した。先行のsource/config/log/expectedは保持し、再採取で上書きしていない。

| family | ケース | TLC判定 | 状態数 / 生成数 |
| --- | --- | --- | --- |
| 共有と課金 | `sharing-chain-10/20/30/60`, `sharing-work-60` | success | 各1 / 2 |
| lexical frame | `sharing-lexical-capture`, `sharing-binder-quantifier/function/filter/map-set/fold-sequence/fold-set/tuple-pattern` | success | 各1 / 2 |
| 遅延引数 | `sharing-unused-argument` | success | 1 / 2 |
| 使用引数の対照 | `sharing-used-argument` | 評価エラー、終了75 | 1 / 1 |
| phase | `sharing-prime`, `sharing-enabled` | success | 各2 / 3 |
| 宣言順の先行入力 | `sharing-prime-forward-reference`, `sharing-enabled-forward-reference` | input-error、終了150 | — |

先行2入力ではNextがActより前に置かれ、TLCが`Unknown operator: Act`を報告した。修正版ではId / Act / Primeの宣言だけをNextより前に移した。式・初期状態・遷移・性質・設定は変更していない。先行入力と拒否ログも保持し、この条件変更を[iteration](../../iterations/0048-operator-sharing.md)へ記録する。

`sharing-work-60`のTLC successは十分予算の意味論の対照であり、idealisのcall-by-nameの費用とは区別する。v2ではこの入力だけmaxWork 256でbounded/workを要求する。他の小モデルはmaxDepth 64、maxStates 10000、maxWork 1000000。未使用引数はv1ですでに正常で、使用時のHeadエラーと人工的なeager評価故障を対にする。

[v1-baseline.json](v1-baseline.json)はv2の変更前に新規保存した。`fbf0d664`の全239 compiler入力（両frontendとも237受理・2拒否）と、新規17入力について、TS/Rust frontendのJSON serialize bytesとnative compile応答のSHA-256、および型付き拒否を保持する。入力digest・revision・件数も固定し、既定と明示`irVersion: 1`の現在出力から計算したSHA-256をそれぞれ保存digestへ比較する。baselineは期待意味論のオラクルには使わない。

step 2では`irVersion: 2`は型付き未対応を返す。回帰はその結果に成功を要求するassertionでRedとなる。v2の定義表・call・parameter・captureは未追加なので、不正IRのcycle/ID/arity/capture/slot/phaseとW=4の直接評価は正常IRの受理対照とともに次のIR実装stepへpendingとする。version拒否をこれらの検証実装済みとは数えない。

step 3cではUNCHANGEDのtuple別名・parameter・nested別名の3入力を追加した。同じjar、1 worker、256 MiB、60秒で一件ずつ採取し、各ログはsuccess、1状態・生成2件だった。追加goldenは[expected-step3c.json](expected-step3c.json)へ分離し、既存expectedとv1 baselineは保持する。cohortは追加3件のv1/v2を検査し、保存baselineのbyte一致は元の239＋17入力に限定して維持する。深い前向きchain・スコープ費用・多arm CASEの合成入力はcompile-only対照で、TLC由来の追加入力へは数えない。

step 3d-aでは生成的外殻inlineの字句frame喪失を8入力で固定した。同じcapture手順・jar・上限で一件ずつ実行し、[expected-step3d-a.json](expected-step3d-a.json)へログ由来の結果だけを保存する。EXCEPTの`sharing-generative-except`は末尾`= 4`で初期状態`x = 0`のSafe違反、終了12。TLCはこの初期違反時に状態数・生成数の集計を出さないため、その2欄はgoldenへ補わず、初期反例だけを記録する。末尾`= 9`の正常対照と、binderのguard付き/レビューそのままの再現・各TRUE対照、membershipのNat直接/alias版の計7入力はsuccess、各1状態・生成2件だった。

binderの先行2入力にはSafeの前に`x = 0`を連言したため、現v2の最初の診断がレビューのbinding countと異なった。先行入力・ログを残したまま、連言だけを除いた`sharing-generative-binder-exact`と`sharing-generative-binder-exact-control`を追加した。条件追加は[iteration](../../iterations/0048-operator-sharing.md)へ記録する。v2は全入力にTLC結果・frontend構造一致・native/Wasm応答一致を要求し、現在の失敗をRedとして残す。v1は従来の誤束縛/型付き拒否も現状assertionとして別に固定し、TLC適合の成功とは数えない。既存expected・ログ・v1 baselineは変更しない。

step 3e-aはaction actualのcaller frameを要求する7入力を追加した。同じ固定jarとcapture手順で一件ずつ取得し、[expected-step3e-a.json](expected-step3e-a.json)へTLCログ由来の期待値だけを保存した。実装から取得したv1の現状は[v1-current-step3e-a.json](v1-current-step3e-a.json)へ別に保存し、TLC oracleや既存v1 byte baselineを置き換えない。frameを必要とするaction callの拒否は許容結果にせず、v2の受理・意味結果・構造・native/Wasm parityを要求するRedである。

| 入力 | TLC結果 | 状態数 / 生成数 | 現v1 |
| --- | --- | --- | --- |
| sharing-action-frame-exists | success | 3 / 7 | complete |
| sharing-action-frame-two-variables | success | 3 / 7 | complete |
| sharing-action-frame-binder-actual | success | 2 / 3 | Invalid bound variable slot |
| sharing-action-frame-except | 初期Safe違反、x=0 | 集計なし | completeという既存誤合格 |
| sharing-action-frame-except-control | success | 1 / 2 | 初期Safe違反という既存偽違反 |
| sharing-action-frame-shadow | success | 3 / 7 | complete |
| sharing-action-frame-enabled | success | 1 / 2 | complete |

EXCEPTの再現はcaller old=9をcallee old=4へ取り違えると誤合格し、比較を9へ変えた対照は偽違反になる。TLCが初期違反ログへ出していない集計をgoldenへ補わない。既存239入力のv2検査でもupstream-channel/parallel-channelsの受理とv1意味結果を要求し、既知Unicode拒否2件は別に維持する。これは有限の回帰入力であり、全action/temporal形の対応や実装完了を意味しない。

step 3e-bでは、前提レビューの値依存ENABLED・prime・temporalと、従来拒否していたgenerative actionを新規tiny入力として追加した。[expected-step3e-b.json](expected-step3e-b.json)は同じ固定TLC・capture手順で一件ずつ取得したログだけから作成し、既存expected・ログ・v1 baselineは変更していない。LET定義本体の自由な`@`については引数あり・なしの2入力をtyped protocol拒否の対照とし、TLC互換のcapture規則を新たに主張しない。

| 入力 | TLC結果 | 状態数 / 生成数 |
| --- | --- | --- |
| sharing-action-frame-enabled-value | success | 1 / 2 |
| sharing-action-frame-generative | success | 1 / 2 |
| sharing-action-frame-prime | Safe違反、x=0からx=1 | 2 / 2 |
| sharing-action-frame-temporal | temporal違反、x=0からstuttering | 1 / 2 |
| sharing-action-frame-temporal-state | temporal formulaを処理できず終了255 | 集計なし |
| sharing-temporal-true-step3e-b | success | 1 / 2 |
| sharing-temporal-forall-step3e-b | success | 1 / 2 |

レビューの`Temporal(p) == [](p)`へ定数のbinder式を渡す入力は、実行前の予想と異なり固定TLCがtemporal違反を報告した。原入力・ログを保持し、独立した状態依存actualと、helperを通さない`[]TRUE` / `[](forall)`の対照を追加した。状態依存actualはTLCの未対応診断で、受理の成功例には数えない。直接の2対照はsuccessだった。条件追加は結果と分けて記録し、temporal helperのTLC結果と実装の差を合格へ置き換えない。

新規6入力の意味比較はv2を対象とし、既存v1の現状snapshotと257 byte guardは維持する。primeのTLC反例はaction名`Step`も保存し、checkerとのoracle比較ではproperty名・深さ・状態列を検査する。TS/Rust構造とnative/Wasmの完全応答の比較ではcheckerのaction名も含める。

## step 3e-c: 記録済みTLC乖離

[divergences.json](divergences.json)の唯一の乖離は`sharing-action-frame-temporal`である。`Temporal(p) == [](p)`へ渡す`(\A b \in {0}: b = 0)`はTRUEなので、性質は`[]TRUE`として全behaviourで成立する。保存TLCのexit13・temporal違反・初期x=0からstuttering・状態数1/生成数2とログdigestを固定し、goldenやログは変更しない。v2のTS/Rustはcomplete・状態数1/生成数2・深さ0・work11を要求し、正規化構造とnative/Wasm完全応答を検査する。乖離記録自体のdigestもテストで固定し、正常入力への誤登録と記録に一致しないログ・違反をrunnerが拒否する対照を置く。

固定revisionでは直接boxが[不変条件へ分類](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/tool/impl/SpecProcessor.java#L1349-L1392)される一方、operator applicationはliveness経路へ進む。ソース読取では、否定後の`<>FALSE`が畳まれず、FALSE粒子が[整合と扱われ](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/tool/liveness/TBPar.java#L243-L265)、[promiseも充足と扱われる](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/tool/liveness/TBPar.java#L343-L345)ことが保存違反を説明する。原因は固定ソースからの推論で、内部tableauの計装確認や他revisionへの一般化ではない。詳細な公開ソースリンクは乖離記録に含める。

直接の`sharing-temporal-true-step3e-b`と`sharing-temporal-forall-step3e-b`は従来のTLC success goldenとの一致と同じ意味結果を要求し、workは各5/7として別に固定する。この乖離をTLC一致へ数えない。`sharing-action-frame-temporal-state`は従来のTLC未対応（exit255）のままで、一致にも乖離にも数えない。他のfixtureは従来どおりTLC goldenへ照合する。

## step 3f-b1: 解析parityの追加対照

[expected-step3f.json](expected-step3f.json)に10本を追加した。同じ固定TLC・jar digest・capture手順で、一件ずつ1 worker、256 MiB、60秒のtimeoutで取得した。CASEの値条件、notin formal、有限CHOOSEのdomain/predicateにあるcaller binder、CHOOSE自体をactualへ渡すdomain/predicateのcaller oldを検査する。共通Initはx=0、NextはUNCHANGED x。CHOOSEの新規正常対照はsingleton domainを使い、選択順の互換性を広げない。

| 入力 | TLC結果 | 状態数 / 生成数 |
| --- | --- | --- |
| sharing-case-condition-step3f | success | 1 / 2 |
| sharing-notin-formal-step3f | success | 1 / 2 |
| sharing-choose-domain-caller-step3f | success | 1 / 2 |
| sharing-choose-predicate-caller-step3f | success | 1 / 2 |
| sharing-choose-old-domain-argument-step3f | success | 1 / 2 |
| sharing-choose-old-predicate-argument-step3f | success | 1 / 2 |
| 後4入力の名前に-violationを付けた比較対照 | constant invariant拒否、終了151 | 集計なし |

比較だけを0・0・4・4へ変えた4対照は、採取前の初期Safe違反という予測と異なり、全て`The invariant of Safe is equal to FALSE`で拒否された。TLCはこれらの反例・状態数・生成数を出していないため補わない。原入力・ログを保持し、初期反例やsuccessへ置き換えない。

0046のchoose-order、choose-equivalent、choose-tuple、choose-nested、choose-capture、choose-no-witness、choose-empty、choose-lazy、choose-predicate-error、choose-tuple-errorは保存runを読取のみで再利用する。各source/config/logとreports.json/manifest.jsonのdigestを新goldenのreusedChoose/reusedRunFilesへ固定した。既存expected・ログ・v1 baseline・乖離記録は変更していない。

## step 3f-b20: レビュー指摘の先行TLC対照

[expected-step3f-b20.json](expected-step3f-b20.json)に12本を追加した。固定TLC・jar digest・capture手順は上と同じで、一件ずつ1 worker、256 MiB、60秒のtimeoutで取得した。期待値はTLCログだけから作り、idealisの出力から補わない。既存のexpected・ログ・v1 baseline・乖離記録は変更していない。

| 入力 | TLC結果 | 状態数 / 生成数 |
| --- | --- | --- |
| sharing-framed-negation-step3f-b20 | Safe違反、x=0からx=1 | 2 / 2 |
| sharing-framed-negation-constraint-step3f-b20 | Safe違反、x=0からx=1 | 1 / 2 |
| sharing-framed-conjunction-step3f-b20 | Safe違反、x=0からx=1 | 2 / 2 |
| sharing-framed-conjunction-constraint-step3f-b20 | Safe違反、x=0からx=1 | 1 / 2 |
| sharing-unframed-negation-constraint-step3f-b20 | Safe違反、x=0からx=1 | 1 / 2 |
| sharing-unframed-conjunction-constraint-step3f-b20 | Safe違反、x=0からx=1 | 1 / 2 |
| sharing-framed-forall-step3f-b20 | success | 1 / 2 |
| sharing-framed-forall-negated-step3f-b20 | 初期状態でProperty Safe違反 | 集計なし |
| sharing-framed-exists-negated-step3f-b20 | 初期状態でProperty Safe違反 | 集計なし |
| sharing-tuple-unchanged-6-step3f-b20 | success | 1 / 2 |
| sharing-tuple-specification-6-step3f-b20 | success | 1 / 2 |
| sharing-tuple-specification-16-step3f-b20 | success | 1 / 2 |

framedの4入力は`Id(p) == p`を介した状態述語で、CONSTRAINTあり・なしの両方を取った。CONSTRAINTありでは、制約の外へ出る後続状態にもTLCはinvariantを検査して違反を報告する。深さはidealisの規約（TLCの報告値−1）でgoldenに記録した。unframedの2入力は、枠を介さない同じ式の対照である。TLCは初期状態のProperty違反に集計を出さないので、goldenへ補わない。

tupleの3入力は`V0 == x`、`Vn == <<V(n-1), V(n-1)>>`の連鎖で、TLCはどれも受理する。n >= 19のtuple連鎖をidealisが拒否する上限はTLCの意味論ではなく、v1が従来から課す入れ子の上限であり、goldenには入れない。
