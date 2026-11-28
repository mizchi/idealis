# rprob の対応範囲

ProB の集合・関係モデルに着想を得た試作。入力は rmodel の構文。

- [x] 型検査済みの rmodel の有限 atom、列挙、和 sort、集合、二項関係をビット列に落とす。和・差・積集合、直積、関係の join と `++`（行の置き換え）、等値・包含、多重度、要素数の比較、有限量化を扱う。単項集合・二項関係の `unchanged(#式)` と位相を指定した要素数同士の比較も検査する。
- [x] `reachableFrom` は根のビット集合から新しい到達先だけを繰り返し展開する。固定行列と疎な関係の両方で参照評価器と照合した。[差分テスト](tests/reference.rs)。
- [x] `select` は有限 atom ごとの述語を評価して集合を作る。状態に依存する選択を参照評価器と照合した。[差分テスト](tests/reference.rs)
- [x] `withoutKeys` は固定・疎の両表現で既存の行のビットを落とす。可変のキー集合を使うモデルを参照評価器と照合した。[差分テスト](tests/reference.rs)
- [x] `closure` は有限 atom の二項関係を正の長さの経路で閉じる。固定行列と疎な関係を参照評価器と照合した。疎な関係では閉包で生じうる順序対も64組の上限に含める。[差分テスト](tests/reference.rs)
- [x] 有限 atom の二項関係の `keys` を固定行列と疎な関係の両方で検査する。[参照評価器との照合](tests/reference.rs)。
- [x] 構成の事実と初期条件、ガードと並行代入を評価する。対応する集合・関係の式で条件式を使える。不変条件、到達可能性、前後の状態を使う遷移性質と、`transition ... on 操作` による引数付き検査、宣言違反、操作の到達を探索する。
- [x] 空、予算切れ、完了、最短の違反・到達深さを区別する。予算は参照評価器と同じく構成、初期候補、到達状態を数える。予算切れは共通の探索レポートの `stop` にも記録する。対応外の入力にはエラーを返す。
- [x] 通常の CLI 検査・安全性検査・経路探索では、候補列挙に先立って `fact` が固定する設定値と `init` が固定する状態値を束縛する。認識する制約は無条件の等式と空集合。残る条件と多重度を検査し、候補が空の場合と予算切れを区別する。[参照評価器との差分試験](tests/reference.rs)
- [x] ビットで符号化する型の合計は64 atomまで。8 atom 以下では固定 8×8 行列を使う。それより大きいモデルでは、宣言と二項式が使いうる順序対をモデル全体で共有し、その合計が64組までなら検査できる。set / some の候補ドメインは20 tupleまで。one / lone は単項で atom、二項で各行の合法な選択を列挙する。操作の引数は最大128個で、引数・量化の展開にも別の上限がある。
- [x] `--safety` は時相の問いを含むモデルの状態・遷移と到達性を検査する。対象外の時相の問いは名前付きで報告する。公平性で探索を絞らない。[回復のモデル](../../examples/model-patterns/crash-recovery/transaction.ideal)。
- [x] 有限レコードをフィールドの直積として符号化する。入れ子の値、単項スロットへの計算付き保存、二項関係のレコード値の射影、経路からのレコード復元を扱う。ビットで符号化する場合は全レコード値を64 atomに数える。[固定・疎表現と ABA の照合](tests/reference.rs)
- [x] `data ... where` の有効な有限値を引数と初期候補に使う。ビット表現、番号で持つ値と二項関係の行、値型の入れ子を扱う。スコープ内で不正な値の保存は宣言違反になり、経路に試みた値を残す。[適用範囲の回帰試験](../idealis-check/tests/data_types.rs)
- [x] 計算したレコードを二項関係の行へ保存できる。更新値の直積、右側の直積を使う和・上書きと条件式に対応する。空キーの直積は空のまま、保存されるフィールドの値域逸脱は `Scope` になる。[オフライン編集と行の差分テスト](tests/reference.rs)
- [x] 有限の値付き variant をタグとペイロードの組ごとに符号化する。タグ判定 `is`、`payload`、レコードとの入れ子、計算したペイロードの保存、二項関係の行への保存と経路の復元に対応する。各ケースの値を全型共通の64 atomに数える。[注文の実装照合](../../tests/model-patterns/checkout.test.ts) · [復旧と入れ子の差分テスト](tests/reference.rs)
- [x] 有限列を長さと要素順で符号化する。列リテラル、`append`、`tail`、`take`、`removeAt`、`removeAll`、`at`、`len`、`elements` と、列演算の結果を直接比較する状態・遷移条件を扱う。途中の最大長超過と保存時の `Scope` を区別する。全値域を符号化する場合は空列を含む候補の総数を全型共通の64 atomに数える。[配送とストリームの差分テスト](tests/reference.rs) · [MessageChannel の照合](../../tests/model-patterns/delivery-ticket-drain.test.ts)
- [x] 全値域が64 atomの枠を超える場合、列の格納先が初期リテラルを持つ単一値の状態変数に限られ、更新がリテラルまたはその状態変数のコピーを選ぶときは、モデル中の列リテラルだけを符号化できる。列の構築演算を使うモデル、列を引数や他の値型に使うモデル、初期値が自由なモデルはこの縮小の対象外。[参照との差分](tests/reference.rs) · [Node.js の再生](../../tests/model-patterns/nested-io-batch.test.ts)
- [x] 64 atomに収まらないレコードと列は、通常検査で `one` / `lone` の単項スロットに有限域の番号で格納できる。構築・コピー・等値・所属・空判定を扱い、自由な初期値を表の実体化なしに列挙する。番号で表した単一のレコードのフィールドを直接読める。フィールドが入れ子の列でも扱い、数値フィールドは範囲を検査する保存先へ渡せる。比較用の `Compiled::new` はビットによる全域の符号化を使う。[差分試験](tests/reference.rs) · [SQLite の復旧](../../tests/model-patterns/restored-queue.test.ts)
- [x] 番号で表した構造値を操作の引数にできる。操作本体を一度だけ変換し、引数の組を宣言順で一件ずつ評価する。引数に依存しない先頭の条件が偽なら試行を省く。有限の経路探索は `--max-arguments` を受け付け、未確定と予算内に得た反例を区別する。時相の問い自体の束縛に使う番号付きの型は対象外。[参照との差分](tests/reference.rs) · [SQLite での補充](../../tests/model-patterns/refill-queue.test.ts)
- [x] 番号で表した列にも `append`・`tail`・`take`・`removeAt`・`removeAll`・`at`・`len` を適用する。`elements` はビットと番号の両方で使える。最大長を超える一時値の比較と、保存時の `Scope` 違反を区別する。[列の回転と順序の照合](../../tests/model-patterns/rotate-queue.test.ts)
- [x] 番号で表す値の一時集合では、`elements`、所属・包含・等値、和・差・積集合、多重度、要素数を扱う。`removeAll` に渡すと含まれる値をすべて除く。構造値の集合を状態スロットに直接置く宣言は rmodel の型検査の対象外。[バッチの適用記録](../../examples/model-patterns/crash-recovery/deduplicate-batch.ideal) · [差分試験](tests/reference.rs)
- [x] 二項関係 `Key -> one/lone Value` の大きな構造値をキー順の混合基数でまとめる。全行が1ワードに収まる場合に、直積、上書き、`withoutKeys`、差・積集合、照合、単一キーの値と複数キーの一時集合を扱う。同じキーに異なる値を含みうる一般の和は対象外。[ワーカーのチェックポイント](../../examples/model-patterns/crash-recovery/worker-checkpoints.ideal) · [差分試験](tests/reference.rs)
- [x] `temporal_witness` は `eventually` と `leads_to` の公平な循環を検査する。引数付きの性質、モデル既定と個別指定の公平性、弱・強、操作全体と引数ごとの区別を扱う。空・成立・循環による違反・未確定を区別する。idealis-check は得られた循環を参照評価器で再生し、通常の有限探索はグラフを保存しない。[有限グラフのテスト](tests/temporal.rs) · [rtlc との照合](../idealis-check/tests/capabilities.rs)
- [ ] 計算した整数や値の構築を含む一般の二項関係式、範囲外の要素を含む列の演算、最大長を超えうる列をさらに別の値に組み込む一般の式。
- [x] `rprob::witness` は選んだ状態・遷移・宣言の問いの構成、初期状態、操作と引数、終状態を返す。完了前の経路と全探索での最短経路を区別する。通常の `Compiled::analyze` は親経路を保持しない。[固定・疎表現の再生試験](tests/witness.rs)
- [x] `rprob::safety_support` と `rprob::witness_support` は探索前に有限の問いの変換を試す。`idealis-check --checker rprob` と `--checker all` に判定を返し、`capabilities` で対応範囲を調べられる。[idealis-check の回帰試験](../idealis-check/tests/capabilities.rs)
- [x] 単項の `one Int` と `one Nat` は、指定範囲の整数を既存のビット表現に載せる。整数は全 sort 共通の64個の上限に含める。数値引数と量化、加減乗算、比較、整数の選択式を扱う。演算途中の値と保存先の値域を区別し、保存で値域を出ると `scope` に深さを返し、証拠のない性質を未確定にする。[ログ再開と符号付き整数の照合](tests/reference.rs)
- [x] `Key -> one Int`、`Key -> lone Int` と Nat の行を既存の二項関係として探索する。行から読んだ一値を算術に使え、計算した値を直接の直積・和・上書きで保存できる。式の比較では空の行や複数行から得た集合を扱う。保存先の値域を外れる場合と必須の行が欠ける場合を区別する。[保持境界と符号付きの行の照合](tests/reference.rs)
- [x] 導出した関係への join を、型検査が一値と確認した整数添字や算術に使える。[名前付きセーブポイントの照合](tests/reference.rs)
- [x] 単項の `set`・`some`・`lone` の Int と Nat を扱う。整数の和・差・積集合、条件式、比較と要素数では、途中で範囲を出た整数も区別する。保存する前にその整数を取り除けば探索を続けられる。[再訪・自動保存・途中の集合の照合](tests/reference.rs)
- [x] `rprob::witness` の `Scope` は、rprob が見つけた操作列と逸脱時の終状態を返す。終状態は選択済みの経路だけを参照ランタイムで実行して復元する。[idealis-check の回帰試験](../idealis-check/tests/witness.rs)
- [ ] ProB の入力形式と制約ソルバー、Event-B の refinement 証明。

[カーネル](src/lib.rs) · [lowering](src/compile.rs) · [差分テスト](tests/reference.rs)
