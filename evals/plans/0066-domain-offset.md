# 0066: Ref domainの添字候補

[#68](https://github.com/mizchi/idealis/issues/68)。基準は54f39529。0063のTCommit profileにはpositionの費用が残った。既存Int fast pathと線形fallbackを保ち、Ref keyと先頭Refの差を添字候補にする。候補位置の完全key一致を確認した場合だけ使い、疎なdomain・異なるintern順・欠落keyを推測だけで返さない。新しいhash/cacheやinline指定は加えない。

TLCのFcnRcdValue.apply/selectのソースを読み、domain外のselectと関数適用のエラーを区別する。先に疎なdomain、意味順とRef順が逆の場合、欠落key、更新と元の不変性、型違いをArena経由で検査する。最適化前にも同じ挙動testを通す。状態のkey、Ref remap、論理work、公開IR/APIは変えない。

beforeのnative/Wasm・Criterionとsourceを固定する。初期の判定は原本TCommit RM=8、Quint6とsharing chainの既存corpus、v1/v2、1 warmup + 3 samples、順序回転・反転とcold/RSSを使う。TCommitは6,816状態・53,513生成と完全応答の一致を要求する。nativeで約5%以上の短縮を採用の根拠とする。小さな差は同じ固定binaryで追加確認し、雑音内ならこの候補を撤去する。

有用なら通常14本のCriterionをABBA、20 samples、warmup 0.5秒、measurement 1秒で比較する。10%以上の後退は固定binaryで確認し、再現する汎用変更を採用しない。build/test/profileを時間測定と重ねない。誤差を追って別の索引や圧縮方式を足さず、一つの仮説として判断する。

rawは追記のみ。before/afterのsource、binary/input/driver hash、全sample、失敗を保存する。採用なら旧版156モデル・1,560要求と必要な21検査・SUPPORT/Jevまで実行する。撤去なら元のproduction sourceへ戻ったことを確認し、残す挙動testとcheck/browser・Rust/portableの検査を実行する。実験変更のevaluate/replay、runtime build/testも確認する。

この小さい候補が有用でなければ、今回は追加のmicro tuningを停止する。小整数圧縮やworker制御の拡張は、既存0/1圧縮で足りない実モデルのRSS制約、または新しいprofileで支配費用が示せるまで保留し、未検証の高速化と数えない。採否と再開条件をiterationと#68/#63へ記録する。

初回corpusのTCommit native checkはv1で約4.3%、v2で約2.8%短縮し、5%に届かなかった。固定したbefore/after binaryを使い、TCommitのcompiled IR checkだけをnative v1/v2・1 warmup + 12 measured samplesで追加確認する。経路順を回転・反転し、各回の完全応答と6,816状態・53,513生成を照合する。追加後も5%未満なら撤去し、14本へ広げない。追加の値を初回へ混ぜない。
