# 0056: 探索本体の関数値を圧縮する

基準は3c11b826。0054/0055のoracle費用ではなくrtlcの関数・sequence値を対象にする。公開IR/Value、完全探索の範囲、状態数・生成数、work、反例と予算の扱いを維持する。TLCのローカルclone d8fb231のFcnRcdValue.select/takeExceptを読み、EXCEPTのdomain外・入れ子・元の関数の不変性を確認した。

## 仮説と実装候補

現行Cell::FunctionはVec<(Scalar,Scalar)>を持ち、EXCEPTはkeyも含めて全cloneする。immutable domainを共有し、値を別に持つ。64要素以下の全Boolean関数はu64へ詰め、更新とhashを定数時間にする。sequenceを含むその他の値は汎用Scalar列へfallbackする。小整数のpackingは必要性が確認できた場合に別stageで検討する。

domainと値のhash衝突では全体を比較する。generic関数の既存衝突fixtureを維持する。record/tuple/mapの外部表現と正規化、typed model valueの比較エラーを維持する。nativeのoverlay/mergeでdomain・payload両方のRefをremapする。新しいcrate依存と公開APIは追加しない。

## 比較条件

変更前のnative/Wasm、Criterion executable、sourceとhashを.artifactsへ保存する。profileは通常測定から分け、既存のcounter M24/function N16/channels N5/ring N100000をprepared IRで16回完全探索する。Node inspectorまたはsamplyを使う。実装候補の採否はprofilerを外した測定で決める。

大規模比較は既存single-modelのfunction N20（1048576 states / 20971521 generated）とchannels N7（2097152 / 22020097）を固定。比較routeはtla-ts 1、TLC 1/8、before/after native 1/8、before/after Wasm 1。各1 warmupと3測定を交互・逆順に配置する。同じsource/config、heap設定、同じNode・arm64 Java・TLC jar、同じホスト。source-to-resultのwall時間を主値とし、API区間・sampled RSSを補助として保存する。全実装のcomplete・状態数・生成数を検算し、unknown/不完全/エラーを速度比へ入れない。TLC区間とRust API区間は異なるのでkernel比にはしない。大規模再測定中にbuild/test/profileを並行しない。

Criterionは既存14本を同じbench sourceで20 samples・warmup 0.5秒・measurement 1秒、before/after/after/before。counter/ring、unknown等の対照を残し、10%以上の後退を明記する。新しい幅1/16/32/64/65と更新・membershipのbenchを追加する場合、同じharnessを先に基準へbuildしてから比較する。

主目標はfunctionでtla-ts/TLCへの速度差を縮め、実際に上回った条件を報告すること。channelsも同じ条件で測り、追い越せなかった場合も結果を残す。固定倍率を仮定しない。既存の小さい正常/違反モデルを別途全routeへ通す。

## TDDと互換性

共有domain、64/65境界、Boolean→一般値→Booleanのcanonical identity、domain外/入れ子EXCEPT、collision、overlayのdomainとpayload Ref競合をテストする。内部表現のテストだけでなく既存213件等の有限モデル、native/Wasmの全応答・予算・反例を基準へ照合し、TLC差分と反例再生を行う。

終了前にfmt/clippy、rtlc/portable/safety/criterion/check/browser、runtime、evaluate/replay、cross/async、package consumer、SUPPORT/Jevを確認する。測定・失敗runは上書きしない。採用判断と残るTLCとの差をiterationへ記す。ローカルcommitまでとし、PR/push/mergeは含めない。

## 入力の型を確認した補足

既存のfunction-cubeはTLA+のBOOLEANではなく整数0/1を使う。channel内のval/rdy/ackも整数0/1だった。最初の候補に、64要素以下の全Int(0/1)を保持する別tagのbit表現を含める。BoolとIntを混同せず、混在・範囲外の値・65要素以上はScalar列へ戻す。任意の小整数packingは今回は含めない。

## 初期候補Aの並列後退を受けた候補B

大規模比較の途中で、Aはfunctionのnative 1が約6.5→3.9秒になった一方、8 workersでは後退が見えた。channelsのwarmupもnative 8が約6.6→9.0秒だった。Aのrunは最後まで保存する。mergeが、既にroot Arenaに属するdomainにも毎回key remap判定とdomain pool検索を行っていた。

Bではdomainにroot所属のimmutable metadataを持たせ、既にrootのdomainはArcをそのまま移送する。workerで新規作成したdomainは従来どおりkeyをremapしroot poolへ登録する。所属metadataは値の等値性・hashに含めない。Aの実行ファイルは変えず、測定中にbuild/testを並行しない。Bを同じ固定条件で新しいrunへ測り直し、最終採否はBの結果で判断する。

## packed値の型保証をmembershipで使う候補C

AのWasm functionは9.54→5.79秒になったが、tla-tsの5.54秒には届かなかった。membershipは全生成候補で関数のkey/value列を構築し直し、全要素を検査していた。Cは値だけのowned iteratorを使う。packedの全Bool/全Int(0/1)と、完全に一致する有限rangeだけは表現上membershipが保証されるため、domainの一致を検査後に論理workを要素数分consumeしてTRUEを返す。

worker内のwork割当ての順序を変えないため、この一括consumeはbudget付きoverlayでは無効にする。誤ったdomain、混在型、異なるrange、広い関数と非有限rangeは既存の順序へfallbackする。rootの予算境界の全応答が一致することを、0/1とBool、幅0/1/16/32/64/65、型不一致・狭いrangeの追加caseで凍結基準へ検査する。Cを最終候補として同じ大規模条件で新しいrunへ測定する。A/Bの測定は初期候補として残す。

## 並列domain参照カウンタの候補D

Cでも8 workersの後退が残った。全workerが同じroot domainのArcカウンタを候補更新ごとに増減していた。Dはrootの小さいdomainが16個以下の場合にworker専用wrapperをfork時に作り、更新ではそのwrapperだけをcloneする。wrapperからrootへの参照は作成時だけ保持し、mergeはrootへ戻す。domainのkeyと値・等値性は共有し、owner/wrapper metadataをsemantic hashへ入れない。workerで新規に作ったdomainは既存のremapへ戻す。Wasmにはworker wrapperを導入しない。

Cのrunは残し、Dを同じ条件で別runへ比較する。小domainが多い入力はwrapperを作らず通常のArcへfallbackし、全domain数に比例したfork費用を追加しない。wrapperは64 key以下に限定する。原子カウンタ競合の影響は仮説であり、採否はprofileと通常測定で決める。

## membershipの呼出位置の訂正

通常のSearch::acceptは重複排除後、新規状態にinvariantを検査する。constraintで除外する境界候補等は別。Cの「全生成候補でmembership」という説明は誤りで、正しくはmembershipを評価する各呼出で列を作っていた。速度の判断は実測と状態・生成・workの一致に基づき、生成数をmembership評価回数として扱わない。
