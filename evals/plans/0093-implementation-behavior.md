# 0093: 語彙の上で実装の振る舞いを書き、実装を照合する

[0092](0092-ubiquitous-language.md)の語彙の上に、実装の振る舞いをrmodelで記述する。最初はroracleの結果の集約（`Status::aggregate`）。モデルの検査（engine）に加え、実装の入出力をモデルのinstanceにして、ralloyの`check_instance`で事実を満たすかを確かめる（照合、grounding）。0092で記録した予算切れの語の違い（ralloyのunknown、rtlcのbounded）は変えない。

## 変更

- rmodelに`enum Name = A | B | ...`を加える。名前の付いた、ちょうど1つずつのatom。Alloyでは`abstract sig`と`one sig ... extends`、TLA+では文字列の集合。enumはscopeを書かない。
- `spec/models/roracle-report.model`: 3つのslotのうちstatusを持つものを比較とみなし（sigのscopeはちょうどの数なので、比較0〜3件をslotで表す。engineを走らせる前に、`sig Comparison`の版から直した）、そのstatusと、集約した結果（`report`）。集約の規則を`fact`で書く。比較がないときはinvalid、errorがあればerror、なければmismatch、unknown、invalid、すべてmatchedならmatchedの順。
- `spec/idealis.pkl`: このモデルの宣言を語彙へ結びつけ、ruleとclaimを加える。Schemaに`Grounding`（モデル、実装の記号、照合するテストの記号）を加え、rspecは記号が存在することを静的に検査する。
- 照合: roracleのテストで、比較0〜3件のすべてのstatusの組（1+5+25+125=156通り）について`Status::aggregate`の結果をinstanceにし、`check_instance`がfactsの違反（invalid）を返さないこと。

## 正しさ（結果を見る前に固定）

- モデルのcheckと期待（ralloyのSATとrtlcで一致すること）:
  - `MatchedOnlyIfAllMatched`（invariant）: holds
  - `UnknownNeverMatched`（invariant）: holds
  - `InvalidNotMismatch`（invalidがあってmismatchがなければ、結果はmismatchでない。invariant）: holds
  - `UnknownOverInvalid`（errorとmismatchがなく、unknownとinvalidがあれば、結果はunknown。invariant）: holds
  - `InvalidHiddenByMismatch`（invalidの比較があっても、別の比較がmismatchなら結果はmismatch。reachable）: reachable
  - `EmptyIsInvalid`（比較がなければinvalid。reachable）: reachable
- 照合: 156通りすべてで、実装の結果がモデルの事実を満たす。
- 人工故障: 実装の重さの順でinvalidとunknownを入れ替える、空のときをmatchedにする、の2件を照合が検出すること。モデルの規則からerrorの行を落とす故障を、claimの検査（`UnknownNeverMatched`などとは別に）か照合のどちらかが検出すること。
- rmodelのenumの構文と型の誤り（重複したmember、scopeにenumを書く、未知のmember）を拒否する単体テスト。

## 範囲外

- `compare`（観測の組から比較のstatusを出す規則）、rtlc・ralloyの結果の振る舞い、予算切れの語を揃えること。
- 状態を持つ実装の照合（traceの再生）。idealis-connectとの接続は後の段階。
