# rcheckの対応チェックリスト

チェック済みは、rmodelのモデルの問いを、登録した検査器が宣言した範囲で答え、用語集の語で返す範囲。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] `--checker rprob` と `--checker all` で有限の安全性・到達性・時相を検査する。対応外の理由は `capabilities` と共有し、時相の違反では再生済みの循環を返す。[登録と CLI の試験](tests/capabilities.rs)

- [x] 問い: モデルの`invariant`・`reachable`と、組み込みの`Declarations`（0101）。
- [x] 検査器の宣言（`supports`）: 参照評価器は関係の取りうるtupleが20以下ならcomplete、明示状態（rtlc）はcomplete、SAT（ralloy）はstepsまでのbounded（0101）。
- [x] SAT に渡す Alloy の宣言数を事前に確認し、上限を超える場合は種類・件数・上限を対応外の理由として返す。型検査や探索のほかの制約はそれぞれの経路で報告する。[境界の回帰試験](tests/registry.rs)
- [x] 答え: 成り立つ・破れる・到達できる・到達できない・空・未確定・エラーと範囲、破れる・到達できるときの最短の深さ。boundedの成り立つ・到達できない、予算切れ、エラーをcompleteの答えとして扱わない（0101）。
- [x] 選び方: `auto`はcompleteで答える最初の検査器、名前の指定、`all`は全検査器と食い違いの報告。boundedの否定の答えは、bound以上の深さの反例とは食い違わない（0101）。
- [x] `check_with` は Rust ホストが渡した検査器を使い、宣言した探索範囲と問いの種類を照合する。有界探索の答えは各検査器の範囲で比較する。[回帰試験](tests/checker_registry.rs)
- [x] `rcheck capabilities MODEL` は検査器の対応範囲と rprob の経路・循環の探索可否を問いごとに出す。組み込み診断の `witness MODEL Scope` と rprob の対応可否は `scopeWitness` に出す。判定には実行時と同じ対応検査を使い、到達状態は探索しない。[CLI の回帰試験](tests/capabilities.rs)
- [x] `checker-request` と `checker-verify` は外部検査器と IR・判定・経路を交換する。有限の到達・違反に加え、時相の循環反例を再生し、閉路・公平性・進行性を照合する。[回帰試験](tests/external_checker.rs)
- [x] `--question NAME --expect VERDICT` は通常の JSON に加えて CI 用の終了コードを返す。有界の不成立証拠と完全探索が必要な判定を区別し、未確定を成功にしない。[回帰試験](tests/expectations.rs)

- [x] 予算: 参照評価器は、到達した状態だけでなく、調べた設定と初期状態の候補も予算に数え、組み合わせを1つずつ作る（0101、0100のcrashした2 seed）。
- [x] 通常の参照検査と経路生成は、無条件の `fact`・`init` で固定された値を先に束縛する。競合する ID 割り当てで状態予算2万の判定と深さを明示検査器と照合した。[回帰試験](tests/registry.rs)

- [x] 値（0103）: SATは整数とrecordのモデルに対応しないと宣言する。明示状態は、否定の答えの前に組み込みの`Scope`を検査し、状態が整数の範囲の外へ出たら未確定と答える。

- [x] 実装の照合（`conform`、0102）: 参照評価器を正本に、設定ごとに実装の初期状態がモデルの初期状態であること、到達する各状態で各action instanceの受け付けと次の状態が一致することを、幅優先で確かめる。答えは一致・bounded（設定の上限）・不一致（最短のcommandの列、期待と実測）・invalid（JSON Schemaに合わない答え）・空・未確定（呼び出しの予算）・エラー。driverは同じプロセス（生成したRustのtrait）か、JSON Linesの外部プロセス（生成したTypeScriptの`serve`）。
- [x] `generate`: モデルからRustのtrait、TypeScriptの型とdriver、Goのinterfaceとdecoder（0104）、MoonBitのtraitとdecoder・`Server::handle`（0105）、JSON Schema、到達できる状態を生成する（0102）。JSON Schemaの検査は、生成するキーワードだけを扱う`schema::validate`で、知らないキーワードはエラーにする。

