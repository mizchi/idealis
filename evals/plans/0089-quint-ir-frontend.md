# 0089: Quint IRを直接読むfrontend（rquint）

mizchiの方針は、Quintのツールチェーンを、ここで統合した道具で置き換えていくこと。今は、Quint 0.33.1がApalache 0.63.0のserver（JVM）経由で生成したTLA+を、rtlcで検査している（[0044](../iterations/0044-quint-generated.md)）。その第1段階として、Quint CLIのJSON IR（`quint compile --target json`）を直接読み、Apalacheを経由しないで検査できるようにする。

## 変更

- 新しいcrate `crates/rquint`。依存はserde/serde_jsonだけで、Wasmにもそのままビルドできる。
- Quint IRの`modules`・`declarations`・式を読み、標準のTLA+（`Integers`・`Sequences`・`FiniteSets`・`TLC`のみ）とTLCの設定（`INIT`・`NEXT`・`INVARIANT`）へ変換する。Apalacheのmodule（`ApaFoldSet`・`Variant`・`:=`）は使わない。
  - 集合の`fold`とlistの`foldl`は、LETの中の`RECURSIVE`演算子で書く。集合の要素は`CHOOSE x \in S: TRUE`の順に取り出す。ApalacheのTLC向けの`ApaFoldSet`と同じ順序。
  - sum typeの値は`[tag |-> "名前", value |-> 値]`。`match`は`CASE`。
  - `nondet x = oneOf(S)`は`\E x \in S`。`all {}`・`any {}`は`/\`・`\/`。
  - Quintの0始まりのlistの添字は、TLA+の1始まりへ写す。
- 対応しない構文（`const`、`temporal`、moduleのimport・instanceなど）は、明示的に拒否する。黙って別の意味にしない。

## 正しさ（結果を見る前に固定）

- 入力: `fixtures/quint-generated/*.qnt`の6モデルを`quint compile --target json`（Quint 0.33.1）で変換したIR。`fixtures/quint-ir/`に、Quintの版とsourceのSHA-256とともに保存する。
- 正常版の`safety`と違反版の`broken`の12件で、`rquint`が出したTLA+をidealis-coreのRust経路（rtla→rtla-compiler→rtlc）で検査する。判定（complete / invariant-violation）と状態数が`fixtures/quint-generated/tlc-oracle.json`（ApalacheのTLA+をTLCで検査した答え）と一致すること。生成数は報告する（TLA+の書き方の違いで変わりうる）。
- 同じTLA+をTLC（JVM、記録時だけ）でも検査し、判定と状態数が一致すること。
- 単体テスト: 各opcodeの変換を、TLA+の式として期待する文字列で検査する。
- 人工故障: listの添字を0始まりのまま写す、`fold`の初期値を捨てる、`matchVariant`のtagの比較を逆にする、の3件をテストが検出すること。

## 範囲外

- Quintのmodule system（import・instance・const）、temporal性質、Quintのrandom simulator。
- 6モデルより広いcorpus（Quintの公式examplesなど）は、次の段階で加える。
