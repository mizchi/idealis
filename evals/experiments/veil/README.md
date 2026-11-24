# Veilの意味論の抽出

[Veil](https://veil.dev/)（verse-lab、Lean 4の上の状態遷移系の検証framework、CAV 2025）を試し、idealisの核の意味論（[docs/kernel-semantics.md](../../../docs/kernel-semantics.md)）との方向の一致と差を確かめた。採用の判断ではなく、意味論の抽出に限る。

- 対象: verse-lab/veil `5a525eeca8cee37b2da8c010552db9b1aae968c2`、Lean `v4.35.0`。`lake build`の後、`lake lean <file>`で実行した（`lake env lean`ではcvc5のnative libraryが読まれない）。
- 入力: [Permissions.lean](Permissions.lean)（`fixtures/rmodel/permissions.model`を手で写したもの）、[PermissionsFault.lean](PermissionsFault.lean)（`read`の付与の確認を外した人工故障）。出力は[evals/runs/veil-probe](../../runs/veil-probe/)。

## 結果

| 検査                                             | Permissions                                           | PermissionsFault                                                                         |
| ------------------------------------------------ | ----------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `#model_check`（設定を1つ指定、`Fin 2`）         | 違反なし（2状態）                                     | `no_leak`・`only_granted`の違反、1歩のtrace                                              |
| `#check_invariants`（すべての大きさと設定、SMT） | 初期状態と`read`・`close`のすべてで保たれる（帰納的） | `read`で`no_leak`・`only_granted`が破れ、帰納法の反例（CTI）をwpと遷移関係の両方から出す |
| `sat trace`（1歩で秘密の文書が開く）             | 見つかる                                              | 見つかる                                                                                 |

idealisの参照評価器はこのモデルで`NoLeak`・`OnlyGranted`が成り立つと答える（scopeの中で完全）。Veilは同じ性質を、大きさと設定を限らない帰納的な不変条件として示した（SMTの答えを信頼、証明の再構成はしていない）。

## 抽出した意味論

### 遷移系（`Veil/Core/Tools/ModelChecker/TransitionSystem.lean`）

`RelationalTransitionSystem ρ σ l`: 背景の理論（不変の状態）`ρ`、状態`σ`、遷移のラベル`l`（actionの名前と引数）と、

- `assumptions : ρ → Prop`（許される理論）
- `init : ρ → σ → Prop`（理論ごとの初期状態）
- `tr : ρ → σ → l → σ → Prop`（遷移関係）

到達可能性は理論ごとに定義される（`reachable sys th s`）。

idealisとの対応: `rel` = `ρ`、`fact` = `assumptions`、`var` = `σ`、`init` = `init`、action instance = ラベル。**核の形は一致する。**

### 型と関係

- `type node`は解釈しないsort（空でなく、等号が決定可能）。検証ではすべての型について示し、モデル検査では`Fin 4`のように具体化する。scopeはモデルではなく検査の引数（idealisと同じ方針）。
- 関係はBoolへの関数（`relation pending : node → node → Bool`）。実行できるようにPropではなくBool。`immutable`（理論）・可変・`ghost`（導出）。多重度の宣言はない（不変条件で書く）。
- 量化はIvyの規約で、大文字の変数は暗黙に全称（`leader N := false`）。

### action（`Veil/Frontend/DSL/Action/Elaborators.lean`）

- actionは1つの命令型のプログラム（`require`、関係の更新、非決定の代入`:= *`、`let x :| P`、`if`、`let mut`）。
- **そこから2つの意味を導く**: 実行の意味（モデル検査とシミュレーション）と、最弱事前条件（wp、Loomのmonad algebra）。遷移関係はwpから作る。写し方を別々に書かない。
- 実行の結果は、成功（次の状態）・assertionの失敗（assertionのidと、失敗した時点の状態）・発散の3つ。`require`が成り立たないのは遷移がないことで、失敗ではない。帰納法の検査は、各actionが例外を出さないこと（`doesNotThrow`）も確かめる。
- 行き止まりの状態（成功する遷移がなく、`terminating`でない）は、モデル検査で違反として報告する。

### 性質と検査

- `safety`と`invariant`（帰納法の補助）: `#check_invariants`は「初期状態が不変条件を満たす」「不変条件と各actionの遷移から、次の状態でも満たす」を、actionと不変条件の組ごとにSMT（cvc5）で検査し、破れれば反例（CTI）を出す。
- `sat trace`・`unsat trace`（`any N actions`と`assert`）: すべての大きさと理論についての有界の記号的な検査。
- `#model_check`は、理論を1つ具体的に与えて、到達できる状態を幅優先で全列挙する。
- 時相の性質（Lentil、TLA+をLeanに埋め込んだもの）: 無限のbehaviorの上のLTL。演算子は`□ ◇ ◯ 𝑈 𝑅 ↝`、fairnessはTLA+と同じ定義（`𝒲ℱ A ≝ ◇□Enabled A → □◇⟨A⟩`、`𝒮ℱ A ≝ □◇Enabled A → □◇⟨A⟩`）。検査はLasso（fairなlassoの空判定、SCCごと）、証明は時相のtactic。

## 方向の確認

| 論点                         | Veil                                        | idealis                                                   | 判断の材料                                                         |
| ---------------------------- | ------------------------------------------- | --------------------------------------------------------- | ------------------------------------------------------------------ |
| 核の形（理論・状態・ラベル） | 同じ                                        | 同じ                                                      | 一致                                                               |
| scope                        | 検査の引数                                  | 検査の引数                                                | 一致                                                               |
| モデル検査の設定             | 1つ与える                                   | factを満たすすべてを列挙                                  | idealisの「設定を1つに固定する問い」（未決）にVeilの形が答えになる |
| actionの意味の源             | 1つのプログラムから実行とwpを導く           | 参照評価器と、2つの写し方を別々に書き、差分テストで照らす | Veilの方が強い保証。idealisは差分テストで補っている                |
| 失敗の扱い                   | assertionの失敗（id付き）・発散             | 多重度の違反（`Declarations`）・範囲の外（`Scope`）       | 「actionが失敗した状態」として1つの概念にまとめられる              |
| 範囲を超えた答え             | 帰納法（CTI）・不変条件の合成（UPDR）・証明 | なし                                                      | idealisの次の検査器の候補                                          |
| 時相の性質                   | TLA+の意味（無限のbehavior、WF・SF）        | 未決                                                      | 互換性のため、同じ意味を採るのが自然                               |
| 実装との接続                 | 読んだ範囲では見当たらない                  | 4言語の型の生成と照合                                     | idealis側の独自の部分                                              |

写し方の対応（手で確かめた範囲）: `rel` → `immutable relation`、`fact` → `assumption`（状態の宣言の後）、`var` → `relation`、`init` → `after_init`、guard → `require`、代入 → 関係の更新。多重度は不変条件、`#`や整数の値はVeilのSMTの断片の外になりうるので、検査器として載せるなら`supports`で範囲を宣言する。
