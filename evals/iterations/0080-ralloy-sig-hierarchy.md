# 0080: ralloyのsig階層

[plan](../plans/0080-ralloy-sig-hierarchy.md)のとおり、`abstract sig`、`extends`、親が1つのsubset sig（`in`）、`sig A, B {}`、signature factを加えた。group `hierarchy`は8モデル・40 commandすべてでAlloyと一致した。速度は0080と追記の0080aの両方で`core-cardinality-formulas`だけが閾値を超え、`hold`。機能は採用し、後退は記録する。

## 実装

- atom: トップレベルのsigごとに連続した区間を持ち、`extends`の子はその中の部分区間を持つ。abstractの親で子がある場合、最後の子が残りをすべて取る。subset sigは親のatomの全部分集合を列挙する。atom名は最も具体的なsigの名前と番号。
- signature fact: `all this: A | 式`とし、Aと`extends`の祖先のfield名を`this.field`へ書き換える。量化子で同名を束縛した内側は書き換えない。
- `check_instance`: トップレベルのsigがatomを持ち、ほかのsigは親のatomを参照する。兄弟の重なりや、abstractの親の子以外のatomは`declaration`の違反。
- sigの上限を8から16に上げた。

## Alloyの答えから決めたscopeの規則

記録したAlloyの答えに合わせて、scopeの規則を2つ決めた。

- 子に明示したscopeは、親の上限を超えない。`for 2 but 3 Cat`でCatを3個にすることはできず（`ChildScope`は解なし）、`for 1`で`Student extends Person`を2個にすることもできない（`ChildFillsParent`は解なし）。
- `one sig`や`exactly N`の子は、親の上限を広げる。`abstract sig Color`と3つの`one sig`の子を`for 1`で解くと、Alloyは解を返した（`BulbsAndLamps`）。最初の実装はこれを`for 1`で詰めて解なしにし、再生テストで不一致になった。子から親へ向かう処理を足して直した。

`for`のないcommandの既定値（各sig 3個、Int 4 bit）は以前から実装済みで、`default-scope.als`でAlloyと一致した。

`sig-facts.als`は、最初にfield名を`next`にしていた。Alloyがこれを`util/integer`の`next`と曖昧だとして拒否したため、`succ`に変えて記録し直した。答えを見て変えたのではなく、Alloyで成り立たないモデルを直したもの。

## 正しさ

| group       | モデル / command | 一致 | 残り                          |
| ----------- | ---------------- | ---- | ----------------------------- |
| `core`      | 13 / 50          | 50   | なし                          |
| `nary`      | 6 / 30           | 29   | 予算切れ1（0079から変わらず） |
| `hierarchy` | 8 / 40           | 40   | なし                          |

人工故障4件のうち3件を検出した。

| 故障                                        | 結果       |
| ------------------------------------------- | ---------- |
| abstractの親に子以外のatomを許す            | 検出       |
| 親のatom集合に子孫を含めない                | 検出       |
| signature factの`this.`への書き換えをしない | 検出       |
| 型検査で子の明示scopeを親の上限で抑えない   | 検出されず |

planの「兄弟のatom区間を重ねる」故障は、そのまま入れると範囲外の計算で探索が暴走しうるため、包含関係の故障（親の集合に子孫を含めない）に置き換えた。最後の1件は、探索の側でも子の数を親の残りで抑えるため、解析の結果としては観測できない等価な故障だった。型検査の抑え込みは`check_instance`のscope検査にだけ効く。

Rust・TS・Chromium・`just check`・`wasm-test`・`package`・`evaluate`・`replay-evaluation`が通った。`semantics`とevalの対照から、未対応として拒否する例の`extends`と`in`を外した。記録したAlloyの答えのJSONは、記録器の出力のまま保つため`vp fmt`の対象から外した。

## 速度

| plan                                                 | 変更                    | 判定   | 超えたケース                            |
| ---------------------------------------------------- | ----------------------- | ------ | --------------------------------------- |
| [0080](../runs/0080-ralloy-comparison/result.json)   | 階層の実装（e688dc40f） | `hold` | `core-cardinality-formulas` 1.106/1.057 |
| [0080a](../runs/0080a-ralloy-comparison/result.json) | 親と子の番号を事前計算  | `hold` | `core-cardinality-formulas` 1.085/1.108 |

baselineはどちらも0079のcandidate（dcaa48067）。0080aでは、ほかの12ケースは0.977〜1.082だった。`core-cardinality-formulas`は6 commandで約3.5 µs、1 commandあたり約0.6 µsで、解析ごとの固定費（`analyze`での階層の検証と表の作成）が見える大きさになる。0080aのplanに従い、これ以上は再測定しない。固定費は、0083でModelに前計算を持たせる候補とする。負荷平均は約4で、専用ホストではない。
