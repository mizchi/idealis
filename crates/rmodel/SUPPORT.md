# rmodelの対応チェックリスト

チェック済みは、その項目に記載した検査経路で確認した範囲。構文・型検査、参照評価器、各検査器の対応範囲は個別に記載する。未チェックは未対応または対象外。

## 対応済み

- [x] アダプターは `Model::value_types_in_dependency_order` でレコード・列・variant の宣言を値型の依存順に取得できる。[有限値の lowering](../rprob/src/compile.rs)

- [x] `sig`（scopeでちょうどの大きさ、1〜7）、静的な`rel`と状態の`var`。単項（`set`・`lone`・`one`・`some`）と二項（`A -> m B`）の関係（0091）。
- [x] `entity Account "説明"` は個体を持つ sort と任意の1行の用語定義を宣言する。型と意味は `sig` と共通。定義は Rust・TypeScript 型と用語集に反映する。[構文の照合テスト](tests/entity.rs)。
- [x] `enum Name = A | B`: 名前の付いた、ちょうど1つずつのatom。scopeを書かず、sigと同じく量化と関係の型に使う（0093）。
- [x] `fact`（varを参照しない設定の制約）、`init`、引数付きの`action`。actionは連言で、`v' = 式`の代入は1つのvarに1回まで、右辺と条件にprimeを書けない。代入しないvarと`rel`は変わらない（0091）。
- [x] TLA+ 変換では、`fact` の静的関係と `init` の状態変数について、無条件の等式や空集合条件から初期値を直接束縛する。値の領域・多重度と元の条件も検査する。関係に依存する値や論理和の制約では従来の列挙を使う。
- [x] `behavior 名前(引数) { guard { 条件 } update { v = 式 } }`。各ブロックは省略でき、両方を書く場合はguardが先。従来のactionと同じ型検査と同時更新を使う。[構文の照合テスト](tests/behavior.rs)。
- [x] `init`、`guard`、`invariant` などの条件ブロックでは、条件をカンマで区切れる。括弧で始まる次の条件を、直前の名前への呼び出しと区別する。[構文の照合テスト](tests/behavior.rs)。
- [x] `update` 内で `if { 条件 } then { 更新群 } else { 更新群 }` を使える。選ばれた枝にない代入は遷移前の値を保持する。入れ子と `else` の省略に対応する。[構文の照合テスト](tests/behavior.rs)。
- [x] 条件付き更新群と、代入の右辺で直接連なる条件式のソース位置・入れ子を保持する。参照探索は型付きモデルの更新と同じ条件で各枝への経路を取得する。[CLI の回帰試験](../rcheck/tests/branches.rs)
- [x] 診断で追える分岐の腕には `expect unreachable "理由"` を置ける。理由は位置情報として保持し、評価する式には含めない。[構文の回帰試験](tests/behavior.rs) · [到達性の回帰試験](../rcheck/tests/coverage.rs)
- [x] `update` の先頭の `let name = 式` は更新前の式を束縛し、後続の束縛と複数の代入から参照できる。型検査で式を展開する。[編集の再送](../../examples/model-patterns/offline-merge/retry.model)
- [x] モデル直下の `derive 名 = 式` は、先に宣言した導出式とモデルの設定・状態を参照できる。型検査中に展開し、独立した構成候補や状態変数を作らない。遷移の `before` と `after` では参照箇所の位相で評価する。[スナップショットの復旧](../../examples/model-patterns/subscriptions/partition-repair.model) · [型検査の試験](tests/behavior.rs)
- [x] `derive` と `let` の展開では、選択式と量化式の束縛を参照先の名前と区別する。[IR の回帰試験](../rcheck/tests/ir.rs)
- [x] 参照評価器は設定と状態のスナップショットから名前を指定して導出値を評価できる。[認証付き履歴](../../examples/model-patterns/auth-history/history.model) · [rcheck の回帰試験](../rcheck/tests/validation.rs)
- [x] 値の条件式 `if { 条件 } then 式 else 式`。両枝の型を検査し、条件と選ばれた枝を同じ状態で評価する。参照評価器と TLA+ で式として扱う。有限 atom の式は、等式や到達集合の中の条件式も Alloy に変換する。空関係の項数と要素数の既存の上限を保つ。[検査器間の照合](tests/engines.rs)。
- [x] `bool { 条件 }` は条件ブロックを Bool 値として使う。同じ条件に `then true else false` を指定した式と生成 IR が一致する。[IR の回帰試験](../rcheck/tests/ir.rs)
- [x] 式: join（単項と二項、二項同士）、`->`、`+`・`-`・`&`、`none`、単項と二項の`#`と絶対値7以下のリテラル。論理式: `in`・`not in`・`=`・`!=`・多重度・`no`・`not`・`and`・`or`・`implies`・`all`・`some`（0091）。
- [x] `P iff Q` は二方向の含意の論理積に展開する。`implies` より弱く結合し、連鎖には括弧が要る。[配信競合](../../examples/model-patterns/subscriptions/delivery-race.model)
- [x] 二項関係の `#` は異なる順序対の個数を数える。宣言された整数の保存範囲は従来どおり検査する。Alloy 変換は個数の上界に応じて整数幅を選び、上限15を超えうる場合を対応外とする。[照合テスト](tests/engines.rs)
- [x] `reachableFrom(roots, edges)` は、両端の atom sort が同じか一方が他方に含まれる関係で、根を含む到達集合を求める。結果の型は根と辺の終点の両方を含む。参照評価器・TLA+・Alloy に変換する。[式を使ったグラフの例](../../examples/model-patterns/worker-transfer/reachability-expression.model)。
- [x] `closure(edges)` は、両端の atom sort が同じか一方が他方に含まれる関係で、1辺以上の経路がある順序対を返す。辺の始点・終点の sort を保持する。空の辺では空関係を返し、自分自身への順序対は循環がある場合に含む。参照評価器・TLA+・Alloy で照合した。[検査器の照合](tests/engines.rs)
- [x] `keys(relation)` は二項関係に現れる左側の値を重複なく求める。参照評価器と TLA+ でレコードへの関係を扱い、有限 atom の関係は Alloy にも写す。[検査器の照合](tests/engines.rs)。
- [x] 二項関係の `left ++ right` は、右辺に現れるキーの行をすべて右辺の値で置き換える。右辺が空なら左辺を保つ。参照評価器・TLA+・Alloy で照合した。[関係の置き換え](../../fixtures/rmodel/relations/override.model)。
- [x] `&` の両辺で対応する sort に包含関係があるとき、結果は狭い sort を採る。二項関係では左右の列ごとに判定する。[IR の回帰試験](../rcheck/tests/ir.rs)
- [x] `invariant`（到達できる全状態）と`reachable`（到達できる状態がある）。関係の型と多重度を検査し、違反は組み込みの `Declarations` で報告する（0091）。
- [x] Alloyへの写し方（ralloyのSATで有界、stepsのlasso）とTLA+への写し方（rtlcで完全、`rel`はInitで列挙）。

