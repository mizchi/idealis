# 0073: 後続が読まない束縛を持つ展開の再利用

[#76](https://github.com/mizchi/idealis/issues/76)で#63の候補を扱う。[事前plan](../plans/0073-dead-binding-reuse.md)を d42509d4 を基準に記録し、評価の前に変えた条件と、2回の独立レビューの後に変えた条件を plan に追記した。push と remote merge は含めない。

`visit_branch` の連続する `Step::Bind` のうち、後続の step が読まない束縛がある場合だけ、読む束縛の値の組ごとに後続を一度だけ評価する。同じ組では、記録した successor 列を同じ順序で visitor へ渡し直す。公開API/IR、状態・生成数・探索順・反例、拒否条件は変えていない。再生では従来の評価が必ず払う最低限の work だけを課金するので、この経路を通る program では work が減り、変更前を超えない。

## 採用した変更

- 後続が読む `Expression::Bound` の slot を、`Guard` / `Assign` / `Choose` / `Bind` の domain、`Call` / `Frame` の arguments・captures と内側の branch から集める。`Frame` の内側を数え過ぎるのは安全側に倒れるだけ。後続に `ENABLED` を含む branch は対象外にする。`children()` が中の branch を返さず、外側の locals を読む可能性があるため。並びが branch の末尾で終わる場合も対象外。
- 計画（並びの終わりと読む slot）は PreparedProgram が作る。対象は program が持つ branch だけで、Init、Next、共有 action 定義、`Frame` の内側、静的な式の `ENABLED` の branch。複製された branch は表になく、従来どおり評価する。
- 記録は 1 回の展開の中だけで使う。1 つの並びにつき継続 4,096 個と successor 合計 4,096 個、1 つの継続あたり successor 64 個まで記録し、超えた分は従来どおり評価する。再生が一度も起きないまま 256 個の継続を記録した並びでは、それ以降の記録をやめる。
- 再生では、継続に入るときに 1 work、successor を visitor に渡すたびに 1 work を課金する。従来の評価が同じ位置で必ず払う最低限の work なので、work は変更前を超えず、work 予算で打ち切られる位置は変更前と同じか、それより先になる。予算で打ち切られない実行では、結果は work 以外変わらない。

## 最初の実装と独立レビュー

最初の実装は論理 work を保とうとし、再生で最初の評価の work を課金した。計画は Arena が branch の address で cache した。独立レビューが、変更前後の binary で再現する4件を示した。

1. `Frame` が複製した `ENABLED` の branch の address が、解放後に別の branch に再利用され、誤った計画で不変条件違反を見逃した。
2. 型の cache で 2 回目の評価が安くなる式で、最初の work を課金したため、`maxWork` 17 の complete が `bounded(work)` に変わり、`Integer division by zero` も `bounded(work)` に置き換わった。
3. 並列の共有予算で work を一括請求し、予算内に生成できた後続状態まで失った。
4. 再生が起きない展開でも全 successor を保持し、Wasm がメモリ上限で停止した。

1 は計画を PreparedProgram に移し、2 と 3 は最初の評価の work を課金しないことでなくし、4 は記録に上限を置いて直した。2 のシナリオは回帰テストにし、最初の実装の binary では `maxWork` 17 で `bounded` になることを確かめた。1 は address の再利用が割り当ての順序に依存し、作った TLA+ では最初の実装でも再現しなかった。そのため、同じシナリオの回帰テストに加えて、複製した branch に計画が見つからないことを crate 内のテストで確かめる。最初の実装の評価は [0073-r1-*](../runs/0073-r1-audit/) に残し、採否の判断には使わない。

2回目の独立レビューは、前回の4件が閉じたことを384通りの変種と約930仕様の差分実行で確かめたうえで、次の2件を示した。

1. 再生が work を一切課金しなかったため、`maxWork` が実行量の上限として働かなかった。読まれない巨大な domain では数分走り、u32 の `generated` が巻き戻った値のまま `complete` を返した。
2. 読み取り解析の経路（Step の `Call` / `Frame` の arguments・captures、`Choose` と後続の `Bind` の domain）、64 件を超えたときに記録をやめる経路、並びの中の深さの検査を外した故障版が、全テストで生き残った。

1 は最低限の課金で直し、2 はそれぞれを踏む回帰テストを加えた。この時点の評価は [0073-r2-*](../runs/0073-r2-audit/) に残し、採否の判断には使わない。採否は [0073-r3-*](../runs/0073-r3-audit/) で判断する。

r1 と r2 の完全応答 corpus は、どちらも 264 比較で差分 0 だった。この corpus は新しい経路を通らず、r3 と同じ入力・同じ比較なので、raw（各 42 MB）はこの iteration に含めず、集計（[r1](../runs/0073-r1-corpus-summary/result.json)、[r2](../runs/0073-r2-corpus-summary/result.json)）と台帳の行だけを残す。r3 は raw ごと残す。

記録した run では、実行したホストの絶対パスとユーザー名を、記録する時点で `<scratch>`、`<after>`、`<clone>`、`<tools>`、`<mise>`、`<home>`、`<user>` に置き換えた。結果の値、計数、sha256 は変えていない。

## 意味と work の検査

`crates/rtlc/tests/dead_bindings.rs` を実装の前に置き、work の比較で Red を確認した。レビューの後に足したテストは、それぞれが対象にする故障版で失敗することを確かめた。

- 束縛を guard で読ませた双子と、complete / deadlock / violation の各版で、status・reason・状態・生成数・深さ・violation が一致し、work は双子の 3 分の 2 未満になる。complete は仕様から数えた 10 状態・232 生成。
- 公開 Quint モデルとその読ませた双子を 1 / 2 / 4 / 8 worker で検査し、どちらも TLC と同じ 4,785 状態・896,821 生成になり、work は双子の 3 分の 2 未満になる。frontier は並列化の条件に届く。
- `ENABLED` の中だけで読む束縛と、演算子の引数で読む束縛では、仕様どおり 5 状態・7 生成を返す。`ENABLED` を対象外にする判定を外した故障版では、このテストが 1 状態・1 生成で失敗した。
- レビューの再現例（`Frame` の `ENABLED`、型の cache と 0 除算、10 万候補の `Choose`）を検査する。
- 後続が束縛を読む経路ごとに、読み落とすと到達状態が変わる仕様で検査する。対象は Step の `Call` の arguments と captures、それを `Frame` に書き換えた版、`Choose` の domain、後続の `Bind` の domain。故障版 M2〜M6 はどれもこのテストで失敗した。
- 64 件を超える successor を持つ継続（100 候補の `Choose`）が、記録をやめて正しい 20,001 生成を返すことを検査する。途中までの列を記録して再生する故障版は失敗した。
- 読まれない 200 万候補の束縛を `maxWork` 100,000 で検査し、変更前と同じ `bounded(work)`・49,999 生成・100,000 work で止まることを検査する。再生で課金しない故障版は失敗した。
- 再帰する action の中の bind の並びで、評価の深さの上限が変更前と同じ位置（再帰 20 回で complete、21 回で `bounded(evaluation)`）にあることを検査する。並びの中の深さの検査を外した故障版は失敗した。
- 並びが branch の末尾で終わる program（コンパイル結果から末尾の guard を外したもの）で、work と予算での打ち切り位置が変更前と一致することを検査する。この場合だけは再生が従来の評価より高くつくので計画を作らない。この除外を外した故障版は、work が 209 から 407 に増えて失敗した。
- 件数の上限そのもの（4,096 と 64）はメモリだけに効き、結果に現れないので、テストでは固定しない。
- 既存テスト `wide_batches_keep_budget_cuts_and_action_constraint_errors` は、打ち切り値を同じ program の総 work から決めるように変えた。
- rtlc の全 301 テスト（release / debug）、cargo fmt、clippy（`-D warnings`）、`just check`、`just wasm-test`（23）、`just portable-test`（cargo 570、node 3 / 22 / 8）、`just evaluate`、`just replay-evaluation`、`just support-plan` が通った。`just safety-test`（1,324）は、他の build と重なった実行で時間制限つきの 1 件（50,000 項の連言を 5 秒以内に拒否する compile 側の test）が 5,027 ms で落ち、負荷のない単独の再実行で全件通った。Jev の checklist を `just support-sync` で同期し、9 scope すべてで矛盾・未評価 0、`just support-check` が通った。`just check` は pnpm 経由で `pnpm-lock.yaml` を書き換えるため、`support-check` はその lockfile を戻してから実行した。

## 合成 Quint モデルでの比較

公開できるように作った合成モデル [NondetTags](../../fixtures/dead-bindings/NondetTags.qnt) は、Quint の `nondet` を重ねて、ある束縛を次の束縛の domain だけで使う形を再現する。Quint 0.33.1 と Apalache 0.62.4 で生成した TLA+ を、IR v2 の native で検査した。新規 process ごとに `check-safety-parallel` だけを実行し、各 1 warmup + 3 samples、before / after を交互に流した。時間は中央値、RSS は `time -l` の最大常駐量の中央値。Apple M5 Pro、非専用ホスト。

| workers | before s | after s |    比 | RSS MiB before / after | work before | work after |
| ------: | -------: | ------: | ----: | ---------------------: | ----------: | ---------: |
|       1 |    1.923 |   0.920 | 0.478 |              5.7 / 5.8 |  51,227,111 | 21,342,230 |
|       2 |    1.039 |   0.525 | 0.505 |              7.1 / 7.1 |  51,227,198 | 21,342,317 |
|       4 |    0.596 |   0.298 | 0.500 |              8.4 / 8.4 |  51,227,273 | 21,342,392 |
|       8 |    0.402 |   0.208 | 0.518 |            11.8 / 11.4 |  51,227,398 | 21,342,517 |

- 全 sample で、before と after の status・reason・状態・生成数・深さ・violation が一致した。safety は 4,785 状態・896,821 生成で、固定 TLC（Apalache 0.62.4 同梱、1 worker）と一致する。
- broken は 1 / 2 / 4 / 8 worker の全てで、before と after の反例を含む結果が work 以外一致した。TLC とは違反を見つけて止まる位置が違うため、計数を TLC と比べない。
- work 予算で打ち切られる実行は、上の 200 万候補の fixture を IR v1 / v2 で before / after 各 3 回流し、status・reason・状態・生成数・深さ・work がすべて一致した（`bounded(work)`、49,999 生成、100,000 work）。

[時間とRSS](../runs/0073-r3-nondet-native-rss/manifest.json)、[work予算での打ち切り](../runs/0073-r3-work-bounded/)、[TLC](../runs/0073-r1-tlc/manifest.json)を参照。TLC の実行は最初の実装の評価で行ったもので、TLC 側は変わらないため再実行していない。

## 既存 corpus と kernel の回帰

- 完全応答 corpus（Quint6・TCommit・共有chain、8 経路・各 4 iteration）で、before / after の 264 比較が work を含めて一致した。264 は 11 入力 × IR v1 / v2 × 4 iteration × after の 3 経路（native、Wasm、cold native）を before native と比べた数。この 11 入力には、後続が読まない束縛を持つ `bind` の並びが 1 件もない。この一致は、新しい経路を通らないプログラムが変わらないことだけを示す。走査の記録には、v1 で unsupported の 3 入力について program がないことによる `KeyError` が残る。
- 14 本の Criterion を 20 samples・warmup 0.5 秒・measurement 1 秒の ABBA で比較した。変化は -1.6% から +1.0% で、10% 以上の後退はなかった。

[kernel](../runs/0073-r3-kernel-summary/result.json)、[corpus](../runs/0073-r3-corpus-summary/result.json)、[driverとlog](../runs/0073-r3-audit/)を参照。corpus driver の環境記録には、0063 と異なり Apalache 0.62.4 同梱の jar と archive を渡した。driver はこの 2 つの sha256 を記録するだけで、TLC を実行しない。

## 残る費用

対象は、後続が読まない束縛を持つ action だけ。束縛がすべて読まれる action の評価費用は変わらない。読まれない束縛があっても値の組が重複しない並びでは記録の費用だけが残り、2回目のレビューの計測では 0.068 秒が 0.098 秒になる例があった。再生がないまま 256 個の継続を記録した並びで記録をやめる規則は、この費用を抑えるために加えたもので、その効果は測っていない。後続がごく軽い並びでは、再生が評価し直すより遅いこともある。3回目のレビューの計測では、`\E a \in 1..3000000 : x' = 1` で 0.147 秒が 0.202 秒になった（work は同じ）。手元のプロジェクトの Quint モデルの profile では、時間の約 4 割がメモリの確保・解放と IR の複製に使われていた。これは別の仮説として扱う。
