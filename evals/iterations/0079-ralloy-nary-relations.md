# 0079: ralloyのn項関係

[plan](../plans/0079-ralloy-nary-relations.md)のとおり、関係を4項まで一般化した。fieldの型に`A -> B`、`A -> B -> C`、`A m -> n B`を書ける。group `nary`は、6モデル・30 commandのうち29でAlloyと一致し、1件は予算切れ（`unknown`）として明示的に残した。速度は、二項以下のcaseで5〜13%（確認組では最大37%）遅くなり、planの条件（1.1以下）を満たさず`hold`。機能は採用し、後退は記録する。

## 変更

- 単項と二項は今のbit表現のまま。3項以上と混ざる演算（和・差・交差・積・join・domain/range制限・等価・包含・`no/some/one/lone`）は、atom番号のtupleの集合で計算し、結果が二項以下なら元の表現へ戻す。
- 複数列のfieldは、ownerのatomごとに列の積（tuple空間）の部分集合をbit maskで選ぶ。scopeから求めたtuple空間が64を超えるcommandは、型検査でresource errorにする。`A m -> n B`は、選んだmaskをAの各atomとBの各atomについて数えて検査する。
- `Instance.fields`はRustで`Vec<Vec<String>>`、TSで`string[]`になった。二項のJSONは変わらない。SDK（`packages/idealis/src/alloy.ts`）とevalのdecoderは2要素以上のtupleを受け付ける。Alloyへの固定（pin）も任意の項数に一般化した。

## Alloyとの差分で見つけた非互換

`no x.u`のように、列の型が交わらないjoinを、ralloyは`ill-typed join`として拒否していた。Alloy 6.2.0は受け付け、空集合として解く。項数の検査だけを残し、型の交わらないjoinを受け付けるように変えた。

## 正しさ

| group  | モデル / command | 一致 | 残り                                              |
| ------ | ---------------- | ---- | ------------------------------------------------- |
| `core` | 13 / 50          | 50   | なし                                              |
| `nary` | 6 / 30           | 29   | `field-composition`の`CompositionHolds`が予算切れ |

`CompositionHolds`は、QとPがscope 3で、Qごとに`r: P -> P`の9 bitを選ぶため、候補が約1.3億個になる。再生テストは、このcommandが`unknown`であることと、同じモデルの残り5 commandがAlloyと一致することを検査する。解けるようになったらテストが落ちて、一覧の更新を求める。0083の探索の改善で扱う。

人工故障: n項のjoinで結合する列を末尾から先頭に変える、tuple空間の積から列を1つ落とす、矢印の右の多重度を検査しない、の3件。最初の5モデルでは、joinの故障を検出できなかった。左が単項のjoinしかなく、先頭と末尾の列が同じだったため。これを見てから`right-joins.als`（5 command）を加えて記録し直し、3件すべてを検出した。結果を見てcorpusを足したことは、ここに残す。このモデルで、上の型の非互換も見つかった。

Rustのテスト（`instance`・`semantics`）、TSのcontract（`ralloy-contract`、`alloy-test`）、Chromium（`ralloy-browser`）、`just check`・`wasm-test`・`package`・`evaluate`・`replay-evaluation`が通った。`semantics`とevalの対照から、未対応として拒否する例の`sig A { r: A -> A }`を外した。

## 速度

[比較](../runs/0079-ralloy-comparison/result.json)。0078の版（1a388b142のライブラリ）とdcaa48067を、`core`の13ケースで交互の順に測った。candidate/baselineの比は、開発組1.039〜1.126、確認組1.058〜1.365で、13ケースすべてで1を超えた。`decision: hold`。測定中の負荷平均は3.0〜3.8で、専用ホストではない。

原因は特定できていない。探索として次を試したが、交互に3回測っても5〜12%の差は変わらなかった。

- `Relation::Nary`の中身をBoxにして、enumの大きさを元に戻す。
- tupleの汎用経路に`#[cold]`と`#[inline(never)]`を付ける。

samplyのprofileでは、関数ごとの時間の配分にほとんど差がなかった。candidateでは`Evaluator::expression`が独立した関数として現れ、inline化の変化が疑わしい。どちらの探索も採用せず、測定したcommitのコードのままにした。0083でprofileから改めて調べる。

```sh
just ralloy-test
just ralloy-oracle-record nary
node dist/evals/experiments/ralloy-oracle/pair.js 0079-ralloy-nary-relations <baseline> <candidate> <candidate-confirmation> <baseline-confirmation> <output>
```