- [x] 参照評価器（`reference::evaluate`）: scopeの中を全列挙して、意味論の正本として判定を返す。空・成り立つ・破れる・到達できる・到達できない・未確定を区別し、破れる・到達できるには最短の深さを付ける（0097）。
- [x] 停止状態の探索では、ガードの有効性と、合法な状態変化の有無を選んで検査できる。状態変化を調べるモードでも、スコープ外や宣言違反があれば診断を残す。[バリアの回帰試験](../rcheck/tests/deadlock.rs)
- [x] 参照評価器と操作の再生は、初期の候補列挙上限を超える関係でも、`fact` または `init` が閉じた式で値を固定していれば値を直接束縛する。値の領域・多重度と元の条件を検査する。操作の再生では上限に達する前でも固定値を優先する。[原本の切り替え](../../examples/model-patterns/lease/resync-source.model)・[回帰試験](../rcheck/tests/witness.rs)。

- [x] sortの静的な型検査（集合と二項関係のsort、join・比較・代入のsortの一致）。多重度を破る遷移は組み込みのcheck `Declarations`で違反として報告し、違反した状態からは遷移しない（0099）。

- [x] 和のsort `type Party = Person + Org`: sig・enumをまとめた、自分のatomを持たないsort。和・比較・joinは部分型の関係で型を付け、Alloyでは`abstract sig`と`extends`、TLA+では子の集合の和に写す。1つのsigが属する和は1つまで、和の入れ子はない（0100）。

