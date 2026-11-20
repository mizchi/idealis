# 0051: idealis-oracleと実装間比較

## APIと採用範囲

crates/oracleにidealis-oracleを作る。Rustの#[test]とResultで使い、Criterionのgroup/入力登録に寄せたOracleGroupを提供する。case、reference、test_functionを登録し、check/benchで共有する。CheckReport::assert_matchedは通常のRustテストを失敗させる。独自のtest runnerやproc macroは作らない。

入力と観測はgenericなRust値。reportはSerialize/Deserializeでき、入力とreference/actualを保持する。比較はPartialEqによる値・referenceの許可値集合。invalid/unknown/errorとmismatchを区別し、空suiteや重複IDを拒否する。多数決をoracleへしない。射影・正規化・独立した実装の選択はadapterの責任。idealis-testingのPBT/縮小は移植・再実装しない。

replayは保存した入力とreference観測を使い、現在のreference関数を再実行しない。reportの構造・ID・保存statusの整合性を検査する。source/binaryのhashとmodel版の固定はhostのmanifestが担当する。

## ベンチの契約

referenceは正しさの基準で、速度比のbaselineは別に指定する。benchは登録したtargetだけを測る。preflightで一致しなければ測らない。warmupと全timed invocationも保存referenceへ照合し、途中の不一致・不明・エラーでratioを出さない。入力準備・出力照合と破棄は時計の外。各invocationの関数呼び出しと出力構築を測り、sampleは固定iterationsの合計時間を保存する。

nativeのbenchはInstant、bench_with_clockはhostが与えるmonotonicなns clockを使う。Wasmでは暗黙の時計・I/Oを使わない。sampleごとにtargetの開始位置を回し、次回は順序を逆にする。raw sampleと順序を残す。ゼロ・逆行clockは未測定として扱う。統計はmean/median/min/maxとbaseline比に限定し、有意差や信頼区間を作らない。warmup/sample/iterationsと最大呼び出し数を検査し、hostの関数をtimeoutで停止する保証はしない。

## 実証と比較条件

既存の有限TLA+モデルからlinear-ring（1,000 states）とfunction-cube（8 bits）の正常版を固定する。TLC revision 5a47802を明示したoracleにし、idealis-native、tla-ts、TLCを比較する。人工故障として結果のcountを壊したadapter、unknown/error、測定中だけ結果を変えるadapterを使う。正常対照と故障検出を分け、実システムの未知バグ発見と数えない。

実プロセス比較はcold processのsource-to-result end-to-endとする。Node/JVM/native起動、各adapterの輸送・出力解析を含め、TLCのworkerは1。7 sample、1 warmup、1 iteration。source・binary・versions、入力、raw logとreportを保存する。checkerのkernel速度比へ読み替えない。prepared Rustでは同じモデルの通常checkとcoverageを20 sample、3 warmup、10 iterationsで別groupへ置き、同じresultへの一致を確認する。

CLIの外部processやファイルI/Oはeval adapterへ置き、libraryはserde/serde_jsonとstdの範囲に保つ。計測中は他のbuild/test/TLC/profileを並走させない。Wasmで差分機能とhost clock経路を実行し、nativeとの同値を確認する。Wasmのwall timeの測定は範囲外。

## 完了条件

TDDで比較・エラー/unknown・空集合・不正設定・保存report改ざん・測定中のdrift・clock・予算・実行順の挙動を検査する。crate fmt/clippy/test、native/Wasm、単独consumer、既存frameworkのcheck/browser、runtime-build/test、evaluate/replayを通す。SUPPORT.mdとJev、CI/packageへ追加する。raw runを編集せず、比較表と採用範囲をiterationへ保存する。registry publishやPR/mergeは今回行わない。

## 01試行後の対照修正

0051-oracle-01ではcold/preparedの正常照合は完了したが、極小の人工drift callbackがnative clockで0 nsとなり、drift検出より先にunknownで停止した。0 nsをunknownにする契約は維持する。人工driftの対照だけを10 ns刻みの論理clockに変更し、不一致検出をclockの分解能から独立させる。実性能のcold/preparedのモデル・回数・計測境界は変えない。01のrawは保存し、次の新しいrunを完全実行として記録する。Java版はstderrから保存し、tla-tsのpackage.jsonもfingerprintへ加える。
