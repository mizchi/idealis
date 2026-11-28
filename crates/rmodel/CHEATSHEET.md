# rmodel 文法チートシート

## 目的からコマンドを選ぶ

リポジトリのルートで `cargo build --locked --manifest-path crates/rcheck/Cargo.toml` を実行する。下の `rcheck` は `crates/rcheck/target/debug/rcheck`。

| やりたいこと                                 | 実行するコマンド                                                                                                                                                              |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 性質を一つ検査する                           | `rcheck model.model --question 名前`                                                                                                                                          |
| 性質ごとに使える検査器を事前に調べる         | `rcheck capabilities model.model`。[対応範囲の見方](../rcheck/README.md#see-available-checkers-before-searching)                                                              |
| CI で期待した判定を検査する                  | `rcheck model.model --question 名前 --expect holds`。[終了コードと探索範囲](../rcheck/README.md#check-an-expected-verdict-in-ci)                                              |
| 操作と引数が一度でも有効になるか調べる       | `rcheck dead-code model.model --instances 操作名`                                                                                                                             |
| モデル全体の操作と更新分岐を点検する         | `rcheck coverage model.model`。[判定と終了コード](../rcheck/README.md#audit-reachability-across-a-model)                                                                      |
| モデル全体の引数の組も点検する               | `rcheck coverage model.model --instances`。[集計と上限](../rcheck/README.md#audit-reachability-across-a-model)                                                                |
| 完了前に実行できる操作が尽きるか調べる       | `rcheck deadlock model.model --unless 完了条件`。完了条件は `reachable` の名前。[ロックの例](../rcheck/README.md#find-a-state-with-no-enabled-operation)                      |
| 待機確認はできるが状態が進まないか調べる     | `rcheck deadlock model.model --unless 完了条件 --ignore-stutter`。[再利用バリア](../../examples/model-patterns/worker-barrier/reuse.model)                                    |
| 違反や到達の操作列を取り出す                 | `rcheck witness model.model 名前`                                                                                                                                             |
| 前後条件を満たす操作までの経路を探す         | `rcheck witness-step model.model 操作名 '前後条件'`。[記述例](../rcheck/README.md#find-a-step-satisfying-a-predicate)                                                         |
| 更新の各分岐に到達する経路を調べる           | `rcheck branches model.model 操作名`。ソースか、分岐マップ付きの IR を使う。[応答の見方](../rcheck/README.md#find-paths-through-conditional-updates)                          |
| 操作を実行する前にモデル上の合法性を調べる   | `rcheck validate model.model input.json`。[入力形式](../rcheck/README.md#validate-before-running-a-command)                                                                   |
| 用語、型、テストの境界ケースを得る           | `rcheck generate model.model glossary`、`typescript`・`rust`・`go`・`moonbit`、`pbt-cases`。[注文・決済の手順](../../examples/model-patterns/checkout/README.md)              |
| 実装で観測した途中状態を保存して再照合する   | `rcheck record-itf` と `rcheck replay-itf`。[ITF の保存と再生](../../examples/model-patterns/checkout/README.md#itf-ファイルから再照合する)                                   |
| 同じ到達状態を独立した Lean の命題で検査する | `rcheck lean-oracle model.model Oracle.lean`。[注文モデルの命題と実行例](../../examples/model-patterns/checkout/README.md#lean-で状態を検査する)                              |
| 同じ観測を返す二つの操作列を探す             | `rcheck observation-pair model.model request.json`。[入力と出力](../rcheck/README.md#finding-a-pair-of-indistinguishable-snapshots)                                           |
| 内部操作を省いて公開イベントを照合する       | `observation-pair` の要求 JSON に `silent` や条件付きの `emit` を指定する。[通知の例](../rcheck/README.md#finding-a-pair-of-indistinguishable-snapshots)                      |
| 接続する前に部品のポートと操作の型を調べる   | `rcheck component 部品.component.json`。[部品の定義と接続](#同じ部品を二度つなぐ)                                                                                             |
| 接続先で部品の仮定・保証・到達条件を調べる   | `rcheck contracts project.rmodel.json`。[保存と配信の接続例](../../examples/model-patterns/offline-merge/two-stores/README.md)                                                |
| 規則から関連するモデル検査と反例を引く       | `rcheck traceability spec.json --rule 規則ID`。検査名から引く場合は `--check モデルパス 検査名`。[規則と検査を引く](../rcheck/README.md#follow-a-rule-to-its-checks-and-back) |
| 実際の画面・Node.js・ストアの観測を照合する  | `@mizchi/idealis/patterns` の環境別アダプターと `recordReplay` を使う。[パターンを接続する](../../docs/rmodel-patterns.md)                                                    |
| 外部検査器の経路や時相反例を再生する         | `rcheck checker-request model.model 名前` と `rcheck checker-verify request.json reply.json`。[証拠の交換](../rcheck/README.md#exchange-evidence-with-an-external-checker)    |

探索結果の `unknown` は未確定で、`bounded` は指定スコープ内の結果。通常のモデル検査で判定を扱う場合は JSON の `verdict` と `coverage` を確認する。`rcheck --help` でコマンドと生成先を確認できる。[rcheck の判定と診断](../rcheck/README.md)

`--checker all` を使ったときは、問いごとに `answers` と `unsupported` を確認する。たとえば注文の進行性は `rcheck checkout.model --question OrderDecides --checker rprob` で検査できる。到達経路は `witness --checker rprob`、時相の反例は通常の判定でも `answers[].lasso` に出る。[経路の取り出し方](../rcheck/README.md#witnesses-for-replay)

`coverage` の結果では、全体の `status` と、操作の `coverage`、分岐の `branchCoverage`、引数の `instanceCoverage` を確認する。引数一覧の上限を超えたときは `partial`、集計対象外の条件式は `excluded` に現れる。[到達性の仕様](../rcheck/README.md#audit-reachability-across-a-model)

`rcheck witness model.model Scope --checker rprob` で、整数の保存がスコープを外れるまでの操作列と、範囲外の値を含む終状態を取得できる。[経路の取り出し方](../rcheck/README.md#witnesses-for-replay)

`capabilities` の `scopeWitness` で組み込み診断名 `Scope` と対応する検査器を確認できる。参照評価器を使う場合は `rcheck witness model.model Scope`。対応可否を表示する段階では、スコープ逸脱の有無は探索しない。

`witness` は既定で参照評価器を使う。明示的に選べるのは `--checker rprob`。`witness` が返すモデルの経路は、`reachableScenario` などを通して実装に渡せる。`record-itf` の入力は設定・初期状態・操作列と、初期時点から各ステップまでの実装観測を含む記録。TypeScript の場合は `recordReplay` でその観測を取り、`toReplayItf` で保存できる。[自動保存の規則から再生まで](../../examples/model-patterns/autosave/README.md) · [実装に接続する手順](../../docs/rmodel-patterns.md)

型検査済みのモデルを保存するには、`rcheck generate model.model ir > model.json` とする。`witness` や `validate` はこの `.json` ファイルを読める。複数のソース断片を指定する `.rmodel.json` はプロジェクト定義に使う。[JSON IR](../rcheck/README.md)

IR から更新の分岐を調べる場合は、同じソースから `rcheck generate model.model branch-map > branches.json` を実行し、`coverage` や `branches` に `--branch-map branches.json` を渡す。[分岐マップの手順](../rcheck/README.md#carry-branch-diagnostics-alongside-the-ir)

`let status = if { … } then … else …` が代入式の値として使われるときは、`branches` の `binding` に `status` が出る。実際にその値を使った操作までの経路を腕ごとに確認できる。分割ダウンロードの `request` が例。[範囲応答のモデル](../../examples/model-patterns/cache-refresh/range-assembly.model)

代入で組み立てるレコードのフィールドに `if` を直接書いても、その位置に診断が出る。無効な更新や、選ばれなかった外側の分岐はそのフィールドの到達に数えない。条件式の述語内で評価する値は集計の対象外。

操作内の特定の枝を見たいときは、前後条件を指定する。たとえば注文・決済で承認済みの要求を再送する経路は次の3手。

```sh
rcheck witness-step examples/model-patterns/checkout/checkout.model capture \
  'is(before(order.payment), Captured)' --max-states 20000
```

`place`、初回の `capture`、再送の `capture` が返る。`dead-code` は操作と引数の組の到達性を調べる。ここで指定したような前状態の枝は `witness-step` で個別に確認できる。

## 宣言と式

| 書きたいもの                 | 書き方                                                                                                                   |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| 名前と説明を持つドメイン用語 | `entity Worker "作業者"`。複数行は `description = """…"""`、翻訳は `description = { ja: "…", en: "…" }`。                |
| 個体の型                     | `sig Lock`、または説明のない `entity Lock`。個体数は `scope Lock = 1` で指定する。                                       |
| 名前の付いた個体             | `enum Carrier = Post \| Courier`。各メンバーが一つの個体になる。                                                         |
| 複数の型をまとめる           | `type Actor = Worker + Robot`。所属する個体は各型の和になる。                                                            |
| 固定の設定・変化する状態     | `rel authorized: Lock -> set Actor`、`var holder: Lock -> lone Actor`                                                    |
| 多重度                       | `set` は任意個、`lone` は0か1個、`one` は1個、`some` は1個以上。二項関係では左辺の個体ごとに右辺の個数を制約する。       |
| 集合と関係の演算             | `A + B` 和、`A - B` 差、`A & B` 共通部分、`A -> B` 直積、`l.holder` join、`none` 空集合                                  |
| 集合についての条件           | `A in B` 包含、`A not in B` 包含しない、`A = B` 等値、`A != B` 非等値、`no A` 空、`#A` 集合の要素数、`#r` 二項関係の組数 |
| 量化と論理                   | `all l: Lock \| 条件`、`some l: Lock \| 条件`、`not`、`and`、`or`、`implies`、`iff`                                      |

`P iff Q` は両方の条件の真偽が一致するときに成立する。`conflict iff some readout.Delivered and some readout.Missing` は、到着と欠落の両方が観測された場合に限って矛盾と判定する。`iff` は `implies` より弱く結合する。複数の `iff` を組み合わせるときは括弧で区切る。[配信競合](../../examples/model-patterns/subscriptions/delivery-race.model)

`type Registration = LegacyRegistration + CurrentRegistration` とすると、停止・制御の引数に `Registration`、旧形式の送受信に `LegacyRegistration` を使える。全登録と旧登録を `&` で交差させた結果は旧登録の集合になる。[デコーダーの更新](../../examples/model-patterns/crash-recovery/decoder-membership.model)

`keys(relation)` は、二項関係に現れる左側の値を重複なく返す。`pending: Ticket -> lone Connection` なら、`keys(pending) in issued` で接続中のチケットが発行済みであることを検査できる。型の付いた空の関係からは空集合を返す。[接続の再利用](../../examples/model-patterns/subscriptions/delivery-slot-reuse.model)

`select(ticket: Ticket | ticket.issued < floor)` は、有限の `Ticket` から条件を満たすキーの集合を返す。更新の右辺で使うと、条件も更新前の状態で評価される。期限切れのキーを `expired` に束縛し、`withoutKeys(receipts, expired)` で二項関係からそのキーを持つ行をすべて除く。[一括削除のモデル](../../examples/model-patterns/offline-merge/batch-retention.model)

`pending: set Nat` は未完了の世代番号を保持できる。追加は `union(pending, visitId)`、取り除くには `difference(pending, visitId)`、含まれるかは `1 in pending` と書く。整数の `+` と `-` は単一の値の算術に使う。[再訪モデル](../../examples/model-patterns/browser/navigation-activation.model)

`#seen` は登録と連番の組の数を返す。同じ連番が2つの登録に現れれば2件。`#keys(seen)` は登録の数。`effects = #seen` なら登録を増やしても式が増えない。Alloy は個数の上界が15を超えうる式を対応外として報告する。[再配送のモデル](../../examples/model-patterns/crash-recovery/decoder-identity.model)

`reachableFrom(roots, edges)` は根の集合を含み、辺を0回以上たどって到達できるノードを返す。辺の両端は同じ atom sort か、一方が他方に含まれる sort を使う。`next: Position -> lone EntryPosition` で根が `EntryPosition` なら、結果も `EntryPosition` の集合になる。原点から探す結果には原点も含まれる。空の根は空集合、空の辺は根の集合を返す。[式を使ったグラフの例](../../examples/model-patterns/worker-transfer/reachability-expression.model)

`closure(edges)` は1本以上の辺をたどれる順序対を返し、辺の始点・終点の型を保つ。`n.closure(edges)` は `n` から到達する集合で、`n` 自身は循環で戻れる場合に含む。開始点を常に含めるなら `reachableFrom(n, edges)` を使う。どの取得位置より後ろにあるかをパーティションごとに求めるには、`captured.closure(next)` と書ける。[スナップショットの復旧](../../examples/model-patterns/subscriptions/partition-snapshot.model)

`fact` は設定を制約し、`init` は初期状態を制約する。ブロックに条件を複数並べると、すべて成立することを要求する。条件の境目は `no published, (Prerequisite - ThresholdMet) in confirmed` のようにカンマでも表せる。次の条件が括弧で始まるとき、前の名前に続く呼び出しと区別できる。操作の右辺は遷移前の状態で評価し、更新は同時に反映する。代入を省いた変数は値を保つ。

```rmodel
model Locking
entity Worker "ロックを取得する作業者"
entity Lock "同時に一人だけが保持できる資源"
enum Robot = Bot
type Actor = Worker + Robot

rel authorized: Lock -> set Actor
var holder: Lock -> lone Actor
fact { all l: Lock | some l.authorized }
init { no holder }

behavior acquire(a: Actor, l: Lock) {
  guard { a in l.authorized no l.holder }
  update { holder = holder + l -> a }
}
behavior release(a: Actor, l: Lock) {
  guard { a in l.holder }
  update { holder = holder - l -> a }
}

invariant OnlyAuthorized { all l: Lock | l.holder in l.authorized }
reachable Held { some holder }
scope Worker = 1, Lock = 1
```

`OnlyAuthorized` は成立し、`Held` は取得操作1回で到達する。旧表記の `action` では条件と `holder' = 式` を同じブロックに書ける。

同じ条件で複数の項目を更新する場合は、`update` 内で枝をまとめられる。次の CAS は比較が成立したときだけ `current` を書き換え、結果はどちらの枝でも記録する。

```text
update {
  if { saved = current } then {
    current = Word { slot: C, tag: current.tag + 1 }
    result = Applied
  } else {
    result = Rejected
  }
}
```

`else` は省略できる。選んだ枝で代入しない変数は値を保ち、条件と各右辺は遷移前の状態を読む。[CAS モデル](../../examples/model-patterns/aba/register.model)

高々一つの値を読むときは、`some` を先に書く。次の例では `request.captured` は `lone Nat` で、値がない要求には `apply` が有効にならない。

```text
behavior apply(request: Request) {
  guard { some request.captured and request.captured >= cursor }
  update { cursor = request.captured }
}
```

同じ指定を二つのガードに分けてもよい。条件式では成立側で使える。反対側や `or` の一方だけの条件からは単一値として扱えない。[再同期モデル](../../examples/model-patterns/lease/resync-order.model)

タブごとの保存版を `pending: Tab -> lone Nat` で表すと、`tab.pending` は1タブ分の読み取り、`pending ++ tab -> revision` はそのタブの版の設定・置換になる。削除はガードで `some tab.pending` を確認して `pending - tab -> tab.pending` と書く。数値比較で高々一つの値を読む場合は、その値より先に `some` を書く。欠けている場合、診断は比較する値の位置を指す。`scope_exit` が出たら、たとえば `scope Int = 0..2` に対して増分のガードが `revision < 2` になっているか確認する。[自動保存の完全な例](review.md#自動保存の小モデルで書いて確かめたこと)

値を選ぶ `if { 条件 } then 式 else 式` は、更新の右辺や不変条件の等式内に書ける。`label = (if { generation = Second } then B else A)` は、世代に対応するラベルを選んで比較する。有限 atom のこの例は SAT でも検査できる。[版ラベルのモデル](../../examples/model-patterns/worker-transfer/graph-version-reuse.model)

`derive matches = bool { localRevision = required, closed }` は、二つの条件がそろえば `true`、それ以外なら `false` を返す。条件を Bool 値にして、更新や導出値に使える。[受領記録の復旧](../../examples/model-patterns/crash-recovery/receipt-checkpoint.model)

操作を一度でも実行できるかは `rcheck dead-code model.model`、引数の組ごとに確認するには `rcheck dead-code model.model --instances acquire` を使う。宣言された有限型に含まれ、ガードで意図して拒否する入力も一覧に現れる。[CLI と注文モデルの例](../rcheck/README.md#dead-behavior)

## 複数の操作や性質で式を共有する

`derive` や `let` の定義の中で束縛した名前は、参照先の式と重なっても元の変数を指す。たとえば内側の候補集合と外側の選択式の両方で `select(candidate: Client | …)` と書ける。[最初に収まる要求の選択](../../examples/model-patterns/semaphore/weighted-first-fit.model)

モデル直下に `derive 名前 = 式` と書くと、設定や現在の状態から計算する式に名前を付けられる。他の導出式を使う場合は先に宣言する。下の `missing` は取得前後のそれぞれの状態で評価される。

```text
derive dependencies = forward + (OrderRow -> backedge)
derive missing = captured.closure(dependencies) - captured

transition FetchReducesOutstandingDependencies on fetch {
  after(missing) = before(missing) - row
}
```

`derive` の右辺は単一時点の式。`before` と `after` は性質で参照するときに指定する。参照箇所には式が展開され、探索する変数は増えない。JSON IR は導出名と式も保持する。操作の引数や局所変数を使う計算は、操作の `update` の `let` で書ける。[依存行の復旧モデル](../../examples/model-patterns/subscriptions/partition-repair.model)

`rcheck witness-step examples/model-patterns/subscriptions/partition-repair.model plan 'some after(missing)'` で、復旧計画を作った直後に不足行がある経路を探せる。`observation-pair` の観測・区別・通知条件でも導出名を使える。[通知を照合する例](../rcheck/README.md#finding-a-pair-of-indistinguishable-snapshots)

## 性質の選び方

| 知りたいこと                                       | 宣言                                                                                                  |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| 初期状態を含む、到達可能なすべての状態で成立するか | `invariant 名前 { 条件 }`                                                                             |
| 条件を満たす状態にたどり着けるか                   | `reachable 名前 { 条件 }`                                                                             |
| 有効な各操作の直前と直後を比較したい               | `transition 名前 { 条件 }`。式は `before(e)`・`after(e)` で時点を指定する。                           |
| 同じ式の値が操作をまたいで保たれるか               | `unchanged(e)` は `before(e) = after(e)` に展開する試作構文。引数には位相指定を含まない状態式を書く。 |

```rmodel
model Shipment
enum Carrier = Post | Courier
variant Status = Ready | Shipped(Carrier)
var status: one Status
init { status = Ready() }

behavior ship() {
  guard { is(status, Ready) }
  update { status = Shipped(Post) }
}
behavior reroute() {
  guard { status = Shipped(Post) }
  update { status = Shipped(Courier) }
}

invariant HasCarrier { is(status, Shipped) implies some payload(status, Shipped) }
reachable CanShip { is(status, Shipped) }
transition KeepsTracking {
  (is(before(status), Shipped) and is(after(status), Shipped))
    implies unchanged(payload(status, Shipped))
}
scope steps = 4
```

この例では `reroute` が配送方法を書き換える。各状態に配送方法は入っているため `HasCarrier` は成立する。 `CanShip` は1手で到達し、`KeepsTracking` は `ship`、`reroute` の2手で違反する。

idealis 自身の採用モデルでは、`unchanged(current)` で同じ版の遷移を選び、採用済みなら `unchanged(adopted)` で保持を求める。改版は採用を解除する条件として別に検査する。[idealis の採用モデル](../../spec/models/evidence-adoption.model) · [採用を保持する規則](../../spec/rmodel-design.md#rmodel-adoption-lifetime)

検査器比較では、量化した版 `r` と検査器 `c` に対して `unchanged(c & r.completed)` を書く。その組の完了状態だけを比較するので、別の組の結果は追加できる。性能測定では、改善を発行する操作で `unchanged(confirmed)` を求める。[検査器比較](../../spec/models/checker-comparison.model) · [測定の発行](../../spec/models/measurement-gate.model)

配送の操作だけで成り立つ規則は `on deliver` で指定する。`request` は `deliver(request: Request)` の引数。

```text
transition RetryPreservesEffect on deliver {
  request in before(receipts) implies unchanged(request.effects)
}
```

[再送のモデル](../../examples/model-patterns/retry/delivery.model)

前状態で読んだ値を後状態のキーに使う場合は、`transition` の先頭で `let` を使える。たとえば候補がスロット内のチケットと一致した場合、後状態ではそのチケットの取得者に読み手が含まれる。

```text
transition FreshCandidateClaimsItsTicket on claim {
  let candidate = before(reader.observed)
  (some candidate and candidate = before(reader.observed.location.cells))
    implies reader in candidate.after(claimed)
}
```

`candidate.after(claimed)` は前状態で読んだ候補を後状態の `claimed` と結合する。束縛名の参照は `before(...)` や `after(...)` の外に置く。`some candidate` は候補が空の場合に含意の前提が成立するのを防ぐ。[リングのスロット再利用](../../examples/model-patterns/worker-deque/ring-reuse.model)

`unchanged(#members)` は要素数の保持を表す。たとえば集合を `{A}` から `{B}` に入れ替えても要素数は1のまま。その遷移で `unchanged(members)` は破れる。

## 値を扱う

`data Amount = Nat` は `Amount(1)` で構築し、`amount.value` で中の値を読みます。同じ `Nat` を包む `Fee` とは別の型です。複数の値をまとめるときは `data Request = { amount: Amount, fee: Fee }` と書きます。[支払申請のモデル](../../examples/model-patterns/payment-data/payment.ideal)

`data Amount = Nat where { value <= 1000 }` は支払額の上限を定義します。申請全体なら `data Request = { amount: Amount, fee: Fee } where { fee.value <= amount.value }` と書きます。試作では `Int`・`Nat` のフィールドを整数リテラルか別の数値フィールドと比較でき、複数の比較を `and` またはカンマで連ねられます。`scope Int = 0..2` は探索時に使う整数だけを指定します。[範囲付きの支払申請](../../examples/model-patterns/payment-data/payment-refined.ideal)

レコードのフィールド名は型ごとに扱う。`Amount { units: Nat }` と `Receipt { units: Nat }` のように同じ名前を使える。`receipt.units` は左辺の型からフィールドを選ぶ。

`lone` のレコードを数値比較に使うときは、存在を先に確認する。例として `some accepted implies accepted.revision >= minimum` がある。連言では `some accepted and accepted.revision >= minimum` の順に書く。遷移の更新後を読むときは `some after(accepted)` で更新後の存在を確認する。

導出した関係から読むときは、`guard { one label.latest }` で結果がちょうど一値と確認できる。更新では `at(snapshots, label.latest)` の添字に使える。[名前付きセーブポイント](../../examples/model-patterns/sqlite-transactions/savepoint-named.model)

`variant` はタグごとに必要な値が異なる場合に使う。注文では未処理を `Pending()`、確定を `Captured(Receipt { units: 2 })` と書く。`payload(order.payment, Captured)` は、注文のタグが `Captured` のとき領収書を返し、別のタグのとき空になる。[注文モデル](../../examples/model-patterns/checkout/checkout.model)

| 書きたいもの           | 書き方                                                                                                                                                                                                 |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 整数、自然数、真偽値   | `Int`、`Nat`、`Bool`。整数範囲は `scope Int = 0..2`。 `Nat` はその範囲の0以上の部分を使う。                                                                                                            |
| フィールド付きの値     | `record Entry { units: Nat }`、`Entry { units: 1 }`、`e.units`。操作引数が `units` でも、`e.units` は `Entry` のフィールドを読む。                                                                     |
| タグと任意のペイロード | `variant Status = Ready \| Shipped(Carrier)`、`Ready()`、`Shipped(Post)`、`is(status, Shipped)`、`payload(status, Shipped)`                                                                            |
| 有限列                 | `sequence Journal = Entry, max = 2`、`Journal[]`、`append(log, e)`、`tail(log)`、`take(log, 2)`、`removeAt(log, 1)`、`removeAll(log, ids)`、`len(log)`、`at(log, 0)`、`elements(log)`。添字は0始まり。 |

```rmodel
model SmallJournal
record Entry { units: Nat }
sequence Journal = Entry, max = 2

var log: one Journal
init { log = Journal[] }
behavior appendEntry(e: Entry) {
  guard { len(log) < 2 e.units > 0 }
  update { log = append(log, e) }
}

invariant Bounded { len(log) <= 2 }
reachable FirstEntry { at(log, 0) = Entry { units: 1 } }
scope Int = 0..2, steps = 3
```

`tail(log)` は先頭を取り除き、空列には空列を返す。`removeAt(log, i)` は指定位置の1件を取り除き、範囲外なら列を保つ。[FIFO の例](../../examples/model-patterns/fifo/project.rmodel.json)

`some i: Nat | at(log, i) = entry` は、`scope Int` の非負部分に含まれる添字を検索する。整数全体の指定範囲を使うなら `i: Int` と書く。添字が範囲外なら `at` は空になる。[最初に収まる要求の選択](../../examples/model-patterns/semaphore/weighted-first-fit.model)

`guard { some at(log, 0) }` を置くと、`update` の `let first = at(log, 0)` を単一の値として扱える。タグ付きの要素なら `is(first, Tag)` で分岐できる。[終了マーカーの例](../../examples/model-patterns/subscriptions/delivery-ticket-drain.model)

`Bounded` は成立し、`FirstEntry` は追加1回で到達する。 `scope` は有限モデルの範囲を決める。SAT の経路長は `rcheck <model> --checker sat --sat-steps N` で指定する。省略時は既定の4状態を使う。従来の `scope steps = N` も SAT の設定として読める。エンジンごとの対応範囲と探索予算は個別に確認する。[対応範囲](SUPPORT.md)

初期値を `init { calls = Invocations[] }` のような閉じた式で固定すると、参照評価器は値域全体を展開せずにその値から進める。4種類・長さ4の列では341通りのうち空列から始め、1手で4件のバッチに到達した。自由な初期値は探索予算を使う。値域の実体化上限に達したものは、探索の完了と区別して報告される。[I/O バッチ](../../examples/model-patterns/node-event-loop/nested-io-batch.model)

## 進行性と公平性

`eventually 名前 { 条件 }` はいつか条件を満たすことを求める。 `leads_to 名前(c: Client) { 前件 } { 後件 }` は、前件が成立するたびに後件がいずれ成立することを求める。 `fairness weak serve` は実行可能な状態が続く操作に、`fairness strong serve` は実行可能になる機会が繰り返しある操作に公平性を課す。各性質の `with { ... }` は既定の公平性を置き換える。 `with {}` で空にできる。

`strong acquire` は、取得できる worker が繰り返し現れるなら、誰かが取得し続けるという前提。`strong acquire(*)` は、取得できる機会が繰り返しある各引数の組に、その前提を課す。二つの worker が同じロックを取り合うモデルで違いを検査した。[取り合いと FIFO のモデル](../../examples/model-patterns/README.md)

```rmodel
model Service
entity Client
var waiting: set Client
var served: set Client
init { no waiting no served }

behavior request(c: Client) {
  guard { c not in waiting c not in served }
  update { waiting = waiting + c }
}
behavior serve(c: Client) {
  guard { c in waiting }
  update { waiting = waiting - c served = served + c }
}

fairness weak request
fairness weak serve
eventually FirstService { some served }
leads_to ServesWaiting(c: Client) with { weak serve } { c in waiting } { c in served }
leads_to MayStarve(c: Client) with {} { c in waiting } { c in served }
scope Client = 1, steps = 4
```

`FirstService` と `ServesWaiting` はそれぞれの公平性の下で成立する。 `MayStarve` には受付後に永遠に待たせる反例がある。時相の性質は TLA+ へ変換して検査する。

## 同じ部品を二度つなぐ

プロジェクトの `fragments` に `{ "path": "storage.model", "namespace": "east" }` を書くと、部品の `@head` と `@commit` は `east_head` と `east_commit` になる。共有変数は `ports` で接続し、`read` と `write` を指定する。部品が読める状態は自身の変数とポートの接続先。書き込みの割り当ては共有変数ごとに一つ。同じファイルを `west` でも使える。[保存と配信の接続](../../examples/model-patterns/offline-merge/two-stores/README.md)

manifest の `exports` に `["east_commit", "west_commit"]` のように接続後の操作名を書く。`rcheck generate project.rmodel.json go --public` で公開操作の型と interface を生成できる。検査と証拠経路は内部の配信操作も使う。[公開操作の接続例](../../examples/model-patterns/offline-merge/two-stores/README.md)

部品の `.component.json` に型断片、ポートを宣言するモデル、本体、読み書きを指定し、`rcheck component 部品.component.json` で型検査する。プロジェクトは各ポートの接続先を指定し、同じ型断片を含める。[部品の接続例](../../examples/model-patterns/offline-merge/two-stores/README.md)

`contract` に仮定、保証、到達条件を部品内の性質の名前で記述できる。`rcheck contracts project.rmodel.json` で合成先の状態を探索し、破れた仮定や保証と、到達した操作の経路を確認する。[動作契約](../rcheck/README.md#check-a-components-behavior-in-a-project)

## 実行と構文の変更

上のモデルを一つずつ `.model` に保存して検査できる。リポジトリのルートで、発送例の違反を見るには次を実行する。

```sh
cargo run --quiet --locked --manifest-path crates/rcheck/Cargo.toml \
  -- shipment.model --checker reference --question KeepsTracking
```

`just rmodel-test` はこのページからモデルを読み、参照評価器の判定・最短深さと TLA+ による公平性の有無の差を検査する。構文を変えるときは、先に書きたい例と互換性の条件を記録し、同じ変更で必要な例と期待値を更新する。[文法の意思決定ログ](grammar-decisions.md) · [検査コード](tests/cheatsheet.rs)

[ブラウザ・Node.js・オブジェクトストアの接続パターン](../../examples/model-patterns/README.md)では、同じモデルを実行して環境の観測と照合する。

[修正・不足機能・候補の台帳](backlog.md)

## 一つの操作内で更新値を選ぶ

条件付き更新で意図して到達不能にしている腕は、`then expect unreachable "理由" { ... }` または `else expect unreachable "理由" { ... }` と書く。`rcheck coverage` で期待を確認でき、腕に到達したら操作列付きの `expectation_mismatch` になる。[受領記録の例](../../examples/model-patterns/crash-recovery/receipt-wire.model)

代入の右辺に直接書く `if` では、腕の値の前に置く。`else expect unreachable "閉鎖後の版のみ復旧する" false` のように書ける。[チェックポイントの復旧](../../examples/model-patterns/crash-recovery/receipt-checkpoint.model)

`update` の先頭で `let nextRevision = revision + 1` と書くと、更新前の状態から得る同じ式を後続の束縛や複数の代入から参照できる。束縛はその `update` の中で使える。[編集の再送](../../examples/model-patterns/offline-merge/retry.model)

`shown = if { request = latest } then request else shown` は、完了した要求が最新のときだけ表示を替える。条件と選ばれた値は操作前の状態から読む。同じ操作で `pending` を更新しても、条件の読み取りには影響しない。[検索のモデル](../../examples/model-patterns/latest-request/search.model)

`append(take(history, cursor + 1), destination)` は現在のエントリまでを残し、次の画面を追加する。`take` の件数が0以下なら空列、長さ以上なら元の列を返す。[認証付き画面履歴のモデル](../../examples/model-patterns/auth-history/history.model)

`loaded = loaded ++ tab -> version` は、そのタブの行を読み込んだ版で置き換える。右辺に現れないタブの行は維持する。右辺に同じキーの値が複数あればすべて残り、多重度は代入先で検査する。[2タブの同時編集](../../examples/model-patterns/tab-settings/settings.model)

`elements(pending) = outcome.Waiting` は、待機列の値と「待機中」の値が同じ集合であることを表す。重複や順序を調べるときは列を比較する。[ストリームの終端](../../examples/model-patterns/stream-terminal/stream.model)

`removeAll(waiting, item)` はその値をすべて取り除き、残りの順序と重複を保つ。右辺には除外する値の集合を渡せる。[待機書き込みの取り消し](../../examples/model-patterns/backpressure/channel.model)

列の先頭を末尾へ移す式は `append(tail(pending), at(pending, 0))`。`some at(pending, 0)` をガードに置くと、`append` に渡す値が1つあることを型検査でも示せる。`at` は範囲外なら値を返さない。保存結果が列の最大長を超えると `Scope` に違反する。[再試行待ちのキュー](../../examples/model-patterns/crash-recovery/rotate-queue.model)

`Checkpoint { pending: saved.pending, retries: saved.retries - 1 }` は、保存済みのキューを保って再試行回数を1つ減らす。回数を `Nat` へ書き込む場合は、`saved.retries > 0` をガードに置く。復旧前の `restored` は `lone` なので、`some restored implies ...` で値があるときにフィールドを検査できる。[再起動と試行回数](../../examples/model-patterns/crash-recovery/restart-budget.model)

履歴を列のまま保持し、`batch in elements(applied)` でバッチ全体が適用済みか調べられる。`#elements(applied) = len(applied)` は、同じバッチが列に重複していないことを表す。[適用済みバッチの記録](../../examples/model-patterns/crash-recovery/deduplicate-batch.model)
