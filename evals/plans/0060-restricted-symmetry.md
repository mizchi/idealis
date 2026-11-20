# 0060: node関数に限定したSYMMETRY

基準は4e838631。Rust nativeの通常wallとprepared探索を主指標にする。Wasmは可搬性・挙動一致を検査し、tla-tsより速いことを採用条件にしない。条件とcorpusを結果を見る前に固定する。raw runは追記のみで、既存run・ledgerを編集しない。

## 対応範囲と前提

Rust frontendのSYMMETRYは、TLCをEXTENDSするflat moduleのゼロ引数定義 `Symm == Permutations(Nodes)` と、CONSTANT Nodesへ直接与える2〜64個の異なるuntyped model valueの有限集合に限定する。複数群、任意の置換集合、typed model value、module bundle/INSTANCEは拒否する。通常の式としてPermutationsを実装する変更ではない。IR/TS契約へoptionalなsymmetry.modelValuesを追加し、設定なしのwire応答を維持する。

状態はInt/Bool変数と、全Nodesをdomainに持つ有限関数に限定する。各関数のpayloadは同じ種類のIntまたはBool。複数関数は同じnodeの値を同じ行へまとめる。行を辞書順へ並べたkeyで同一視し、全N!置換を列挙しない。payloadにnode参照、nested function/record/sequence、任意のmodel valueを持つ状態は拒否する。

TLCのSYMMETRYと同様、宣言した群がInit/Next/invariantを保つことは利用者の前提で、自動的なequivariance証明を提供しない。IRでは、個別の群要素を直接参照する式や、CHOOSE、fold、sort/select-seq等の列挙順に依存する式を保守的に拒否する。完全な集合としてのNodesの使用だけを受理する。このgateを仕様の対称性の証明とは扱わない。

初期版はsafetyのinvariant/deadlockのみ。temporal/fairness、state/action constraint、initial/action property、coverageとの併用を拒否する。nativeの複数worker要求はsymmetry時に逐次へfallbackする。Wasmでも同じkeyを使う。VIEWは実装しない。

## 探索・反例・予算

同一視用のkeyと、探索・trace用の具体状態を分ける。具体状態をcanonicalな別ラベルへ置き換えず、元の親から生成した値を保存する。hash衝突はkey全体を比較する。同じ行のmultisetだけを比較し、関数を独立にsortして相関を失わない。

maxStates・depth・generatedは縮約後の探索に対する計数。workには群の準備とkey構築を計上し、予算切れをcompleteへ変えない。無効なshapeや設定も拒否する。counterexampleは縮約なしのNextへ再生して実際の遷移とinvariant違反を検証する。livenessと最短lassoは対象外。

## 参照・TDD

TLC clone d8fb231のTool.getSymmetryPerms、TLC.Permutations、TLCStateMut.fingerPrintを読んだ。TLCがfingerprint用の代表値と具体状態を分ける点、全置換の費用を確認した。TLCのfingerprint byte列へ合わせる変更ではない。

Redを先に保存する。groupの不正・重複・範囲外、typed/個別要素・順序依存式・temporal等の拒否、同orbit/異orbit、複数関数の相関、hash衝突、全group要素のdomain検査、混在payload、具体trace、empty/deadlock、work/state/depth予算を検査する。modelvalueの文字列化で同一視しない。

TLCに対する固定corpusはN=2/3/4、local counter上限K=1/2、安全版とforall nodeのf[node]<Kという故障版を組にする。複数のBool関数の相関、BoolとInt、global scalar、初期違反・empty・deadlockを追加する。native/Wasmと1/8要求を照合する。fault traceは縮約なしのIRから毎遷移を検証し、期待値へSUTを転用しない。

## 比較と採否

主benchmarkはN=7、各nodeのcounterを0..5で循環させる。縮約なしは279936状態・1959553生成、縮約ありは792状態・5545生成を期待する。通常のfresh processでRust native plain/symmetryの1/8要求、TLC plain/symmetryの1/8 workerを1 warmup後3回、交互・逆順に測る。入力と全応答・heap/host・commandを保存する。cold startとpreparedを分ける。縮約の有無で計数が異なることを正しく扱い、完全探索でないsampleを速度比に使わない。

新しいCriterion targetにN5/K3、N7/K5、N5の2つのBool関数を置く。同じ候補binaryのplain/symmetryを20 samples、warmup 0.5秒、measurement 1秒で測る。既存14本は同じsourceでbefore/after/after/beforeを測り、10%以上の後退を明記する。通常測定中にbuild/test/profileを並行しない。nativeで有用な短縮がなく、誤った反例や見逃しがあれば採用しない。

基準native/Wasmへの既存172入力・全応答照合とTLC既存155件・32件・保存再生も回帰として行う。check/browser、portable/safety、fmt/clippy、runtime、evaluate/replay、package、SUPPORT/Jevとstaged検査を通す。既存確認seedを新しい独立した確認実験と数えない。READMEと各SUPPORTに条件・前提・対象外・nativeの採否を残す。ローカルcommitまでとし、push/PR/mergeは行わない。

## 対照測定後の追加診断

最初の14本のrunは変更前binaryのring/channelが反復間で数倍変動し、counterの8-worker要求は平均で約13%後退した。原因は特定できておらず、初回を破棄せず、同じ入力・凍結binary・順序とsample条件で全14本を再実行する。追加診断は元のrunと併記し、独立した確認seedや改善の再現と数えない。通常経路の10%以上の後退が再現する場合は、その箇所を修正して再検査する。初回の異常な対照から高速化を主張しない。
