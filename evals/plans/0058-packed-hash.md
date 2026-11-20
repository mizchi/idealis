# 0058: packed関数のhashを軽くする

基準はd65c4ec0。0057のprofileで残ったSipHasherの費用を対象に、64要素以下のBool/Int(0/1)関数の内部hashを変更する。式評価・汎用関数のhash・状態探索の構造は変更しない。

## 候補と契約

packed payload、cached domain hash、Bool/Intを区別するsaltをxorし、MurmurHash3のfmix64で混合する。公式smhasher 07bb4de10a63e8cc2e1724865454eba635742383のpublic domain実装を参照する。saltは0x9e3779b97f4a7c15。64bitのwrapping arithmeticを使い、新しい依存と公開APIを追加しない。

hashは内部索引にだけ使い、domain/payloadの完全比較を維持する。TLC clone d8fb231のFcnRcdValue.fingerPrintを読み、関数のdomainと値を扱うことを確認する。TLCのfingerprint形式を実装する変更ではない。公開Value/IR/JSON、型の区別、canonical identity、logical work、反例と予算切れ、完全探索の範囲を維持する。

## TDDと分布の検査

先に公式C++のfmix64から0、1、最上位bit、全bit、0x0123456789abcdef、salt、既存domain衝突のhashに対する7出力を保存する。Rustへgolden vectorのテストを追加してRedを確認し、実装後にGreenへする。

同じdomainの4096 packed関数でhashが重複しないことと、下位8bitの各bucketが空でなく最大64件以内に収まることを検査する。これは固定入力の回帰で、暗号学的性質や全入力の分布保証とはしない。既存のgeneric/domain衝突を維持し、64要素のBool/Intでhashが一致する入力も作り、異なる型の値が統合されないことを検査する。

## 固定する性能比較

before/afterのnative/Wasm、Criterion executable、sourceとhashを凍結する。既存14本を同じbench sourceから20 samples、warmup 0.5秒、measurement 1秒で、before/after/after/beforeの順に測る。10%以上の後退は明記する。

大規模比較はfunction N20（1048576状態/20971521生成）とchannel N7（2097152/22020097）。tla-ts 1、TLC 1/8、before/after native 1/8、before/after Wasm 1を、1 warmup後3回、交互・逆順に測る。小さい4モデルの正常・違反版を全routeへ通す。主値は共通TS frontendと起動を含むwall時間。Node Wasmをブラウザの速度として扱わない。completeと計数を検算し、通常resultはbefore/afterで全体比較する。

通常測定中にbuild/test/profileを並行しない。全sampleと最小・最大を残し、非専用ホストの負荷変動を含むことを明示する。主目標はfunctionのnative/Wasmを改善し、channelと他の対照に残る後退も評価すること。固定倍率は仮定しない。通常測定が改善しなければ候補を採用しない。

## 検証と終了

基準native/Wasmとの172入力・予算境界・全応答照合、TLC既存corpusと保存再生を行う。使用済みの確認seedは回帰として扱う。新しい独立した確認実験とは数えない。

fmt/clippy、check/browser、portable/safety/criterion、runtime、evaluate/replay、package、SUPPORT/Jevとstaged検査を行う。profileは通常測定後に別途採取し、CPU sample構成をwall速度比には使わない。失敗runも残し、source archive・結果・採用判断をiterationへ保存する。ローカルcommitまでとし、push/PR/mergeは行わない。
