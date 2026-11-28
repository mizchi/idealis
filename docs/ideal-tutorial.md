# はじめての Idealis

図書館では1冊を同時に借りられるのは1人です。返却すると借り手はいなくなりますが、貸し出したことのある本として記録は残ります。

Idealis では、こうした業務の用語と操作を `.ideal` ファイルに書き、どんな操作順でも規則が守られるかを検査できます。

## 貸出モデルを書く

次の内容を `library.ideal` として保存します。

```rmodel
model Library
entity Copy "図書館が所蔵する1冊"
enum Member = Aki | Ren

var borrower: Copy -> lone Member
var everBorrowed: set Copy
init { no borrower, no everBorrowed }

behavior borrow(c: Copy, m: Member) {
  guard { no c.borrower }
  update {
    borrower = borrower + (c -> m)
    everBorrowed = everBorrowed + c
  }
}

behavior giveBack(c: Copy, m: Member) {
  guard { m in c.borrower }
  update { borrower = borrower - (c -> m) }
}

invariant LoanIsRecorded { keys(borrower) in everBorrowed }
reachable RenCanBorrow { some c: Copy | Ren in c.borrower }
transition ReturnKeepsRecord on giveBack { unchanged(everBorrowed) }

scope Copy = 1, steps = 4
```

`entity Copy` で「蔵書の1冊」に名前を付けます。引用符内の説明は、後で生成する用語集にも使われます。`scope Copy = 1` で検査対象を1冊に絞り、`enum Member` には名前付きの会員を2人置きます。

`borrower` は蔵書と会員の対応です。`lone` は蔵書1冊につき会員が0人か1人という制約で、`c.borrower` でその本の借り手を読めます。`everBorrowed` は貸し出したことのある本の集合です。どちらも `var` で宣言し、`init` で空から始めます。

`borrow` の `guard` は、その本に借り手がいないときに成立します。`update` は貸出の組 `c -> m` と履歴を加えます。

右辺はすべて操作前の状態を読み、更新は同時に反映されます。`giveBack` が更新するのは `borrower` だけなので、`everBorrowed` はその値を保ちます。

| 検査したいこと                                       | このモデルでの書き方                                                                                                |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| 到達するすべての状態で、貸出中の本が履歴に載っている | `invariant LoanIsRecorded`。 `keys(borrower)` は貸出中の本の集合で、`in` は集合の包含を表します。                   |
| Ren が借りている状態に一度でもたどり着く             | `reachable RenCanBorrow`。 `some c: Copy \| ...` で、条件に当てはまる本が存在するかを調べます。                     |
| 返却しても履歴が変わらない                           | `transition ReturnKeepsRecord on giveBack`。 `unchanged(e)` は1回の操作の前後で `before(e) = after(e)` を求めます。 |

履歴は貸出のたびに増え得ます。返却の前後に限って値を比較したいので、`ReturnKeepsRecord` には `transition ... on giveBack` を使います。

## 検査して、反例を読む

リポジトリのルートに `library.ideal` を保存した場合は、次のコマンドで実行できます。

```sh
cargo build --locked --manifest-path crates/rcheck/Cargo.toml
rcheck=crates/rcheck/target/debug/rcheck

"$rcheck" library.ideal --checker reference --question LoanIsRecorded --expect holds
"$rcheck" library.ideal --checker reference --question ReturnKeepsRecord --expect holds
"$rcheck" witness library.ideal RenCanBorrow
```

最初の2件は `holds` です。3件目は `reachable` を返し、`trace.commands` に Ren が蔵書 `Copy0` を借りる操作が1件入ります。

```json
[{ "c": "Copy0", "kind": "borrow", "m": "Ren" }]
```

貸出の `update` から `everBorrowed = everBorrowed + c` を外して、次のコマンドを実行します。

```sh
"$rcheck" witness library.ideal LoanIsRecorded
```

`violated` が返ります。最初の貸出で `borrower` に組が入り、履歴は空のままです。違反までの操作は `trace.commands`、違反時の状態は `trace.finalState` にあります。

