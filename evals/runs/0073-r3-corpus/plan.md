# 0073: 後続で参照しない束縛を持つ展開の再利用

[#76](https://github.com/mizchi/idealis/issues/76)で#63の候補として扱う。基準は d42509d4。action の連続する `bind` のうち、後続の step がどれも参照しない束縛がある場合に、同じ後続を何度も評価する費用を測って減らす。公開契約・IR v1/v2・拒否条件・探索順序・状態数・生成数・反例は変えない。push と remote merge は含めない。

## 観測

Quint の `nondet` は TLA+ では入れ子の `\E` になる。生成物では、ある束縛を次の束縛の domain だけで使い、action の本体では使わない形がよく現れる。

```tla
\E preferOwned \in {TRUE, FALSE}:
  \E preferFailure \in {TRUE, FALSE}:
    \E tag \in Tags:
      \E e \in IF preferOwned THEN Owned(preferFailure) ELSE Inputs(tag):
        Body(e)
```

`Body` が読む束縛は `e` だけなので、`(preferOwned, preferFailure, tag)` の組が違っても、`e` が同じなら後続の評価結果は同じになる。この形では、1 状態の展開で本体を評価する回数が、異なる `e` の個数の 2 倍前後になる。本体が重いモデルでは、時間の大半がこの重複した評価になる。

## 仮説

`visit_branch` の連続する `Step::Bind` の並び（位置 p から q の手前まで）について、位置 q 以降の step が参照する `Expression::Bound` の slot を IR から集める。並びが束縛する slot のうち参照されないものがあれば、位置 q からの後続は次のものだけで決まる。

- 現在の状態
- その時点の `next`
- 並びより前の locals
- 並びのうち参照される slot の値
- frame と ctx

並びの中では `next` と並びより前の locals は変わらない。したがって、参照される slot の値の組を key にして、最初の評価で visitor に渡した後続の列を記録する。同じ key がもう一度現れたら、記録した列を同じ順序で visitor に渡し直す。

- visitor（`accept`）は重複でも従来どおり呼ぶので、生成数・既存状態の判定・constraint・action property・coverage の計数は変わらない。
- 記録は、並びを訪れる 1 回の呼び出しの中だけで使う。状態をまたいで保存しない。
- 参照の集計は、`Step::Guard` / `Assign` / `Choose` / `Bind` の domain、`Call` / `Frame` の arguments・captures と内側の branch を辿る。`Frame` の内側は自分の locals を持つので数え過ぎになるが、安全側に倒れるだけ。
- `Expression::Enabled` は `children()` が中の branch を返さず、外側の locals を読む可能性がある。後続に含まれるときは、並び全体を参照ありとして扱い、従来の経路を使う。
- 並びが branch の末尾で終わる場合（後続が visitor だけの場合）は対象外とする。

work は、実際に実行した処理だけを計上する。再生した後続の評価分は足さない（状態展開 cache の前例と同じ）。そのため、work は減ることがあり、work 予算で止まる位置も変わりうる。完全応答の比較では、work を他の項目と分けて報告する。

## 固定条件と対照

- before は d42509d4 の source / binary。after は同じ commit にこの変更だけを加えたもの。sha256 を記録する。
- 意味の対照：同じ program の、束縛を後続の guard（`x = x`）で参照させた版では、記録の経路を使わない。両者で status・states・generated・depth・反例の trace が一致し、work は記録の経路を使う版のほうが小さいことを、Rust の回帰テストで確かめる。テストを先に置き、変更前に work の比較だけが失敗する（Red）ことを記録する。
- 安全側の対照：後続に `ENABLED` を含む branch、v2 の `Call` / `Frame` の captures で束縛を渡す branch、並びが末尾で終わる branch、violation や deadlock が並びの途中の後続で起きる branch を回帰テストに含める。
- 既存の完全応答比較（`evaluator-corpus`）を before / after で流し、work を除く全項目の一致と、work の増減を分けて保存する。TLC の保存済み oracle と cohort への照合を変えない。
- 既存 14 本の Criterion を 0063 と同じ条件（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）で比較する。束縛の死なない kernel での後退は、10% 以上なら同条件の ABBA で再確認し、再現する後退を残したまま採用しない。
- 上の形を再現する公開可能な Quint 生成物を、固定 Quint（0.33.1）と Apalache（0.62.4）で fixture に追加する。native の 1 / 2 / 4 worker で before / after を各 1 warmup + 3 samples 測り、TLC の状態数・生成数と照合する。
- build・test・benchmark は同時に流さない。

## 記録と完了条件

run は追記のみ。source / binary / input・driver・実行順・予算・host・全 sample を保存し、失敗も残す。work の減少を、状態数や生成数の変化と混同しない。

終了前に `just check`、`just wasm-test`、`just safety-test`、`just portable-build` / `just portable-test`、crate の fmt / clippy、`just evaluate` / `just replay-evaluation` を実行する。SUPPORT.md を更新し、`just support-plan` / `just support-sync` で Jev の比較を確認する。API key がない場合の skip は、検査済みと区別して記録する。採否と残る費用を iteration と README に戻す。

## 評価の前に変えた条件

最初の実装は、再生した後続の work を課金しなかった。rtlc の既存テスト `wide_batches_keep_budget_cuts_and_action_constraint_errors` で、`maxWork` 10000 の打ち切りが起きなくなり、失敗した。0063 と同じく論理 work を公開契約として保つため、評価を流す前に次のように変えた。

- 再生では、最初の評価で記録した後続の work を課金する。記録する値からは、visitor の work と、新しい cell の作成で課金した work を除く。同じ値の cell は 2 回目には作られないので、元の再評価と同じ work になる。
- 単一 worker で、残りの予算が「後続の work + visitor の work」より小さいときは、再生せずに従来どおり評価する。
- 並ぶ `bind` の各位置では、従来の `visit_branch` と同じ順で深さの検査と 1 work の課金を行う。
- 回帰テストの work の比較は、d42509d4 で取った work と打ち切り位置の固定値との照合に置き換えた。

計測の条件も、評価を流す前に #63 の項目 5 に合わせて変えた。公開 Quint モデルは、1 / 2 / 4 / 8 worker で、時間とピーク RSS を測る。

## 最初のレビューの後に変えた条件

最初の実装に対する独立レビューで、変更前後のbinaryで再現する4件の不具合が見つかった。次のように変え、評価はすべてやり直す。

- 計画は PreparedProgram に移し、program が持つ branch だけに作る。対象は Init、Next、共有 action 定義、`Frame` の内側、静的な式の `ENABLED` の branch。Arena で branch の address を key にした cache は、`Frame` が複製した一時的な branch の address が解放後に別の branch に再利用されると、誤った計画を当てて不変条件違反を見逃した。複製された branch は表になく、従来どおり評価する。
- 再生では work を課金しない。arena の型・membership の cache があるため、再評価の work は最初の評価より小さくなりうる。最初の work を課金すると、予算切れの位置が変わり、`Integer division by zero` が `bounded(work)` に置き換わる場合もあった。並列では共有予算の一括請求で、予算内に生成できた後続状態まで失った。work は実行した評価だけを数える（状態展開 cache の前例と同じ）。予算で打ち切られない実行では、判定・状態・生成数・探索順・反例は変わらない。work 予算で打ち切られる実行は、変更前より先まで進むことがある。
- 記録の量に上限を置く。1 つの並びにつき継続 4,096 個と successor 合計 4,096 個、1 つの継続あたり successor 64 個まで記録する。超えた分は記録せず、従来どおり評価する。最初の実装は、再生が起きない展開でも全 successor を保持し、Wasm がメモリ上限で停止した。
- 既存テスト `wide_batches_keep_budget_cuts_and_action_constraint_errors` の打ち切り値は、固定の 4000 / 10000 ではなく、同じ program の総 work の半分と総 work − 1 にする。打ち切りを確かめる意図は変えない。
- レビューの再現例を回帰テストに加える。並列のテストは、frontier が並列化の条件に届く公開 Quint モデルで、TLC の計数と照合する。

## 2回目のレビューの後に変えた条件

2回目のレビューは、前回の4件が閉じたことを差分実行で確かめたうえで、次の2件を示した。

- 再生が work を一切課金しないため、`maxWork` が実行量の上限として働かなかった。読まれない巨大な domain では数分走り、u32 の `generated` が巻き戻った値のまま `complete` を返した。再生では、継続に入るときに 1 work、successor を visitor に渡すたびに 1 work を課金する。これは従来の評価が同じ位置で必ず払う最低限の work なので、work は変更前を超えず、予算で打ち切られる位置は変更前と同じか、それより先になる。
- 解析の読み取り経路（Step の `Call` / `Frame` の arguments・captures、`Choose` と後続の `Bind` の domain）、64 件を超えたときに記録をやめる経路、並びの中の深さの検査を、どのテストも固定していなかった。それぞれを外した故障版で失敗する回帰テストを加える。件数の上限そのものはメモリだけに効き、結果に現れないので、テストでは固定しない。

あわせて、再生が一度も起きないまま 256 個の継続を記録した並びでは、それ以降の記録をやめる。読まれない束縛があっても値の組が重複しない場合の記録の費用を抑えるため。評価はすべてやり直す。
