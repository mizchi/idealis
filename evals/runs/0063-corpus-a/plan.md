# 0063: 通常評価と定数slot・関数lookupの費用

[#65](https://github.com/mizchi/idealis/issues/65)で#63の次のP1を扱う。基準は162fd73d。公開契約・IR v1/v2・論理work・拒否条件を保ち、0062で残った通常評価の費用を小さい変更で測る。pushとremote mergeは含めない。

## 仮説と順序

最初はPreparedProgramのData式address検索を試す。現行は内部usize keyをSipHashへ渡す。addressだけに使う非公開hashへ置換し、HashMapの完全key比較は維持する。ユーザー値・visited・membershipのhashを混ぜて変更しない。cacheの同値判定・lazy評価・失敗の再試行・Arena間のScalar所属・worker remapを保つ。

次にArena::cellとFunction::positionの関数境界を調べる。小さい値アクセスの呼出し費用がTCommitのprofileへ現れていたため、inlineを候補とする。domainの順序やkeyの意味、範囲外の拒否は変えない。共通評価の深さ判定は実際の残るprofileに基づいて検討し、v1へv2の制限を広げない。候補は別々に固定し、改善がなければ戻す。

## 固定条件と対照

- 変更前のsource/binaryは.artifacts/0062-releaseから複製し、sha256を検査する。benchを拡張する場合は、実装を変える前に同じbenchでbaselineをbuildし直す。両者を混同しない。
- 既存14本のCriterionを20 samples、warmup 0.5秒、measurement 1秒、before/after/after/beforeで測る。初期候補はsearchの4モデル・1 workerを先に測る。有用なら14本へ広げる。build・test・profileを同時実行しない。
- 全4モデルで約5%以上の短縮、または実モデルでの有用な短縮を採用の根拠とする。10%以上の後退は追加の同条件ABBAで確認し、後退が再現する汎用変更を採用しない。閾値未満も数値を保存し、確定した改善と呼ばない。
- TCommit原本RM=8、Quint6生成物とsharing chainは0062と同じ入力・予算。v1/v2のcompile/check/source要求と新規Rust process/RSSを別に、1 warmup + 3 samplesで比較する。TCommitは6,816状態・53,513生成を要求する。sample weightをwall比と同一視しない。
- 旧版156モデル・1,560要求のnative/Wasm完全応答比較、既存v1/v2のwork/depth境界、lazy定数・未選択branch・失敗後の再評価・worker remapを検査する。TLCの保存済み固定oracleとcohortへ照合する。新しい意味論を実装しない。

## 記録と完了条件

runは追記のみ。source/binary/input・driver・実行順・予算・host・全sampleを保存し、失敗と撤去した候補も残す。既知のTLC乖離1入力と未対応は一致へ読み替えない。同じ固定benchの反復を独立モデルでの確認とは数えない。

終了前にcheck/browser、compiler/safety/portable、fmt/clippy、analysis、runtime build/test、evaluate/replay-evaluation、packageを実行する。SUPPORT/Jevの46比較をlive/offline/stagedで同期する。API keyなしのskipは検査済みと区別する。採否と残る費用をiterationとREADME、#65/#63へ戻す。
