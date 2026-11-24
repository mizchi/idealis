# 0083: ralloyの対称性の除去

[plan](../plans/0083-ralloy-symmetry.md)のとおり、lex-leaderによる対称性の除去を入れ、sigの階層の順序を`Model::parse`で1回だけ計算するようにした。`core-set-algebra`は128 msから34 msになった。最初の測定（0083）は小さいケース1つで`hold`になり、割り当てをなくす追記（[0083a](../plans/0083a-ralloy-symmetry-setup.md)）で`adopt`になった。0078からの累積は、報告のみの比較（[0083b](../plans/0083b-ralloy-cumulative.md)）で測った。

## 方法

実装前の探索profileでは、`core-set-algebra`の時間の98%が式の評価だった。候補を評価する前に減らせば効く。

- 式はatomを、sigとfieldを通してしか参照しない。すべてのsigへの所属が同じでInt atomでない2つのatomを入れ替えると、解は解に移る。
- 同じ所属の隣り合うatomの互換ごとに、fieldの全行を入れ替えた構造（ownerの行と、各行の中のbitの両方）と比べる。元の構造が辞書順で大きい候補は評価しない。
- 各軌道の辞書順で最小の代表は、すべての互換の条件を満たすので残る。解のある・なしは変わらない。factを満たす構造があれば代表もfactを満たすので、`no-instance`と`no-counterexample`の区別も変わらない。
- 2列以上のfieldがあるcommandでは除去しない（tuple空間の置換を扱わない）。そのため`nary`の`CompositionHolds`は予算切れのまま。

返すinstanceと、`candidates`・`factInstances`・`work`は変わる。

## 正しさ

5 group・37モデル・174 commandのうち173でAlloyと一致し、`nary`の1件は0079から変わらず予算切れの`unknown`。

| 故障                                               | 結果       |
| -------------------------------------------------- | ---------- |
| Int atomも入れ替えの対象にする                     | 検出       |
| 行の中のbitを入れ替えず、ownerの行だけを入れ替える | 検出       |
| 入れ替えの判定でsubset sigへの所属を無視する       | 検出されず |

最後の1件は、誤りではなく等価な変更だった。subset sigは探索で選ぶ変数なので、subset sigへの所属だけが違うatom同士も、問題全体では入れ替えられる。fieldの並びで最小の構造が軌道の中に必ずあり、それは解のまま残る。今の実装は必要より保守的で、subset sigを無視すれば、より強く除去できる余地がある。

Rust・TS・Chromium・`just check`・`wasm-test`・`package`・`evaluate`・`replay-evaluation`が通った。

## 速度

| 比較                                                             | baseline | 判定    | 主な結果                                                  |
| ---------------------------------------------------------------- | -------- | ------- | --------------------------------------------------------- |
| [0083](../runs/0083-ralloy-comparison/result.json)               | 0082     | `hold`  | `set-algebra` 0.267/0.296、`sig-multiplicity` 1.057/1.158 |
| [0083a](../runs/0083a-ralloy-comparison/result.json)             | 0082     | `adopt` | `set-algebra` 0.282/0.289、全13ケースが閾値内             |
| [0083b](../runs/0083b-ralloy-comparison/result.json)（報告のみ） | 0078     | なし    | 下の表                                                    |

0083では、worldごとに入れ替えられる組を求める時、atomごとに所属の`Vec<bool>`を作っていた。候補の少ない`sig-multiplicity`（約4 µs）では、この固定費が見えた。0083aで各sig集合のbitを直接比べるようにし、1.023/1.010になった。

0078の版からの累積（0083b、中央値µs、開発組と確認組の比）:

| ケース                      |     0078 |   0083a | 比            |
| --------------------------- | -------: | ------: | ------------- |
| `core-set-algebra`          | 115435.5 | 34849.7 | 0.302 / 0.300 |
| `core-dense-graph`          |    904.2 |   303.1 | 0.335 / 0.339 |
| `core-pigeonhole`           |     42.5 |    18.7 | 0.440 / 0.433 |
| `core-exact-scopes`         |      9.3 |     4.3 | 0.458 / 0.456 |
| `core-dag-closure`          |    145.6 |    68.2 | 0.468 / 0.482 |
| `core-iff-implies`          |     32.0 |    15.9 | 0.496 / 0.462 |
| `core-multiplicity`         |     66.8 |    40.6 | 0.608 / 0.596 |
| `core-preds`                |     16.0 |    10.2 | 0.641 / 0.599 |
| `core-cardinality-formulas` |      3.5 |     3.4 | 0.989 / 1.039 |
| `core-quantifiers`          |      7.3 |     7.9 | 1.091 / 1.031 |
| `core-graph-cycles`         |      5.7 |     5.9 | 1.043 / 1.056 |
| `core-sig-multiplicity`     |      3.5 |     4.0 | 1.142 / 1.157 |
| `core-restrictions`         |      8.3 |     9.7 | 1.165 / 1.170 |

8ケースは0.30〜0.64倍に速くなった。数µsの4ケースは、0079で入った固定費の分だけ遅いまま残っている（最大1.17倍）。負荷平均は3〜5で、専用ホストではない。

## 残り

- `nary`の`CompositionHolds`（約1.3億個の候補）。2列以上のfieldの対称性の除去と、subset sigを無視したより強い除去が次の候補。
- 0079から残る小さいケースの固定費（原因は未特定）。