- [x] 核のIRのJSON（version 1、`ir`、0108）: rmodelの型検査前の構文木。式・論理式は`op`で区別する木、知らないtag・欠けたfield・知らないfieldは拒む。`rcheck generate <model> ir`で書き、`.json`の入力は`rmodel::validate`で型検査してから検査器に渡す。adapterが写せなかった部分は`omitted`。
- [x] IR の版をフィールドや式の型より先に検査する。初期の Pkl IR を固定して現行ソースの判定と到達経路に照合する。[固定入力](../../fixtures/rcheck/ir-compat/README.md) · [回帰試験](tests/ir.rs)
- [x] 明示的な集合の和と差は `set_union`・`set_difference` として IR に運ぶ。算術の `add`・`sub` と区別し、負数を含む集合の所属と更新を参照検査器・明示検査器で照合する。[回帰試験](tests/ir.rs)
- [x] `select` の変数・型・述語を IR に運び、参照検査器と明示検査器の到達判定を照合する。ralloy の集合内包は対応外として報告する。[回帰試験](tests/ir.rs)
- [x] `without_keys` を IR に運び、キーが値を共有する場合と削除対象が存在しない場合を参照・明示・SAT の各検査器で照合する。[回帰試験](tests/ir.rs)
- [x] `closure` を IR の式のタグとして往復し、二項関係として型検査する。[IR の往復テスト](tests/ir.rs)
- [x] `entity` の用語定義を IR の任意フィールド `entity_definitions` と `generate glossary` に写す。定義を持たない IR は従来の形式で読める。[IR の往復テスト](tests/ir.rs)。
- [x] `rcheck dead-code` は参照評価器で、到達する状態で成立するガードを操作ごとに調べる。`--instances ACTION` は選んだ操作の引数の組を最大4,096件まで個別に調べる。未確定、空、IR の欠落をデッド扱いしない。[到達性の CLI テスト](tests/dead_code.rs)。
- [x] `rcheck deadlock` は操作の全引数のガードを調べ、有効な操作がない到達状態の経路を返す。`--unless` で完了条件を指定し、初期状態なし・未確定・宣言違反を区別する。[回帰試験](tests/deadlock.rs)
- [x] `deadlock --ignore-stutter` は有効な操作が同じ状態を返す場合も停止候補にする。状態が変わる操作、範囲外への遷移、宣言違反を生む遷移が残っていれば、その状態を停止と判定しない。[再利用バリアの対照](tests/deadlock.rs)
- [x] `rcheck witness MODEL QUESTION --checker rprob` は有限 atom・整数・関係の探索が得た状態・遷移・宣言の経路を共通の JSON 形式で返す。[rprob の再生試験](../rprob/tests/witness.rs)
- [x] `rcheck branches` はソースの条件付き更新群と入れ子の前提を追い、成立・不成立の各枝への経路または到達不能・空・未確定を返す。[CLI の回帰試験](tests/branches.rs)
- [x] `rcheck coverage` は一回の参照探索で全操作と条件付き更新群の到達性をまとめる。空・未確定・宣言違反を区別する。`generate branch-map` で元ファイルと分岐の入れ子を保存し、IR を使う `coverage` と `branches` に渡せる。[CLI の回帰試験](tests/coverage.rs)
- [x] `coverage --instances` は同じ探索で引数の組を集計する。列挙上限を超える操作は一覧を省略し、到達不能とは別に診断する。[CLI の回帰試験](tests/coverage.rs)
- [x] 代入の右辺が直接 `if` の場合は then・else と連続した条件式を `kind: value` として報告し、分岐マップ v2 で保存する。v1 は更新グループの情報を読み込める。[CLI の回帰試験](tests/branches.rs)
- [x] 更新の `let` が `if` で始まり、代入式の値として使われる場合は、複数の利用箇所を元の `if` の一行にまとめる。分岐マップ v3 は式へのパスを保存し、v1・v2 も読み込める。述語だけでの利用は対象外。[範囲応答と利用箇所の回帰試験](tests/branches.rs)
- [x] レコードのフィールドなど、更新の値を組み立てる式の内側の `if` も集計する。直接書いた分岐へのパスは分岐マップ v4、`let` から展開したパスは v3 で表す。述語内の評価は対象外。[HTTP 304 と範囲応答の回帰試験](tests/branches.rs)
- [x] 更新グループと代入の右辺で直接連なる `if` の `then`・`else` に `expect unreachable` を書ける。`rcheck coverage` は到達したときに経路付きで不一致を返し、予算切れは未確定にする。理由は分岐マップにも残る。[CLI の回帰試験](tests/coverage.rs)
- [x] `rcheck traceability` は rspec の規則・節とモデルの検査を双方向に引き、期待判定と現在の証拠を返す。Markdown の本文、リンクなし、探索の未確定、モデルの違反を区別する。[自己モデルの回帰試験](tests/traceability.rs)
- [x] `rcheck witness-step` は操作名・引数・前後状態の条件に合う遷移を探し、その操作を末尾に持つ経路を返す。到達不能、空、予算切れ、スコープ逸脱を区別する。[回帰試験](tests/witness.rs)
- [x] `record-itf` と `replay-itf` は有限の状態観測を ITF で保存・再照合する。一致、最初の差分、記録不足や不正な操作を区別し、符号付き64ビット整数と列の順序を保持する。[CLI の回帰試験](tests/itf_replay.rs)
- [x] 操作が0件の ITF では初期観測1件を照合する。観測がなければ入力エラーになる。[CLI の回帰試験](tests/itf_replay.rs)
- [x] 参照探索は大きな構造値の操作引数を一件ずつ生成する。`witness`、`witness-step`、`dead-code` は `--max-arguments` で試行数を指定できる。引数予算を使い切った場合、全探索の結論と既に見つけた経路を区別する。[CLI の試験](tests/witness.rs)