- [x] 値（0103）: `Int`（scopeの`Int = lo..hi`、範囲の外は組み込みのcheck `Scope`）、`Bool`、`record`（構造で等しい値、literal `R { f: e }`、field `x.f`。左辺の型でフィールドを解決する。別のレコード型や操作引数でも同じ名前を使える）。構造値や数値を持つ二項関係は`one`・`lone`、算術`+ - *`と整数の比較は値が1つに決まる式だけ。`some` による決定は後続の連言と含意の結論に適用し、更新前後は独立に確認する。TLA+へ写し、Alloyへは写さない（`alloy_supports`）。

- [x] `some e` の成立を確認した後、`lone` の単項変数、単一キーの `lone` 行、列の `at` で読んだ値を単一値として使える。根拠は後続のガードと更新、または条件式の成立側で有効。[回帰試験](tests/behavior.rs) · [再同期モデル](../../examples/model-patterns/lease/resync-order.model)。
- [x] `one e` の成立を確認した後、導出した関係から読んだ値も一値として添字や算術に使える。[回帰試験](tests/values.rs) · [名前付きセーブポイント](../../examples/model-patterns/sqlite-transactions/savepoint-named.model)
- [x] ソースの数値比較で型や個数が不正なオペランドの位置を報告する。高々一値の場合は先に `some` で存在を確認するよう案内する。[CLI の回帰試験](../rcheck/tests/numeric_diagnostics.rs)

- [x] `Nat` は `Int` の部分型で、`scope Int = lo..hi` の0以上の値を使う。引き算の値は整数で、負の値を Nat の変数や record フィールドに保存しようとすると `Scope` の違反になる。
- [x] 論理式の `all` と `some` は有限 `Int` と `Nat` を束縛できる。参照評価器と TLA+ で `scope Int` の値域を使う。SAT の変換 の有界整数には写さない。[IR の回帰試験](../rcheck/tests/ir.rs)
- [x] `select(atom: Sort | predicate)` は有限 atom の条件に合う集合を返す。参照評価器と TLA+ 変換で遷移前の選択を照合した。組み込みの ralloy は集合内包を対応外とする。[一括削除](../../examples/model-patterns/offline-merge/batch-retention.model) · [回帰試験](../rcheck/tests/ir.rs)
- [x] `withoutKeys(relation, keys)` は二項関係から左側のキーで行を取り除く。同じ値を持つ別のキーは残る。関係に存在しないキーには削除対象がない。[回帰試験](../rcheck/tests/ir.rs)
- [x] 単項の `set Int`・`set Nat`・`some Int`・`some Nat` は、有限スコープ内の整数を複数保持できる。`union`・`difference` は集合演算を明示し、`1 in pending` のようにリテラルで所属を調べられる。[再訪モデル](../../examples/model-patterns/browser/navigation-activation.model) · [IR の回帰試験](../rcheck/tests/ir.rs)

- [x] 名前付きの有限列と値付き variant。列は最大長1〜4。`tail(列)` は先頭を除き、空列には空列を返す。`take(列, 件数)` は先頭から指定件数を取り、件数を0から列の長さまでに収める。`removeAt(列, 添字)` は0始まりの指定位置を取り除き、範囲外では列を保つ。`removeAll(列, 除外集合)` は指定した値の出現をすべて取り除き、残りの順序と重複を保つ。`elements(列)` は列に出現する値を集合で返す。参照評価器と TLA+ で検査する。型検査では有限の値域の大きさを計算する。参照評価器は固定した初期値と更新を構造で検査し、候補列挙が要るときだけ値域を展開する。参照探索の単項 `one`・`lone` の自由な初期値は、候補を一件ずつ生成して状態予算に計上する。参照探索の操作引数も一件ずつ生成して予算に計上する。引数に依存しない先頭のガードで拒否した操作は、その状態の候補を生成しない。一括の状態生成、引数一覧の出力、TLA+ 変換には個別の上限がある。variant の各ケースは値なしまたは1値で、値型の再帰は拒否する。[有限列の例](../../fixtures/rcheck/structured/ordered-log.model) · [決済の例](../../fixtures/rcheck/structured/payment.model)。
- [x] `eventually`、量化できる `leads_to`、操作に対する弱／強公平性を TLA+ に写す。問いごとに公平性を上書きできる。[配信の評価](../../evals/iterations/0116-structured-temporal.md)。

