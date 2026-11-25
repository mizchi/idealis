# rspecの対応チェックリスト

チェック済みは、Pklで書いた仕様を読み、静的な結びつき・claim・生成物を決定的に検査し、意味のdriftをjev-lintへ渡す範囲。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] `spec/Schema.pkl`の用語（id・種類・日英の名前と定義・別名・避ける語・モデルとコードへの参照）、rule（日英の文・用語・claim・形式化しない理由）、claim（モデルのcheckと期待する判定）（0092）。
- [x] D1〜D4: idの一意性、参照の解決（モデルは宣言、コードは識別子として現れること）、参照するモデルの語彙が閉じていること、checkがすべてclaimされること、期待がcheckの種類と合うこと（0092）。
- [x] D5: claimの期待を、ralloyのSATとrtlcの判定と照らす（`just spec-check`）。D6: Pklの評価結果・Markdown・jev-lintのmanifestが古くないこと（0092）。
- [x] `transition` の名前と期待値を claim として検査する。発送仕様の `ShippedStays` は rmodel の参照評価器に照合する（0118）。
- [x] `ClauseRule` は節ごとに名前付きの claim を持てる。参照評価器による現在の判定と、対応レビューの版を `just clause-report` で照合する（0120）。
- [x] 採用判断には、現行候補と同じ版の節レビュー・意味比較・ゴール改善が必要。二つの候補版のモデルと実装テストを照合する（0122）。
- [x] 一つのモデルについて、版・モデル入力・予算を固定し、参照評価器と rprob の確定結果から採用用の意味比較を作る（0123）。
- [x] rprob の固定ワークロードの実測を、版付きのゴール改善の入力として利用できる。事前の測定条件と評価版を照合する（0124）。
- [x] `Grounding`: 実装の記号と、実装の結果をモデルの評価器で検査するテストの記号を宣言し、存在を検査する。最初の照合はroracleの`Status::aggregate`（0093）。
- [x] jev-lint docsのmanifestを仕様から生成する。用語集と各crateの文書（terminology）、ruleとモデル（semantic）。答えはledgerに追記し、助言として扱う（0092）。

## 照合

- [x] 2つの仕様（語彙21語、corpusの語彙25語・rule 12・claim 20）で、D1〜D6がすべて通る。D1〜D4・D6の故障13件を単体テストが報告する（0092）。

[評価0092](../../evals/iterations/0092-ubiquitous-language.md)を参照。

## 非対応・対象外

- [ ] コードの意味のdrift（ast-grepのruleとcutoffの較正）、各実装の振る舞いの記述（0093）。
- [ ] jev-lintの答えを合格・不合格として扱うこと、CIの合否に使うこと。

## 制約と根拠

- [x] コードの参照は、識別子が現れることだけを確かめ、意味は確かめない。
- [x] 静的に決まることはjev-lintへ渡さない。

[公開境界と検査](src/lib.rs)、[検査の実行](examples/spec_check.rs)、[テスト](tests/links.rs)を参照。