- [x] `rcheck observation-pair` は同じ設定の単独モデルで、指定した到達条件を満たし、観測フィールドが等しく内部フィールドが異なる二経路を返す。予算切れで組が見つからなければ unknown。[CLI の回帰試験](tests/witness.rs)。
- [x] `observation-pair` の `comparison: "history"` は、同じ設定内で初期状態と各操作後の全観測を比較する。途中の観測が異なる組を除き、予算切れやスコープ逸脱を到達不能にしない。[履歴比較の試験](tests/witness.rs)。
- [x] 履歴比較の `silent` は操作名ごとに観測を省き、異なる手数で同じ公開観測列を出す経路を返す。結果の操作列と最小合計手数には内部操作も含む。[内部リトライの試験](tests/witness.rs)。
- [x] 履歴比較の `emit` は操作引数と前後状態を持つ条件式で、名前付きイベントの有無を決める。公開時はイベント名と観測フィールドを比較する。[条件付き通知の試験](tests/witness.rs)。
- [x] 履歴の各経路は、名前付きイベントの操作番号と射影値を `events` で返す。ローカル HTTP の実行では各操作までのイベント列を照合する。[実行試験](../../tests/model-patterns/retry-threshold.test.ts)。
- [x] `derive` をソースと JSON IR の両方から `witness-step`、`observation-pair` の観測・区別・通知条件に参照できる。導出された通知値は組の配列で返す。[IR とクエリの往復](tests/witness.rs) · [HTTP の照合](../../tests/model-patterns/retry-threshold.test.ts)。
- [x] `rcheck lean-oracle` は、参照評価器が完全探索できた非空のモデルから設定と状態を抽出し、別の Lean ファイルに JSON で渡す。偽の観測と実行エラーを区別する。[入出金の命題](../../formal/rmodel/Bank.lean) · [oracle の CLI テスト](tests/lean_oracle.rs)。
- [x] adapterの差分テスト（0108）: roracleで、rmodelの文の判定を基準に、adapterのIRの判定を照らす。unsupported・omittedはunknown、拒まれたIRはerrorで、一致に数えない。
- [x] `*.rmodel.json` のファイル一覧を一つの rmodel として型検査し、チェッカーと生成器で共有する。出金と単一任期の Raft 選挙で確認する。
- [x] 同じ断片を異なる名前空間に接続し、部品の `@name` と共有変数へのポートを解決する。プロジェクトの型検査後に権限外の状態参照・操作代入と共有変数の二重書き手を拒否する。導出式を展開した状態参照も照合する。[東西の保存と配信](../../examples/model-patterns/offline-merge/two-stores/README.md) · [回帰試験](tests/linked_projects.rs)
- [x] manifest の `exports` から、Rust・TypeScript・Go・MoonBit と JSON Schema の公開コマンドを `generate ... --public` で生成する。内部操作を含む検査と証拠経路も保持する。[回帰試験](tests/public_bindings.rs)
- [x] `rcheck component` は型断片・ポート宣言・本体を単独で型検査する。リンク時に共有の型断片とポートの型・多重度を照合し、部品の読み書き権限を使う。[回帰試験](tests/components.rs)
- [x] `rcheck contracts` はリンク後の有限探索で部品の仮定・保証・到達条件をまとめて調べる。型が合う書き手による仮定の違反、成立する保証に対する未到達の入力例、予算切れを区別する。[回帰試験](tests/components.rs)
- [x] `generate <model> pbt-cases|pbt-typescript` は完全探索できる有限範囲から、隣接整数、一遷移、隣接入力後の一操作で受付が変わる組を導出する。再生用 JSON と fast-check のプロパティを生成する。探索未完了・不正・境界例なしはエラー。[CLI のテスト](tests/linked_projects.rs) · [TypeScript の正常・故障対照](../../tests/rcheck/withdrawal.pbt.test.ts)
- [x] `generate <model> pbt-go` は `generate <model> go` の interface を実装する関数を受け取り、同じ境界例を Go の fuzz seed として再生する。[Go の出金実装と故障対照](../../tests/rcheck/go/withdrawal/withdrawal_test.go)
- [x] IR の `reachable_from` は根と辺の式を保持する。構造を往復し、rcheck で同じ到達性を検査できる。[差分テスト](tests/witness.rs)。
- [x] IR の `keys` は関係式を保持する。読込時に二項関係であることを型検査する。[往復と拒否の試験](tests/ir.rs)。
- [x] CLI は `omitted` を含む IR の検査を拒否する。どの遷移や前提を落としたかの情報は診断に出す。[欠落した遷移の回帰テスト](tests/ir.rs)。
- [x] コマンド JSON の `kind` は操作名を持つ。同名の引数は `$kind` に写し、証拠経路、PBT、validator、生成する Schema と4言語のデコーダーで使う。[回帰試験](tests/witness.rs)。
- [x] 証拠経路と状態出力は、集合を要素の JSON 表現の順に揃える。列の `$sequence` は要素順を保ち、`validate` は順不同の集合を受け付ける。[回帰試験](tests/witness.rs)。
- [x] `Validator` と `rcheck validate` は入力値、設定、初期状態、現在のガード、遷移の範囲と不変条件を実行前に確認する。Rust の `dispatch` は実装の応答も照合し、不一致でそのセッションを無効にする。[処理前の検査](tests/validation.rs)。
- [x] `rcheck validate` は要求の `derive` で指定した導出式を操作の前後で評価する。導出値は状態のスキーマから独立した組の配列として返す。[表示の照合](../../tests/model-patterns/patterns.test.ts) · [CLI の回帰試験](tests/validation.rs)
- [x] `rcheck validate` で `command` を省略すると、指定した初期状態と操作列を検証し、最後の状態と指定された導出値を返す。[初期画面の照合](../../tests/model-patterns/bootstrap.test.ts)
- [x] 指定値を型に従って読み取り、初期候補や引数候補を列挙せずに事前検査する。単項・二項関係、入れ子の構造値、操作引数、操作付き遷移条件で検査する。[処理前の検査](tests/validation.rs) · [保存キューの再生](../../tests/model-patterns/restored-queue.test.ts)。
- [x] 有限列と値付き variant を IR、状態 JSON、生成型へ渡す。検証と PBT は列の順序を維持する。[回帰テスト](tests/structured_temporal.rs)。
- [x] rtlc で `eventually` と `leads_to` を検査する。時相の反例は周期の開始点を持ち、空や予算切れは合格にしない。参照評価器と SAT は時相の問いに対応しない。[回帰テスト](tests/structured_temporal.rs)。
- [x] 参照評価器と rtlc は `transition` を検査し、処理前の validator も同じ性質を照合する。SAT は Alloy 対応範囲の前後状態を steps まで検査する。操作指定は前後状態を作る指定操作の各引数に適用する。[遷移の回帰テスト](tests/transition.rs)。
- [x] SAT は有限 atom の条件式を含む不変条件を steps まで検査する。[参照グラフの条件式](../rmodel/tests/engines.rs)。

