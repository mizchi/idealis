# 0090: ralloyの時相（Alloy 6のvar）をSATで有界に検査する

Quintを置き換えていく第2段階（[0089](0089-quint-ir-frontend.md)の次）。構造（静的な関係）と振る舞い（状態の列）を1つのモデルで扱い、SATで有界に検査する。新しいIRを別に作る前に、ralloyの関係のIRを時間方向へ広げ、Alloy 6.2.0との差分で意味論を固定する。

## 変更

- 構文: fieldの`var`、単項の`always`・`eventually`・`after`（`not`と同じ優先度）、後置のprime `'`、scopeの`N steps`。stepsを書かない時相モデルは、Alloyの既定と同じ10 steps。
- 意味論（Alloy 6の有界検査）: traceは位置0..k-1の状態と、k-1から戻るloopの位置l（0≤l<k）を持つlasso。長さk以下のlassoは、loopを展開して長さちょうどkのlassoにできるので、ちょうどkだけを探す。
  - 時相演算子のないfact・command本体は位置0で評価する。fieldの宣言（型と多重度）は全位置で成り立つ。静的なsig・fieldは全位置で共有する。
  - `after P`は次の位置（k-1の次はl）、`always P`・`eventually P`は、その位置から到達できる位置すべて・いずれか（位置iからはmin(i, l)..k-1）。
- SAT backend: `var` fieldは位置ごとに関係を持ち、loopの位置をちょうど1つの変数で選ぶ。k-1の次の状態はloopの変数で選んだ状態と一致させる。
- 時相モデルは、幅によらずSATで解く。総当たりは扱わない（`supported`でないモデル、`div`・`rem`を含むモデルは明示的にエラー）。
- witnessの独立した検査: `Instance`に、var fieldの位置ごとの値とloopの位置を加える（静的なモデルでは出力しない）。評価器を時相に広げ、SATのinstanceを`check_instance`で検査し直す。

## 正しさ（結果を見る前に固定）

- 入力: `tests/alloy/temporal`の5モデル・29 command（lights・growth・mutex・token・counter）。Alloy 6.2.0（SAT4J）の答えを`just ralloy-oracle-record temporal`で記録し、`cargo test`で再生する。
- 29 commandすべてで、解の有無がAlloyと一致すること。既存の5 group・178 commandも一致したままであること。
- SATが出したtraceのinstanceは、すべて`check_instance`でrunは`Matched`、checkは`Mismatch`（Property）になること。
- 人工故障: k-1からloopへ戻る遷移を無視する（traceを有限にする）、`always`を位置iより前のloopの部分で評価しない、fieldの宣言を位置0だけにする、の3件を差分テストが検出すること。

## 測定（判定には使わない）

- 29 commandの解く時間とSATの変数・節の数を報告する。速度の採用基準は設けない。

## 範囲外

- var sig、`until`・`releases`・`;`・過去の演算子（`before`・`once`・`historically`）、`1..N steps`の下限、無限の（unbounded）検査。
- TSのcontract（`packages/idealis/src/alloy.ts`）でのtraceの受け渡し。