確認したら更新を戻します。ここで確かめたのは、蔵書1冊と会員2人のモデルでの振る舞いです。`scope` はモデルの範囲を決め、`steps` は Alloy の有界検査などで使います。

`--max-states` などで制限する探索予算は別にあり、予算切れなどで結果が未確定なら `unknown` が返ります。使える検査器は `"$rcheck" capabilities library.ideal` で確認できます。

## 「貸出中なら、いずれ返却される」を書く

元のモデルの `scope` の前に次の性質を追加します。

```rmodel
leads_to LoanEventuallyReturns(c: Copy) with { weak giveBack(*) }
  { some c.borrower }
  { no c.borrower }
```

`leads_to` は1冊ごとに、貸出中になった時点からいずれ借り手がいない状態に戻ることを求めます。`weak giveBack(*)` は、実行可能なままに留まる引数の組について、返却がいずれ実行されるという仮定です。

```sh
"$rcheck" library.ideal --checker rprob --question LoanEventuallyReturns --expect holds
```

この仮定の下では `holds` です。`with { weak giveBack(*) }` を `with {}` にして、次のコマンドを実行すると、貸し出した後に返却せず留まり続ける反例が見つかります。確認したら `with { weak giveBack(*) }` に戻します。

```sh
"$rcheck" witness library.ideal LoanEventuallyReturns --checker rprob
```

## 用語と型を実装へ持っていく

ソースに書いた `Copy` の説明は用語集に、`Member` や操作の引数は実装用の型に出せます。TypeScript では会員の型が `"Aki" | "Ren"` になります。Rust や Go にも同じソースから型を生成できます。

```sh
"$rcheck" generate library.ideal glossary
"$rcheck" generate library.ideal typescript
"$rcheck" generate library.ideal rust
"$rcheck" generate library.ideal go
```

金額に名前を付けるなら `record Amount { units: Nat }`、決済状態ごとに持つ値が違うなら `variant Payment = Pending | Captured(Receipt)` と書けます。`scope Int = 0..2` と指定すると、このモデルで扱う自然数は0、1、2です。

注文と決済の例では、型とプロパティベーステストの境界ケースを生成します。操作前は `validate` で入力を検査し、操作後は実装の状態をモデルと照合します。途中の観測は ITF というトレース形式で保存して再生できます。[注文と決済の実行例](../examples/model-patterns/checkout/README.md)

現行の `.ideal` には用語と形式的な性質を書けます。自然言語の規則文を性質にひも付けるときは、現時点では別途 Pkl に記述します。[言語の設計](language-architecture.md)

### 説明を複数行・複数言語で書く

最初のモデルの `entity Copy "図書館が所蔵する1冊"` を次の宣言に置き換えられます。`//` で始めた行はソースを読む人へのコメントで、`description` の本文は用語集に残ります。

```rmodel
// 貸出と返却は、所蔵する1冊を単位に扱う。
entity Copy description = {
  default: "図書館が所蔵する1冊",
  ja: """図書館が所蔵する1冊。
同じ題名でも、所蔵する1冊ずつを区別します。""",
  en: """A single copy held by the library.
Copies with the same title are tracked separately."""
}
```

`"""` の間は改行や空白も本文に入ります。言語指定が不要なら `entity Copy description = """…"""` と書けます。`"en-US"` のようにハイフンを含む言語タグは引用符で囲み、出力では `en-us` になります。

```sh
"$rcheck" generate library.ideal glossary-localized
```

`glossary-localized` は全言語を出力します。`glossary` は従来どおり、言語指定のない説明と `default` を平坦な JSON に出します。

### ほかのモデルを読む

会員管理を貸出の外に出すと、資格を直接読む場合と通知で受け取る場合で、停止直後の振る舞いが変わります。[貸出と会員管理をつなぐ例](../examples/model-patterns/library-membership/README.md)

待ち行列の `sequence`、固定の関係 `rel` とその制約 `fact`、複数ファイルの接続はチートシートに例があります。[文法チートシート](../crates/rmodel/CHEATSHEET.md)

画面遷移や Node.js のタスク実行順序にも、同じ構文で状態と操作を書けます。[環境別のモデルパターン](rmodel-patterns.md)