## 照合

- [x] corpus（`fixtures/rmodel`・`spec/models`）の32 claimで、`auto`の答えがclaimと一致し、`all`の3つの答えが食い違わない（0101）。

## 非対応・対象外

- [x] 明示検査器の時相反例を、循環位置と停止ステップを保った参照コマンドに変換する。有限の引数の組ごとに付けた遷移名から、実際に選ばれた引数を復元し、全辺を照合する。明示検査器の展開上限がある。[回帰試験](tests/witness.rs)。
- [ ] 明示状態の有限違反と SAT の反例を核の値へ戻し、参照評価器で検査し直すこと。
- [ ] `auto`で、選んだ検査器が未確定のときに次の検査器へ移ること。
- [ ] SMTによる帰納法などの新しい形式化の検査器。
- [ ] 値の集合、実装の状態の一部だけを観測すること。

## 制約と根拠

- [x] 参照評価器が対応しない大きさでは、TLA+の写し方も初期状態を`SUBSET`で数え上げるので、明示状態の予算が尽きやすい。そのときは未確定と答える。

[検査器と選び方](src/lib.rs)、[登録した検査器](src/checkers.rs)、[実装の照合](src/conform.rs)、[テスト](tests/registry.rs)・[実装](tests/domain.rs)を参照。
