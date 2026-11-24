# 0093: 語彙の上で実装の振る舞いを書き、実装を照合する

[plan](../plans/0093-implementation-behavior.md)のとおり、roracleの比較の集約（`Status::aggregate`）を、0092の語彙の上でrmodelのモデルに書き、実装をそのモデルと照合した。

- **モデル**: [`spec/models/roracle-report.model`](../../spec/models/roracle-report.model)。結果の値はrmodelに加えた`enum`で、`Matched`などのmemberを語彙の用語（compare.matchedなど）へ結びつけた。ruleは4つ、claimは6つ。ralloyのSATとrtlcの判定は、6つとも先に書いた期待と一致した。
- **照合（grounding）**: 比較0〜3件の156通りすべてで、`Status::aggregate`の結果をモデルのinstanceにし、ralloyの`check_instance`でモデルの事実を満たすことを確かめた（`aggregate_conforms_to_the_report_model`）。0090で作ったwitnessの検査器を、実装の照合にそのまま使っている。
- **語彙**: `spec/idealis.pkl`は25語・rule 4・grounding 1。rspecはgroundingの実装とテストの記号の存在も検査する。

## 人工故障

| 故障                                         | 検出                                                                                                                                                |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| 実装: 重さの順でinvalidとunknownを入れ替える | 照合（`[Unknown, Invalid] -> Invalid`がfactsの違反）                                                                                                |
| 実装: 比較がないときをmatchedにする          | 照合（`[] -> Matched`がfactsの違反）                                                                                                                |
| モデル: errorの規則を落とす                  | claimの検査（`MatchedOnlyIfAllMatched`と`UnknownNeverMatched`がSAT・rtlcの両方でviolated）。照合は通る。事実が弱くなるだけなので、plan の想定どおり |

## jev-lint（jev-1.14.0、UNREVIEWED）

用語集が変わったので、terminologyの比較は聞き直しになった。追加した4つのruleとモデルの比較は、すべてsupports。0092で矛盾とされたidealis-cliのSUPPORT.mdはsupportsになった。ralloyのREADMEは矛盾（0.63）のままで、0092と同じく根拠を特定できない候補として残す。[記録](../runs/0093-implementation-behavior/drift.json)。

## 計画からの差

- 最初に書いたモデルは`sig Comparison`で比較を表していたが、rmodelのsigの大きさはちょうどscopeの数なので、比較0件を表せなかった。engineを走らせる前に、3つのslotのうちstatusを持つものを比較とみなす形へ直し、planにも書いた。
- idealisの仕様のruleもjev-lintのsemanticの比較に含めた（0092では領域の仕様だけだった）。

## 次の段階

- `compare`（観測の組から比較のstatusを出す規則）、rtlc・ralloyの結果の振る舞い。予算切れの語（ralloyのunknown、rtlcのbounded）を揃えるかの判断はまだ保留。
- 状態を持つ実装の照合（idealis-connectでmodelのtraceを実装に再生する）。
