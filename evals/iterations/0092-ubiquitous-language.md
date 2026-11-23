# 0092: ユビキタス言語と、層の間のdrift検知（rspec）

[plan](../plans/0092-ubiquitous-language.md)のとおり、仕様の層を作った。Pklで用語・rule・claimを書き、用語をモデルの宣言とコードの記号へ、ruleをモデルのcheckへ結びつける。決定的に決まることは`crates/rspec`で検査し、意味の食い違いだけを`jev-lint docs`へ渡す。

- **語彙**: `spec/idealis.pkl`にidealis自身の語彙21語（探索の範囲・判定・比較・モデル）を書き、`docs/glossary.md`へ生成した。`fixtures/rmodel/spec.pkl`に0091のcorpusの語彙25語・rule 12・claim 20を書き、`expected.json`を置き換えた（期待は同じ）。
- **決定的な検査**: D1〜D6がすべて通る。claimの20件は、ralloyのSATとrtlcの判定に一致する。故障13件（D1×3、D2×4、D3×2、D4×4）とMarkdownの陳腐化を単体テストが報告する。
- **jev-lint**: 用語集と各crateの文書24組（terminology）、ruleとモデル16組（semantic）、対照4組。対照はすべて意図どおり（矛盾の対照は矛盾、正常の対照は支持）。

## jev-lintの結果（jev-1.14.0、UNREVIEWED）

対照:

| 対照                                 | 答え        | 矛盾の確率 |
| ------------------------------------ | ----------- | ---------: |
| 有界を完全・証明済みとみなす文書     | contradicts |       1.00 |
| 有界を有界として扱う文書             | supports    |       0.01 |
| `PERM-NO-LEAK`を反対の意味にしたrule | contradicts |       0.66 |
| `PERM-NO-LEAK`の言い換え             | supports    |       0.15 |

実際の文書で矛盾とされた2件を、本文を読んで分けた。

| comparison                      | 1回目 | 分類                                                                                                                                                                                                                                                                             |
| ------------------------------- | ----: | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/idealis-cli/SUPPORT.md` |  0.73 | 正しい指摘。CLIは予算切れを`bounded`と出力するが、用語集は`bounded`を深さ・stepsの打ち切りに限り、予算切れを`unknown`の別名にしていた。rtlcの`bounded`は、深さ・状態数・work・temporal・evaluationの上限をreasonで区別する。用語集がrtlcの意味より狭かったので、用語集を直した。 |
| `crates/ralloy/README.md`       |  0.59 | 根拠を特定できない。表の語は用語集と合っている。読んでいて、no-counterexampleを「指定scope内を列挙して」と書いた古い記述（SATで解くcommandは列挙しない）を別に見つけ、直した。                                                                                                   |

直した後の2回目でも、2件は0.55と0.54で矛盾のまま残った。skillの指示どおり繰り返し追わず、残った候補として記録する。ほかに、語彙と関係しない文書3件が`unrelated`、ruleとモデルの4組が`insufficient_context`で、どちらも一致には数えない。

[1回目](../runs/0092-ubiquitous-language/run-1.json)・[2回目](../runs/0092-ubiquitous-language/run-2.json)・[3回目（rspecの文書を追加）](../runs/0092-ubiquitous-language/run-3.json)。rspecの文書へformatterをかけた後に2件を聞き直した[4回目](../runs/0092-ubiquitous-language/run-4.json)が、ledgerの現在の答え。答えは`.jev-lint/judgments/spec`のledgerにあり、`just spec-drift-replay`でAPI keyなしに再生できる。

## 分かった語の違い

work予算が尽きたとき、ralloyは`unknown`（判定なし）、rtlcは理由workの`bounded`（調べた部分に違反なし）を返す。語は違うが、どちらも合格ではない点は同じ。0092では用語集にこの違いを書いたまま、コードは変えていない。どちらかへ揃えるとAPIが変わるため、別に判断する。

## 計画からの差

- jev-lintは助言（AGENTS.md）なので、CIの合否に使わない。CIは`just rspec-test`と`just spec-check`（pklを固定版・checksum付きで入れる）だけを合否にする。
- `jev-lint docs`は、OpenRouterのkeyもある環境では対話的な確認を求めるため、`OPENROUTER_API_KEY`を外し、support-syncと同じ`jev-1.14.0`を使った（`just spec-drift`）。
- planにない`rspec`自身の文書2件をmanifestに加えた（manifestはcrateの文書から生成するため）。

## 次の段階

- 0093: この語彙の上で、各実装（ralloy・rtlc・roracleなど）の結果の振る舞いをrmodelで記述し、claimで結びつける。予算切れの語を揃えるかどうかも、そこで扱う。
