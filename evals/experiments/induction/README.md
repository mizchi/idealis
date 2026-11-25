# 帰納法の検査器の試作（plan 0106）

rmodelのモデルの`invariant`と`Declarations`が、すべての大きさで帰納的かをz3で検査する試作。crateには入れていない。[plan](../../plans/0106-induction-proto.md)・[記録](../../iterations/0106-induction-proto.md)。

```sh
cargo build --release --manifest-path evals/experiments/induction/Cargo.toml
evals/experiments/induction/target/release/idealis-induction-proto fixtures/rmodel/*.model
evals/experiments/induction/target/release/idealis-induction-proto --generated 9400 9699
```

- `src/vc.rs`: VC。sigは解釈しないsort、enumはdatatype、関係はBoolの関数、式は「tupleが式に属する」一階の式。
- `src/smt.rs`: z3（`z3 -in`、20秒）と、sortの大きさを小さい順に固定して取り出す具体的なCTI。
- `src/recheck.rs`: CTIを、sigをenumにしたモデルに書き直して参照評価器にかける。
- 人工故障（`INDUCTION_MUTANT=post-multiplicity`・`frame`）は試作の中だけの切り替え。
