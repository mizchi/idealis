# 0062: 共有IRの統合と最新版の比較

umbrella [#63](https://github.com/mizchi/idealis/issues/63) のP0を実行する。比較・profileは [#64](https://github.com/mizchi/idealis/issues/64)、共有IRは [#60](https://github.com/mizchi/idealis/issues/60) で追跡する。基準はローカル `3f249cc7`。共有IRの実装はdraft PR #62の `a89e1d9cbf0260f2be24d898cab60daaf20e3f8e` をローカルへ統合し、重複実装を避ける。remoteのmerge・pushは行わない。

## 仮説と契約

共有IR v2はoperatorのinline膨張とcaller/calleeのbound slot混線を解消する。主指標は受理範囲、IR bytes、compile時間、RSS。実行時の指数仕事の除去とは数えない。v1を既定のまま保ち、既存の正常・故障・拒否・budget結果とSYMMETRY/代表展開/PORを回帰検査する。v2のmodule連結非対応、action外殻の非共有、独立した深さ上限は明記する。

call-by-name、caller lexical frame、未使用引数、prime/ENABLED、capture、work課金はTLCの実装・保存oracleと照合する。v2のcall/frame課金によるwork差を隠さない。縮約未対応のv2は安全な拒否または既存条件に従う通常探索とし、誤って縮約しない。深いIRのdrop変更もv1の速度・破棄費用と小さいstackで検査する。

## 順序とRed

1. このplanを先にcommitする。0061-finalのsource/binary/digestをbaselineとして保持する。
2. #62のテスト・実装をレビューし、現行での共有IR拒否をRedとして保存する。既存のv1応答、SYMMETRY/PORを対照にする。
3. 固定headをローカル統合する。衝突を解消し、v2の深いchain、binder/capture、lazy引数、prime/ENABLED、不正IR、work境界、TS/Rust構造、native/Wasm応答を検査する。保存したQuint生成物は改変しない。
4. #62のanalysis parityが通常のtask/CIから漏れている点を修正し、実行可能な前提buildをtaskに記す。既存のv1 fixture・回帰を保持する。
5. 正しさを確認した候補をfreezeし、build/testと別に通常時間・RSS・profileを採取する。全失敗を新しいrunへ保存する。

## 比較条件

同じTLA/config、maxDepth=10000000、maxStates=10000000、maxWork=4000000000で完全探索を比較する。既存function N20とchannel N7はtla-ts 1、Java TLC 1/8、baseline/candidate native 1/8、baseline/candidate Node Wasm 1の9経路を使う。各1 warmup + 3 samples、経路順を回転・反転する。小さい4モデルの正常/故障も全経路で検査する。状態数・生成数・verdictが一致しない時間を速度比較に使わない。

公開実モデルの対照には既存Quintの6生成物と正常/故障設定を使う。record・集合・循環は既存の縮約なし小モデルとSYMMETRYの対照を使う。小さい入力のcold時間を探索kernelの速度とは呼ばない。IR v2のcompile bytes/定義数/call数、compile/checkの費用はv1と別に報告し、v1拒否を無理にinlineしない。

prepared kernelは既存14本のCriterionを同じbinary設定でbefore/after/after/beforeで比較する。20 samples、warmup 0.5秒、measurement 1秒。共有IRと削減の追加対照は既存cohortを再実行する。profileはnamed release binary、samplyの既存driverで通常時間から分ける。compile/check、式評価、値intern/update、visited、merge/accept/invariant、dropの観測可能な内訳を記録し、sample比をwall比と混同しない。

OS・CPU・Node/Rust/Java・jar・tla-ts version、source/binary/inputのhash、workers、順序、全sample、RSS、raw profileを保存する。hostは専用機ではない。変動・10%以上の後退・未対応・実行失敗を残し、結果後に条件を変えるときは追記してから新規runにする。既存0061の速度を新しいrunへ混ぜない。

## 採否と完了条件

共有IRの採用はTLCの意味一致、v1の互換性、inline膨張・slot混線の解消を前提にする。v1の性能後退があれば原因と大きさを明記し、有用性と分けて判断する。最新比較とprofileから#63のP1の順を更新する。測っていない私有モデルや全TLA+への保証を主張しない。

終了前にcheck/browser、portable-build/test、compiler-test、safety-test、quint-test、analysis parity、runtime-build/test、fmt/clippy、evaluate/replay-evaluation、packageを実行する。共有TSテスト変更時はmutation。SUPPORT/Jevを同期しoffline/staged検査を通す。キーなしのskipは検査済みと区別する。結果と制限をiteration、README、#64と#63へ反映する。#60/#62のremote mergeやcloseは行わない。

## 公開仕様の追加入力（実行前）

Quintの小さい生成fixtureだけで判断しないため、TLCの `d8fb231de4e4b5d02c1b2ed1ac759c086f694021` の `general/performance/PaxosCommit/TCommit.tla` を原本どおり保存する。RMを8個のmodel valueに固定し、TCInit/TCNext、TCTypeOK/TCConsistent、CHECK_DEADLOCK FALSE、縮約なしで比較する。THEOREM自体の証明を実行したとは扱わない。TLCの結果を先に保存し、TS/native/Wasmの受理・拒否と計数を照合する。受理する経路のみ1 warmup + 3 samplesでcompile/prepared/常駐requestの費用を測る。拒否は対応範囲の限界として残す。

Quint6正常fixtureも同じ反復数でbaseline v1/candidate v1/v2を比較する。公開仕様を受理できた場合は、通常計測の後にnativeの名前付きprofileを別採取する。IR v2の構造対照には既存sharing chainの10/20/30/60を使い、compile時間とIR bytesを分ける。値cacheを持たないので、両枝を評価する指数chainはworkで止まることを要求する。

## 初回の後退と次の候補（結果後の変更）

統合直後のABBAはcounter 1 workerが1.62倍、functionが1.39倍、channelが1.29倍の時間になった。採用を保留する。0062-profile-integratedではeval自体のself sampleがcounterで約38%、ringで約36%だった。これは因果の確定ではない。

候補Aは小さいleaf dispatchを復元し、v2の深さ検査をnon-leafの境界へ移す。leafでも深さ128の拒否を保つ。contextを値で返すdescendは参照更新へ変更し、同じ検査・課金・phase・lazy評価を保つ。変更前のsnapshot/profileを残し、既存v1/v2の境界と完全応答を先に検査する。その後同じ14本のABBAで比較する。悪化すれば撤去する。visitorのdispatchやIRのlayout変更は別の候補として、Aの結果前に混ぜない。

Aの小さいdispatchは後退を減らしたが、counter 1は+26%、function 1は+12%、channel 1は+14%の時間差が残る。scalar/callを大きい集合演算用frameへ通した途中版はdebugの深い評価でstack overflowし、別の小さいnon-leaf関数で分岐させて修正した。境界テストを弱めず、失敗logを保持する。

候補Bは128までしか使わない非公開evaluation_depthをusizeからu8へ変え、contextの受け渡しを軽くする。他のdispatchやIR layoutは変えない。A/Bで同じsearch4モデル・1 worker、20 samples、0.5秒warmup、1秒measurementのABBAを先に比較する。有用な短縮があれば最終14本と外部比較へ進む。後退が残る場合、候補Cとしてv1のbranch visitorを静的dispatchへ戻す境界を別に試す。

Bの実行前に、Aのleaf検査がv1の長いENABLED branchにも128上限を適用する回帰を追加テストで再現した。Bではv2だけに上限を適用するcold分岐を復元する。長いbranchの深さ加算はu8へ切り詰めず飽和させ、v2の128以上の拒否を維持する。Bの費用差をu8の変更だけの効果とは断定しない。

候補Cではbranch visitorをgenericな静的dispatchへ戻し、v2のframe continuationだけを非generic関数の境界で扱う。v2のbranch・next候補・caller環境の復元は変えない。通常のbranchがframeを使うためだけにdyn visitorを経由する費用を除く仮説で、B/Cのsearch4モデル・1 workerを同じABBAで比較する。深いframe・work・branch計数・caller復元を既存テストへ照合し、型の変更だけで速度向上とは数えない。

Cはchannelが約24%短縮した一方、counterとfunctionが約12%後退し、汎用の変更として撤去する。source/binaryと全sampleは0062-c / 0062-kernel-cに保持する。候補DはBへ戻し、小さいleaf用evalにinline(always)を指定する。v2の関数参照にも使うevalの存在が、通常呼出しのinline判断へ影響する可能性を試す。新しいVM、workの変更、モデル名による経路切替はしない。B/Dの4モデル・1 workerを同じABBAで比較し、境界テストと最終14本でも確認する。

## 外部比較の4モデル化（実行前）

初期計画のfunction/channelに加え、既存READMEのcounter-product N48とlinear-ring N1000000も同じ9経路・1 warmup + 3 samplesで再測定する。古い0022の4モデル表を、新しい同一runの4モデル表へ置き換えるためで、結果を見た入力選別はしない。明示したIDEALIS_COMPACT_ALL_MODELS=1で既存driverの対象を広げ、従来の既定2モデルは維持する。通常探索はIR v1で比較し、IR v2の受理・サイズ・費用は別表にする。

実モデルcorpusのnativeには、常駐JSONLのcompile/check/source各要求に加えて、macOS time -lによるsourceから結果までの新規Rust process・最大RSSを測る。各1 warmup + 3 samples。Node親のJSON encodeとtime wrapperを含み、Node起動やTS frontendの費用は含まない。これは9経路比較のNode親process込みの値とは別の区間である。

最終14本で小さいchannel N5の8-worker要求に+11.5%が残ったため、同じ固定binary・同じ条件・同じfilterだけをABBAで1回追加確認する。large N7の同条件比較とは別に報告し、良い反復だけを採用しない。新しい独立モデルでの確認とは数えない。
