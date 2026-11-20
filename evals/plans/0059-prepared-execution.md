# 0059: prepared定数、型別実行、並列受理を順に評価する

基準は5c272b9a。定数・不変値のprepared化、型別の実行計画、並列受理の順に進め、各段階を分けて評価する。公開API/IR/JSON、完全探索の範囲、canonical identity、logical work、エラーの評価順、最短反例と予算の扱いを維持する。TLC clone d8fb231のLazyValueを読み、状態依存式を値としてキャッシュする危険と遅延評価を確認した。

## 段階1: Data定数の遅延prepared化

immutableなprogramからData式とslotの対応を作り、ArenaはslotごとのScalarを遅延初期化する。値全体を毎回hashせず、既に評価したslotはIDを返す。program/worker間で不変の表を参照し、可変のcacheと分離する。式の登録で値を評価しない。未評価分岐のエラーを前倒しせず、失敗を成功としてcacheしない。動的・未登録の式は従来のdata処理へ戻す。workerのlocal Refはmergeでremapし、checkの間でcacheを共有しない。

## 段階2: 型別の実行計画

まず算術・比較・選択等の葉operandを持つ頻出式を、checkedな型別命令へ準備する。初回から全IRのVMへ移さず、対象外の式は既存interpreterへ戻す。Read/Bound/Old/prime、左operandの型検査から右operandの評価へ進む順序、overflowと型エラーを維持する。Data operandは段階1のslotを使う。不変の計画をworkerへ参照させ、hot pathで共有Arcをcloneしない。対象範囲とfallbackを明記する。

## 段階3: 並列の訪問済み照合

各worker deltaのmerge順を維持し、merge後の候補に対して訪問済みstateのlookupを並列化する。既知のstateを逐次acceptへ渡し、未知の候補はaccept時にも再照合する。候補間の重複、constraints、error順、state/depth budget、invariant検査の順序を維持する。hashだけでは同一視しない。小さい候補batchは逐次へ戻す。完全な並列acceptやsharded state storeを実装したとは主張しない。

## TDDと採否

各段階でテストを先に追加し、Red/Greenを確認する。lazyな未評価・失敗、dynamic式、worker Ref競合、型エラー/overflow/prime、既知/未知/同batchの重複とhash衝突、budgetと反例を検査する。基準native/Wasmへの172入力・全応答照合と、並列の正常・違反・境界budgetを確認する。意味が変わる最適化を採用しない。

各段階のsource/native/Wasm/Criterion executableを凍結する。既存14本、同じbench source、20 samples、warmup 0.5秒、measurement 1秒でbefore/after/after/before。10%以上の後退は明記し、効果がなければ採用しない。失敗候補も保存する。

最終版は基準と同じfunction N20（1048576状態/20971521生成）、channel N7（2097152/22020097）をtla-ts 1、TLC 1/8、before/after native 1/8、before/after Wasm 1へ通す。1 warmup後3回、交互・逆順に測る。小さい4モデルの正常/違反版を全routeへ通し、complete・計数と通常resultの全体一致を検算する。起動込みwallとprepared Rustを分け、Node Wasmをブラウザ速度へ一般化しない。

通常測定中にbuild/test/profileを並行しない。全sampleと範囲を残す。ホスト条件の変動が大きい区間は改善の根拠にしない。最終profileは別に採取し、CPU sampleからwall速度比を推定しない。

## 終了条件

check/browser、portable/safety/criterion、fmt/clippy、runtime、evaluate/replay、package、TLC既存155件・32件・保存再生、SUPPORT/Jevとstaged検査を行う。既存確認seedは回帰で、独立した新しい確認実験とはしない。raw runを上書きせず、各段階の範囲・採否と残る課題をiterationへ記録する。ローカルcommitまでとし、push/PR/mergeは行わない。
