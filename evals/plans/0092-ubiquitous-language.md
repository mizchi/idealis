# 0092: ユビキタス言語と、層の間のdrift検知（rspec）

mizchiの方針: まずユビキタス言語を作って語彙を統一し、その上に各実装の振る舞いを記述する。jev-lintを前提に、層の間のdriftを検知する。[0091](0091-rmodel-language.md)の3層（仕様・モデル・engine）のうち、仕様の層を作る。

## 層と結びつき

- **用語（Pkl）**: `spec/Schema.pkl`の`Term`。id・種類・日英の名前と定義・許す別名・避ける語と、表すものへの参照を持つ。参照はモデルの名前（`.model`の宣言・check）と、コードの記号（ファイルと識別子）。
- **rule（Pkl）**: 日英の文、使う用語、claim。claimはモデルのcheckと期待する判定（holds・violated・reachable・unreachable）。
- **projection**: Pklから日本語の用語集とruleの一覧（Markdown）を決定的に生成する。jev-lintはこの生成物を読む。
- 2つの仕様を書く。
  - `spec/idealis.pkl`: このリポジトリ自身の語彙。各crateが使う結果の語（complete・bounded・unknown、instance・counterexample、matched・mismatch・invalid、witness・trace・lasso・設定・状態・frameなど）と、各crateでその語を表す記号。
  - `fixtures/rmodel/spec.pkl`: 0091のcorpusの領域の語彙とrule。`expected.json`をこのclaimへ置き換える（判定の期待は変えない）。

## 検査

決定的に決まることは`crates/rspec`で検査し、意味の判断だけをjev-lintへ渡す。

- 決定的（`cargo test`と`just spec-check`）:
  - D1: id・用語の一意性、ruleが参照する用語の存在。
  - D2: モデルの参照が宣言またはcheckへ解決すること。コードの参照が、そのファイルに識別子として現れること。
  - D3: 仕様が参照するモデルの宣言はすべて、いずれかの用語に結びつく。checkはすべて、いずれかのruleのclaimに属する。
  - D4: claimの期待がcheckの種類と合う。ruleはclaimを持つか、理由付きで`unformalized`。
  - D5: claimの期待が、ralloyのSATとrtlcの判定に一致する（0091の検査をclaimから行う）。
  - D6: 生成したMarkdownが、Pklの評価結果から作ったものと一致する（`pkl eval -f json`の出力をcommitし、`just spec-check`が評価し直して比べる）。
- jev-lint（`jev-lint docs`のmanifest、判定はledgerに追記し、CIではreplay）:
  - J1 terminology: 用語集（reference）と各crateの`SUPPORT.md`・`README.md`（document）。用語の定義と矛盾する使い方、避ける語の使用。
  - J2 semantic: ruleの一覧の各rule（document）と、claimが指すモデルのファイル（reference）。ruleの文がモデルの意味と食い違うか。

## 正しさ（結果を見る前に固定）

- D1〜D6の故障: 未知の用語、解決しないモデル・コードの参照、用語のない宣言、claimのないcheck、種類と合わない期待、古い生成物を、それぞれ拒否する単体テスト。
- J1・J2は、意図的に食い違わせた対照で確かめる。J1: 用語集で「boundedは完全な検査ではない」と定義し、document側に「SATの結果は完全な検査」と書いた対照の文書を置く。J2: ruleの文を反対の意味にした対照（`NoLeak`を「誰でも機密文書を開ける」）。それぞれ矛盾として報告され、正常版は報告されないこと。対照はmanifestの別のcomparisonにして、本番の判定と混ぜない。
- 実際のcrateの文書へのJ1の結果は、判定の前に見ない。報告された矛盾は1件ずつ読み、正しい指摘・言い換え・誤りに分けて記録する。jev-lintの判定を、そのまま合格・不合格に読み替えない（qualityVerdictはUNREVIEWED）。

## 範囲外

- 各実装の振る舞いを、この語彙の上にrmodelで記述すること（0093）。
- コードの意味のdrift（ast-grepのruleとcutoffの較正）。0092ではコードは識別子の存在だけを決定的に検査する。
- dspecの多言語projection・実装へのgrounding。