- [x] `weak serve(*)` と `strong serve(*)` は操作の各引数の組に公平性を課す。引数を書かない `weak serve` と `strong serve` は操作全体に適用する。rtlc と TLC で照合した。[ロックの公平性](../../examples/model-patterns/worker-fairness/competitive.model)。
- [x] 参照評価器は再生した循環経路の閉路・進行性・公平性を調べる。外部の `eventually` と `leads_to` の反例を検査し、予算切れを区別する。[外部検査器の回帰試験](../rcheck/tests/external_checker.rs)

- [x] `transition` は実行可能な各操作の前後を比較する。式の `before` と `after`、対照のプライムを型検査し、参照評価器と TLA+ で検査する。[出荷の例](../../fixtures/rcheck/structured/shipped-transition.model)。
- [x] `transition 名前 on 操作 { ... }` は指定した操作の合法な呼び出しに適用する。操作の引数を前後で同じ値として参照できる。参照評価器、TLA+、rprob、rcheck の操作前検査で扱う。Alloy の対象範囲では各引数の遷移を steps まで検査する。[再送の例](../../examples/model-patterns/retry/delivery.model)。
- [x] `transition` の先頭で `let` に式を束縛できる。明示した前後状態は型検査時の展開で保持する。束縛名は位相指定の外から参照し、`ticket.after(claimed)` のように前後の値を結合できる。[リングのスロット再利用](../../examples/model-patterns/worker-deque/ring-reuse.model)。
- [x] `transition` 内の `unchanged(式)` を前後の等値比較へ展開する。引数は位相指定を含まない状態式。[構文の照合テスト](tests/behavior.rs)。

- [x] 実装のためのbindings（`bindings`、0102）: JSONの形（atomは名前、Boolはboolean、recordはobject、二項関係は定義域のatomを鍵にしたobject）、JSON Schema（draft 2020-12）、Rustのtraitと型、TypeScriptの型とJSON Linesのdriver、Goのinterfaceと厳密なdecoder・`Serve`（0104、出力はgofmtの書式）、MoonBitのtraitと厳密なdecoder・`Server::handle`（0105、targetに依存しない）。参照評価器を1歩ずつ進める`reference::Machine`。 コマンド JSON では `kind` 引数を `$kind` に写し、操作タグの `kind` と区別する。[契約](src/bindings.rs)。

- [x] `validate`（0108）: parserを通らない構文木（核のIRのJSONなど）を、parserと同じ型検査と大きさの上限で検査する。Boolは名前が現れたときに足す。

## 照合

- [x] corpusの5モデル・20 checkで、ralloyのSAT・rtlc・Alloy 6.2.0・TLC 1.8.0の判定が、結果の前に書いた期待とすべて一致する。明示状態の深さはstepsの内側（0091）。

[評価0091](../../evals/iterations/0091-rmodel-language.md)を参照。

## 非対応・対象外

- [ ] 3項以上の関係、任意の時相論理式、三状態以上を直接参照する性質、module、sig の階層。
- [ ] Pklの仕様との結びつけ（0092）、Quint IRからの写し方。
- [ ] SATの有界な結果（stepsの外）を完全な検査として扱うこと。

## 制約と根拠

- [x] 未知の名前、fact での var の参照、transition の外にある式の prime、二重の代入、arityの違い、名前の隠蔽はエラーにする。
- [x] 2つの写し方は構文木だけを共有し、互いの出力を使わない。
- [x] 列挙範囲と演算の表現範囲を分ける。参照評価器は符号付き64ビットで表現できる算術を正確に評価する。TLA+ lowering は TLC に移植できる範囲 `-2147483647..=2147483647` を使う。型検査された式の範囲を各上限と比較し、超えうるモデルや lowering を拒否する。[0115 の結果](../../evals/iterations/0115-nat-validation.md)

[構文と型](src/syntax.rs)、[写し方](src/emit.rs)、[照合テスト](tests/engines.rs)を参照。
