# 0089: Quint IRを直接読むfrontend（rquint）

[plan](../plans/0089-quint-ir-frontend.md)のとおり、Quint CLIのJSON IR（`quint compile --target json`）を標準のTLA+とTLCの設定へ写すcrate `crates/rquint`を作った。Apalache（JVM）を経由せずに、Quintのモデルをrtlcで検査できる。

- **照合**: 0044の6モデル・12件で、rtlcの判定と状態数が、ApalacheのTLA+をTLCで検査した答えと一致した。生成数も12件すべて一致した。同じ出力をTLC 1.8.0で検査しても、12件で判定と状態数が一致した。
- **builtinの範囲**: `Coverage.qnt`の事実（list・集合・record・map・算術・sum type・`nondet`）を、Quint 0.33.1のsimulatorと同じ判定で検査できた。
- **出力**: `Integers`・`Sequences`・`FiniteSets`・`TLC`だけを使う。依存はserde_jsonだけで、`wasm32-unknown-unknown`にもビルドできる。

## 照合の記録

- IRは[fixtures/quint-ir](../../fixtures/quint-ir/)に、sourceとIRのSHA-256とともに保存した（[provenance](../../fixtures/quint-ir/provenance.json)）。Quint CLIは`pnpm dlx @informalsystems/quint@0.33.1`で実行した。
- TLCの答えは[tlc-rquint.json](../../fixtures/quint-ir/tlc-rquint.json)。`just rquint-tlc <output>`で記録し直せる。テストは、記録したTLA+のSHA-256が今の変換の出力と同じことを確かめる。
- 初期状態で不変条件が破れる3件では、TLCは状態数を報告しない。この3件は判定だけを比べた。
- `broken_index`は状態によらない定数なので、rtlcは探索の前に「Constant invariant or property is false」として拒否する。違反として扱い、合格にはしない。

| モデル       | 性質   | 判定                | 状態 | 生成 |
| ------------ | ------ | ------------------- | ---: | ---: |
| Counter      | safety | complete            |    4 |    5 |
| Counter      | broken | invariant-violation |    3 |    3 |
| SetFold      | safety | complete            |    1 |    2 |
| SetFold      | broken | invariant-violation |    – |    – |
| SequenceFold | safety | complete            |    1 |    2 |
| SequenceFold | broken | invariant-violation |    – |    – |
| Choice       | safety | complete            |    3 |   10 |
| Choice       | broken | invariant-violation |    3 |    4 |
| Match        | safety | complete            |    3 |    4 |
| Match        | broken | invariant-violation |    3 |    3 |
| NestedFold   | safety | complete            |    1 |    2 |
| NestedFold   | broken | invariant-violation |    – |    – |

## 変換で決めたこと

- Quintの`init`は変数への代入なので、TLA+のInitでは代入をprimeのない等式にし、`init`から呼ぶactionも展開する。
- 集合の`fold`は`CHOOSE x \in S: TRUE`の順に要素を取り出す。ApalacheがTLC向けに定義する`ApaFoldSet`と同じ順序で、可換でない畳み込みでもApalache経由の結果と同じになる。
- rtlcは`:>`・`@@`に対応しないため、`put`は`[k \in DOMAIN m \cup {key} |-> ...]`の関数の構成で書いた。
- `const`・`temporal`・`run`の定義、未知の宣言・opcode・名前は拒否する。

## 人工故障

plan で固定した3件を入れた。

| 故障                                | 単体テスト | 振る舞いのテスト    |
| ----------------------------------- | ---------- | ------------------- |
| listの添字を0始まりのまま写す       | 検出       | 検出（Coverage）    |
| `fold`の初期値を捨てる              | 検出       | 未検出→追加後に検出 |
| `matchVariant`のtagの比較を逆にする | 検出       | 検出（両方）        |

単体テストは出力の文字列を比べるので、3件とも検出した。振る舞いのテストは、`fold`の初期値を捨てる故障を検出しなかった。corpusのfoldの初期値がすべて0だったため。結果を見た後に、初期値が0でない`foldl`と`fold`の2つの事実を`Coverage.qnt`へ加え、Quintのsimulatorで判定し直してから、振る舞いのテストでも検出することを確かめた。

## 範囲外と次の段階

- Quintの公式examplesのような広いcorpus、`const`（TLCの`CONSTANT`への写し方）、temporal性質は未対応。
- 次の段階（plan 0090以降）は、構造と振る舞いを1つにまとめるkernel IRの設計。
