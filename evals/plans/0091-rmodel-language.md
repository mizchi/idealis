# 0091: 構造と振る舞いを1つのモデルで書く言語の核（rmodel）

mizchiの方針（Q3の3）: 関係で書く構造、状態、actionで書く振る舞いを、同じ世界の1つのモデル体系で記述する。同じモデルを複数のengineで検査し、今まで1つの道具で扱えなかった問題（設定の空間と振る舞いの組み合わせ）も同じ枠で書けるようにする。

## 位置づけ（dspecとの関係）

mizchiの試作[dspec](https://github.com/mizchi/dspec)は、Pklで人間向けの仕様（語彙、`when`/`must`の節を持つrule、多言語の文）を書き、backendごとの小さなDSLからLean・Alloy・Quintのテキストを生成していた。ここでは3層に分ける。

1. 仕様（Pkl、0092で作る）: 語彙・rule・claim。claimはruleを、モデルの名前（`invariant`・`reachable`・`action`・`sig`・`rel`）と期待する判定へ結びつける。節の式を文字列で持たない。
2. モデル（この0091）: 構造・状態・振る舞いを1つの意味論で書く。backendごとのDSLは作らない。
3. engine: このリポジトリのralloy（SAT）とrtlc（明示状態）。Alloy・TLCは記録したoracle。

このため、モデルの宣言と検査の名前は、Pklから参照される安定した識別子として扱う。0091の期待する判定は`expected.json`に置き、0092でPklのclaimへ移す。

## 言語の核（v1）

```
model Permissions
sig User
sig Role
rel grants: User -> set Role      // 静的な関係（設定）。solverがすべての候補を考える
rel cleared: set User
var open: User -> set Doc         // 状態
fact { ... }                      // 設定の制約。varを参照しない
init { ... }                      // 初期状態
action read(u: User, d: Doc) {    // 遷移。代入しないvarは変わらない（暗黙のframe）
  d in u.grants.reads
  open' = open + u -> d
}
invariant NoLeak { ... }          // 到達できるすべての状態で成り立つか
reachable SecretRead { ... }      // 成り立つ状態に到達できるか
scope User = 2, Role = 2, steps = 6
```

- 値: sigのatomの集合（単項）と二項関係。多重度は`set`・`lone`・`one`・`some`（二項は右側、単項は集合の大きさ）。整数は単項の式の`#e`と、絶対値7以下のリテラルだけ。sigのscopeは1〜7で、整数は4 bit（-8〜7）で桁あふれしない。
- 式: join（`e.r`・`r.e`）、`->`、`+`・`-`・`&`、`none`、`#`。論理式: `in`・`=`・`!=`・`no`・`some`・`one`・`lone`・`not`・`and`・`or`・`implies`、`all`・`some`による量化、`{}`の連言、整数の比較。
- action: 本体は連言。primeを付けたvarは`v' = 式`の形でだけ書け、右辺にprimeを書けない。代入しないvarと、すべての`rel`は変わらない。varの多重度は全状態で成り立ち、破る遷移は起きない。
- 遷移はいずれかのactionか、何も変えないstutter。scopeのsigの大きさはちょうどその数。

## engineと写し方

1つのモデルを、独立した2つの写し方で検査する。

- **関係とSAT**: Alloy 6のテキストへ写す。すべての`rel`・`var`は`one sig World`のfield。ralloyのSATで、長さstepsのlassoを有界に探す（0090）。同じテキストをAlloy 6.2.0でも検査する。
- **明示状態**: TLA+とTLCの設定へ写す。atomは文字列、関係はtupleの集合。`rel`は、Initで`SUBSET`から事実を満たすものを選んで変えない変数にする（設定の空間を明示的に列挙する）。rtlcで検査し、同じテキストをTLCでも検査する。
- 判定: `invariant`は違反の有無、`reachable`は到達の有無。明示状態は完全（complete）、SATはsteps内の有界（bounded）として区別する。

## 正しさ（結果を見る前に固定）

- corpus: `fixtures/rmodel/`の4モデル・17 check（mutex・permissions・permissions-open・sessions）。各commandの期待する判定を`fixtures/rmodel/expected.json`に先に書く。
- 次の4つがすべて期待と一致すること。ralloyのSAT（Alloy経路）、rtlc（TLA+経路）、Alloy 6.2.0（Alloy経路の記録）、TLC（TLA+経路の記録）。
- 明示状態が見つけた違反・到達の深さdは、steps-1以下であること（SATの有界が足りている）。足りないcommandは比較に数えず、件数を報告する。
- 人工故障: 暗黙のframeを落とす（代入しないvarを自由にする）、varの多重度を初期状態だけにする、joinの向きを逆にする（`e.r`を`r.e`として写す）、の3件を検出すること。
- 構文と型の誤り（未知の名前、`fact`でのvarの参照、actionの外のprime、右辺のprime、同じvarへの2回の代入）を拒否する単体テスト。

## 範囲外

- 閉包、3項以上の関係、時相の性質（`always`・`eventually`の入れ子）、整数の変数、module。
- 0089のQuint IRからこの言語への写し方。
