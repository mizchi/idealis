# 0064: 準備済みSafetyProgramとbatch

基準はbef6255f。#63の次のP1として、同じ仕様を繰り返すPBT・差分検査の費用を測る。以降も優先順に候補を評価し、効果がばらつきに埋もれるか、複雑さ・後退が効果を上回る段階で止める。push・remote mergeは含めない。

## 契約と実装順

所有するRustのPreparedSafetyProgramとSafetyProgram::prepareを追加する。不変な検証metadata・Data式のaddress索引を再利用する。定数値の重複clone、unsafe、自己参照を避け、programを固定したBoxで所有する。Data値自体は評価時の引数を使い、preparedな索引に値の参照を保存しない。v2のmetadataは参照する定義のarityと解析summaryを所有する。

native/Wasmのprepare-safety / check-prepared-safety / check-prepared-safety-batch / release-safetyを定義する。TS helperとinterfaceはsrc/contracts.tsへ契約を置く。既存のone-shot・既定v1・明示v2・型付きエラーは維持する。新しいprepareは静的な検証だけを行い、ASSUME・定数invariant・未選択Dataを先に評価しない。

各checkのArena・constant Scalar cache・visited・graph・symmetry代表・work・coverageを新規にする。batchは逐次のjob列として扱い、各jobのbounds/optionsと結果またはerrorを保持する。全体の成功をモデルの合格と呼ばない。不正なhandleやJSON構造は外側のprotocol error、妥当な形の個々のjobの実行エラーはそのjobのerrorとする。batchは1〜256件とし、空・過大を拒否する。

## Redと対照

最初にRustとportable protocolのテストを置き、未実装によるRedを保存する。正常・故障・empty・assumption-violation・bounded/work/depth/state・実行error、異なる予算の反復と失敗後の復帰、handleのrelease/二重release/別Engine/有限modelとの種別違いを検査する。v1/v2、prime/ENABLED・caller捕捉、未選択の不正Data、深い拒否、coverage、SYMMETRY/PORとnative 1/8もone-shotの完全応答へ照合する。保持したprogramへ外側から変更を反映させない。

TLCの固定oracleと旧版156モデル・1,560要求、既存のanalysis/frame parityを再実行する。既知のTLC乖離1入力、v2の未対応、unknown/boundedを一致・合格へ読み替えない。実ブラウザWasm、TS/Rustの単独consumerも通す。

## 計測と採否

beforeのsource/native/Wasmを固定する。Quint6生成物、TCommit RM=8、既存4モデルの小さいfixture、sharing chainの10/60を使う。探索予算は既存のものに合わせ、状態・生成数・work・verdictが一致するものだけを速度比較する。準備だけ、IRを毎回渡すone-shot、handle反復、batchを分け、反復数1/10/100で各1 warmup + 3 samples、経路順を回転・反転する。起動・compileは別区間とする。max RSSも保存する。

反復nativeで約20%以上の有用な短縮、または受理範囲を保った明確な境界費用削減を採用の根拠とする。既存14本のCriterionも20 samples、warmup 0.5秒、measurement 1秒のABBAで検査し、10%以上の後退は固定binaryで再確認する。後退が再現する汎用変更を採用しない。build/test/profileと通常計測を重ねず、runは追記のみ。

終了前にcheck/browser、compiler/safety/portable、analysis、fmt/clippy、runtime build/test、evaluate/replay-evaluation、usable/cross-language、packageを通す。共有テスト変更時はmutation。SUPPORT/Jevのlive/offline/stagedを同期する。キーなしのskipを検査済みと数えない。採否、残る費用と次の優先度をiteration/README/#63へ戻す。

## 反復数と実行費用の区別（計測前）

TCommitは既存runで1検査が約0.1秒だったため反復1/10とし、他の小さい入力は1/10/100を測る。inputはQuint6・TCommit・small4モデル・chain10/60の13件。v1のchain60拒否は保存して計測対象から外す。prepare/check/releaseの区間を分け、合計は各区間の和とする。結果の照合は時間区間の外へ置く。

batch/handleのWasmと常駐nativeは同じプロセス/instanceを使い、各sampleでhandleを作って破棄する。coldとRSSはnativeの同じrequest列をmacOS time -lで別processに渡して測る。生の応答列はgzipで全件保存し、rawの情報を捨てずに巨大なpretty JSONの重複を避ける。
