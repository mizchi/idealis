# safety探索の削減option

Rustの`SafetyProgram::check_with_options`と、TSの`checkSafety`の第4引数で探索optionを指定する。既定では追加の遷移削減を行わない。native/WasmのJSON `check-safety`もoptionalな`options`を受け取り、未知のfieldと不正なworker数を拒否する。

```ts
checkSafety(core, program, bounds, { workers: 8, symmetryActions: true });
checkSafety(core, otherProgram, bounds, { partialOrder: true });
```

```rust
use rtlc::SafetyOptions;
let result = program.check_with_options(bounds, SafetyOptions {
    partial_order: true,
    ..Default::default()
})?;
```

`workers`は1〜256で、既定は1。Wasmは逐次実行する。`symmetryActions`は[限定SYMMETRY](symmetry.md)が必要で、現在状態のnode行が同じ操作を代表だけ展開する。`partialOrder`とSYMMETRYの併用は拒否する。coverage APIは従来の逐次・全展開を維持する。Rustの`Request::CheckSafety`をstruct literalで作る場合は`options: None`を追加する。既存JSONでは省略できる。

`work`は実行した経路の論理費用で、1 workerと複数workersの値が同じとは限らない。同じ`maxWork`でも停止地点の一致は保証しない。変更前後を比較するときはworker数を揃え、予算切れをcompleteへ置き換えない。

## 独立した単調なflag操作に限定したPOR

部分順序削減（POR）は、独立する操作の実行順の重複を省く。現在の実装は次の形の操作だけを候補にする。

```tla
Start == /\ ~started
         /\ started' = TRUE
         /\ UNCHANGED <<counter, otherFlag>>
```

他のbranchが`started`を読み書きせず、invariantも参照しないことをIRから確認する。単なるUNCHANGEDは依存に含めない。未知の式、次状態の参照、bindingを含む操作は保守的に対象外とする。現在状態でflagがBoolでない場合も通常探索へ戻し、評価エラーを隠さない。

有効な候補があれば、その操作を先に1つ実行する。選択するたびに未設定flagが減り、他の操作はflagを戻せない。この条件で削減した操作だけを巡る循環を避ける。他の操作やinvariantがflagを参照しないため、これらの操作を前へ移動しても観測する値は変わらない。

temporal/fairness、state/action constraint、initial/action propertyでは通常探索へ戻す。PORの指定時はnativeも逐次実行する。一般の依存解析やcycle proviso、任意のTLA+に対するPORは未実装。

states/generated/work/depthは削減後の探索の値。completeは適用条件を満たす削減に対するsafety検査の完了で、全具体状態を列挙したという意味ではない。反例は元のNextを通るが、元のグラフで最短とは限らない。boundedとemptyの扱いは通常探索と同じ。10個の独立flagの人工モデルでの測定と、依存・循環の拒否対照は[0061の評価](../evals/iterations/0061-symmetry-next.md)を参照。
