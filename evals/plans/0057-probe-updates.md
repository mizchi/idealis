# 0057: 重複した関数更新のコピーを省く

基準は5b72a9f9。0056で残ったchannelの費用を対象に、汎用Scalar列を持つ関数の更新を、コピー前のborrowed viewでintern tableへ検索する。既存の値があればそのIDを返し、新しい値だけを確保する。packed Bool/Intの更新と、汎用値からpackedへ戻る更新は既存の経路を使う。式VMの変更は今回へ混ぜない。

## 契約とTDD

ローカルTLC clone d8fb231のFcnRcdValue.takeExceptを読み、元の値を変更しないこと、domain外と入れ子の更新を確認する。hashだけで一致とはしない。domainと全payloadを比較し、workerのbase/localの衝突を区別する。公開API/IR/JSON、canonical identity、work、予算切れとエラー、完全探索の範囲を維持する。新しい依存は追加しない。

borrowed viewのhashと完全比較を、独立に構築した更新後の値へ照合するテストを先にRedへする。packedへの復帰はviewを使わず既存の正規化を維持する。既存のhash衝突fixtureへ更新経路を追加し、衝突する別値を返さないことも検査する。基準native/Wasmとの172入力・予算境界・全応答照合とTLC差分を行う。

## 固定する測定

beforeのnative/Wasm、Criterion executable、sourceとhashを凍結する。0056の既存14本と同じbench source、20 samples、warmup 0.5秒、measurement 1秒、before/after/after/beforeでCriterionを測る。10%以上の後退は明記する。

大規模比較は0056と同じfunction N20（1048576状態/20971521生成）とchannel N7（2097152/22020097）。tla-ts 1、TLC 1/8、before/after native 1/8、before/after Wasm 1を1 warmup後3回、交互・逆順に実行する。各回のcomplete・状態数・生成数を検算し、通常resultはbefore/afterで全体比較する。小さい4モデルの正常・故障版も全routeへ通す。主値は共通TS frontendと起動を含むwall時間。Node Wasmをブラウザの速度として報告しない。異なるAPI区間をkernel比へ使わない。

通常測定中にbuild/test/profileを並行しない。採用はchannelの実測改善と、対照に残る後退で判断する。固定倍率を仮定せず、効果がなければ採用しない。profileは別に採取し、通常速度比として使わない。小さい既存profile入力をprepared IRで16回、native 1/8へ実行する。

## 終了条件

fmt/clippy、check/browser、portable/safety/criterion、runtime、evaluate/replay、package、SUPPORT/Jevとstaged検査を行う。TLC既存corpusと保存再生を確認する。失敗runも上書きせず、条件・結果・採用判断をiterationへ記録する。ローカルcommitまでで、push/PR/mergeは行わない。

## 候補Aからの見直し

AのCriterionはchannelの8 workersで約8%改善したが、大規模functionの8 workersでは小幅な後退が見えた。Aのrunは最後まで保存する。packed値はprobeを使わないため、guardの追加費用とcode generationの影響を確認する。

候補BはviewがFunction全体ではなくdomainとScalar sliceを直接借用する。hash/比較時にpayload表現を要素ごとに判定せず、変更位置の前後をsliceで比較する。scalar_updateのpacked guardにinline指定を付ける。runtimeはAの通常測定が完了してから変更する。Bも同じ14本と9経路で別runへ測り、A/Bを混在した表にしない。
