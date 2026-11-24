# rquintの対応チェックリスト

チェック済みは、Quint CLIのJSON IR（`quint compile --target json`）を標準のTLA+とTLCの設定へ写し、rtlcで検査できる範囲。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] 平坦化した1 moduleの`var`・`val`・`def`・`action`・型定義。`init`・`step`と名前を指定した不変条件を`INIT`・`NEXT`・`INVARIANT`にする（0089）。
- [x] 論理・整数・比較・`ite`、集合（和・積・差・`map`・`filter`・量化・`to`・`powerset`・`flatten`・`size`）、list（`nth`の0始まりの添字・`append`・`concat`・`head`・`tail`・`length`）、tuple、record、map（`mapBy`・`get`・`set`・`put`・`keys`）（0089）。
- [x] 集合の`fold`（`CHOOSE`の順）とlistの`foldl`を、LETの中の`RECURSIVE`演算子で書く（0089）。
- [x] sum typeの値を`[tag, value]`のrecordにし、`match`を`CASE`にする。`nondet`の`oneOf`は`\E`、`all`・`any`は`/\`・`\/`（0089）。
- [x] 出力は`Integers`・`Sequences`・`FiniteSets`・`TLC`だけを使い、ApalacheのmoduleやJVMを必要としない。依存はserde_jsonだけで、Wasmにもビルドできる。

- [x] `to_alloy`: 整数とunitのvariantの状態変数を持つモデルを、Alloy 6へ写してralloyのSATで有界に検査する（5 bitの整数・8 steps）。状態に依存しない式は変換時に評価し、bit幅に収まらない定数は拒否する（0095）。状態に依存する足し算・引き算・符号反転の桁あふれは`c_overflow`で検出し、反例があれば判定を未確定にする。状態に依存する掛け算は拒否する（0096）。

## 照合

- [x] 0044の6モデル・12件が、ApalacheのTLA+をTLCで検査した答え（判定と状態数）と一致する。同じ出力をTLC 1.8.0で検査しても一致する（0089）。
- [x] `to_alloy`で写せる0044の10件で、ralloyのSATとAlloy 6.2.0がTLCと一致する。SequenceFoldの2件は5 bitに収まらないので拒否する（0095）。
- [x] `Coverage.qnt`の事実が、Quint 0.33.1のsimulatorと同じ判定になる（0089）。

[評価0089](../../evals/iterations/0089-quint-ir-frontend.md)を参照。

## 非対応・対象外

- [ ] `const`・`temporal`・`run`の定義、module間のimport・instance（平坦化しないIR）、Quintのrandom simulatorとREPL。
- [ ] 上に挙げていないopcode（`always`・`eventually`・`setOfMaps`・`range`・`slice`・`select`など）。未知のopcodeは拒否する。
- [ ] SATの経路でのbool・record・map・listの状態変数、状態に依存するfold、掛け算・割り算の桁あふれの符号化。
- [ ] Quintの型検査と効果の検査。IRはQuintが検査した後のものとして読む。

## 制約と根拠

- [x] 未知の宣言・opcode・名前、`errors`を含むIR、指定した定義の不在はエラーにし、別の意味へ写さない。
- [x] TLC互換の判定は、rtlcの対応fragmentと予算の範囲に限る。

[公開境界と変換](src/lib.rs)、[照合テスト](tests/quint_generated.rs)を参照。
