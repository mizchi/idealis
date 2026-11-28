# DDD・業務語彙・形式検証を結ぶ先行研究

## DDD のドメインモデルから証明支援までつなぐ研究はあるか

### Takeaway

DDD を明示し、一つの実行意味論から Event-B と Rodin の証明義務を生成する査読先行例がある。自然言語の規則や戦略的 DDD の Context Map まで同じ体系の中で検証した事例は、この検索では確認できなかった。

### Cited Findings

- **Le, Nguyen, Nguyen, Dang, 2026 年 3 月 26 日、UDML**。構造制約（DCSL）、業務振る舞い（AGL）、アクセス方針（RBACDom）の注釈付き DSL を、クラス・属性・操作・アクティビティノードの共通モデルと共有状態に結合する。構造の整形式は OCL。Event-B の context/machine に変換し、振る舞いの遷移を guard/invariant が制約する。Open Journal Systems の投稿ワークフローのモデルで Rodin の証明義務 381 件（自動 369、対話 12）を解消したと報告し、ProB も使用。§3、5、6.3、表6・7。ケースでは refinement と収束イベントを扱わず、ワークフロー全体の liveness は今後の課題。Event-B の context は静的要素のコンテナであり、DDD の Bounded Context と同一視できない。自由文の解釈、用語集、Context Map の実験は本文に見当たらない。 [誌面 DOI](https://doi.org/10.25073/2588-1086/vnucsce.6743) · [全文 PDF](https://jcsce.vnu.edu.vn/index.php/jcsce/article/download/6743/224)
- **Fukuzaki, Liu, Butler, 2022、DevFemOps**。利用者との対話から QA が SOFL の型・モジュール・プロセスに業務語彙を使い、SOFL のプロセス（事前・事後条件）と実装のクラス／メソッドの対応を保つ方法を提案。形式仕様を設計・変更・実装テストの起点にする。論文の評価は提案と変更事例の議論であり、自然文の自動翻訳や全実装を対象にした機械証明の実績として扱えない。 [論文](https://doi.org/10.1080/09540091.2022.2099347)
- **Arnaud Bailly, 2017**（技術記事）。Evans の Cargo/Voyage の予約例を Idris で書き直し、予約後の値と「当該 Cargo が実際に含まれるか」の判定証拠を依存対で返す。**Andor Pénzes, 2021**（実装と発表）は Wlaschin の注文処理を Idris に移し、Bounded Context、Workflow、型付き遷移と Node.js 実装を提示。依存型によるテストは README 上 WIP。論文の比較表に入れる場合は実装例と明記すべきで、自由文から証明への接続を報告するものではない。 [Bailly の原記事](https://abailly.github.io/posts/dependent-types-ddd.html) · [Pénzes の実装](https://github.com/andorp/order-taking)

### Inferences

- UDML に倣い、業務規則の意味を rmodel の単一 IR（状態・操作・名前付き制約）に置き、prover 側の義務生成と有限トレース検査をそこから派生させるのが自然。Lean 外部ファイルを同じ命題の証明として認めるには、生成済み命題の型に照合する必要があり、同名の定理や手書きの再定式化では足りない。
- 依存型の例は集約内で保たれる局所契約の候補。例えば受理時の結果に事後条件を持たせる。型・契約の保証、複数イベントの時相的な保証、外部実装との trace conformance は別々の証拠になる。

### Gaps

- “domain-driven design” “theorem prover”/Lean/Coq/Isabelle、Event-B、 “ubiquitous language” “formal verification” などで、用語・戦略的 DDD・自由文の意味・形式証明をすべて同時に検証する公刊システムは確認できなかった。網羅的調査という意味ではない。UDML の検証は OJS に基づくモデルの報告であり、実 OJS 実装全体の証明ではない。

## 自然言語・BDD と証明支援が結びつく具体例はあるか

### Takeaway

Event-B 系に、自然言語の要件やドメイン用語から受入例を作り、モデルの振る舞いと証明を同じプロジェクトで回す具体例がある。正規化済みの構造化入力、受入シナリオ、形式命題のどれが証拠かを読み分ける必要がある。

### Cited Findings

- **Williams, Darwish, Schneider, Michael, 2020、Swiftaid**（産業事例）。英国 Gift Aid 規則 → Gherkin の例 → 人手で段階的に詳細化した Event-B モデル。3 機械の証明義務を Rodin で処理し、ProB の対話シミュレーションをドメイン専門家・設計者と使用。2019 年公開の実サービスの開発で用い、ある候補機能はわずかな利便性に対してモデルで見えた複雑さが大きいと判断した。Gherkin だけの例が尽くさない挙動を形式モデルが明確化する。公開物は Rodin モデル。DDD の Bounded Context や自由文の自動証明を示した論文ではない。 [論文](https://doi.org/10.1007/s00165-020-00512-5) · [オープンモデル](https://doi.org/10.5281/zenodo.3715494)
- **Snook, Hoang, Dghaym, Salehi Fathabadi, Butler, 2021**。ドメイン専門家が読める Cucumber ベースのシナリオ DSL、Event-B モデル、Scenario Checker プラグイン。粗い例を先に書いてモデルの refinement を導く Tokeneer と、与えられた具体例を抽象化し途中のモデルで実行する ERTMS/ETCS Hybrid Level 3。モデルのアニメーションで指定例の実行可能性と期待状態を確認する。 [論文](https://doi.org/10.1016/j.sysarc.2020.101833) · [全文](https://eprints.soton.ac.uk/442450/1/JSA.pdf) · [モデルとシナリオ](https://doi.org/10.5258/SOTON/D1026)
- **Siqueira, de Sousa, Silva, 2017、BDD2EVENT-B**。SBVR 準拠の構造化自然言語で事業目標、機能、ユーザーストーリー、受入基準を段階化し Event-B に落とすという研究提案。列車例を提示。題名のとおり research idea で、成熟した自動変換ツールの根拠として扱わない。 [論文](https://doi.org/10.1109/FormaliSE.2017.5) · [著者の発表](https://www.fmeurope.org/images/formalise/FormaliSE2017Sousa.pdf)
- **Murali, Ireland, Grov, 2016、UC-B**。自然文と構造化された Event-B 数理式の両方を持つ use case を Rodin プラグインで編集し、その形式部分から Event-B モデルを生成する。自然文を自動で意味決定する方式ではなく、形式側を著者が明示する。 [著者全文](https://pure.hw.ac.uk/ws/files/10666971/abz_2016_submit.pdf)
- **Dines Bjørner, 2014/2019、Domain Engineering**。用語、注意深く書いた自然言語の narrative、形式記述を対応させる。2014 年論文はパイプラインの例に RAISE/RSL を使い、narrative を形式の読みと位置づけた。methodology として、既に成り立つドメイン記述、規範的な要求、ソフトウェア設計を区別。両記述間を自動で同義と判定したという主張はない。 [2014 年全文](https://www.imm.dtu.dk/~dibj/domains/assc-2014-bjorner.pdf) · [2019 年 TOSEM 論文](https://doi.org/10.1145/3295738) · [2006 年講義・用語と narrative の関係](https://www2.imm.dtu.dk/~db/5lectures/role-s.pdf)

### Inferences

- 自然言語を proof statement の一部と呼ぶ方法は三つある。自由文＋手書き命題の対応を審査する、制限した表現を構文解析して命題へ下げる、命題から説明文を生成する。自由文付き証明で自然文の意味まで証明したことにはならない。
- Snook らの例を rmodel に使うなら、シナリオを ITF を生成する入力として最初から型検査し、読者が期待する境界例と、モデルが生成した反例を両方再生する。rmodel を既存の shipment ITF につなげると、業務例と全遷移命題の差を確認できる。

### Gaps

- BDD2EVENT-B の汎用実装や産業再現は確認できなかった。Swiftaid と Snook の公開アーティファクトは所在を確認したが今回再実行していない。

## 戦略的 DDD とコード生成から何が借りられるか

### Takeaway

Context Mapper は既存比較に入っている。一次論文を読み直す価値は、用語・要件・戦略的境界を一つのモデルで進める段階にある。その静的検証や API 生成が業務の全実行を証明するわけではない。

### Cited Findings

- **Kapferer, Zimmermann, 2020、Context Mapper/CML**。戦略的 DDD の Bounded Context、Subdomain、Context Map、上流／下流、共有カーネル、公開言語などを記述。組合せに対する整形式規則とエラー／警告、変換を持つ。実証は試作・アクションリサーチ・ケーススタディ。 [MODELSWARD 論文](https://doi.org/10.5220/0008910502990306) · [全文](https://www.scitepress.org/PublishedPapers/2020/89105/89105.pdf) · [実装の言語意味論](https://contextmapper.org/docs/language-model/)
- **Kapferer, Zimmermann, 2021、CML 拡張**。構造化 user story/use case から Entity、Subdomain、Bounded Context を段階的に生成。Listing 5.1 の自動車保険事例では、動詞・Entity 名・属性が入力になり、ユースケースをどのサブドメインにまとめるかは人が選ぶ。生成された境界には TODO、型が決められない箇所には汎用型が残る（§5.2）。2020 年別論文はサービス分解リファクタリングと MDSL サービス契約生成を報告。ここでいうサービス契約は API 成果物であり、業務不変条件の機械証明ではない。 [著者全文 2021](https://contextmapper.org/media/978-3-030-67445-8_11_AuthorsCopy.pdf) · [Service Design 2020 全文](https://contextmapper.org/media/SummerSoC-2020_Domain-driven-Service-Design_Authors-Copy.pdf)
- **Giallorenzo, Montesi, Peressotti, Rademacher, 2022、LEMMA → Jolie**。DDD ドメイン型・サービス宣言から Jolie の型/API を出す変換を形式的に定義し、生成器を実装。生成された API をガイドに開発者が実装するため、API 型の導出と実装の振る舞いの立証を混同しやすい箇所の比較になる。 [著者プレプリント](https://arxiv.org/abs/2202.11397)
- **Meyer, 1992、Design by Contract** は、事前条件（呼出者の責任）、事後条件（実装の保証）、クラス不変条件をルーチン・型の形式的な一部に置く。**Tschannen et al., 2015、AutoProof** は後続の Eiffel＋Boogie による逐次オブジェクト指向プログラムの機能正しさの検証器。DDD の境界・自由記述は研究対象外。 [Meyer 論文](https://www.kth.se/social/files/59526bfb56be5b4f17000807/meyer-92-contracts.pdf) · [AutoProof 論文](https://arxiv.org/abs/1501.03063)

### Inferences

- rmodel の Bounded Context は、まず名前のスコープと公開型を定義し、意味の異なる TrackingStatus が並立できるようにする設計法を比較できる。CML の組織・チーム関係をすぐ導入する根拠は薄い。
- 同じ表面 DSL を採否する評価では、用語スコープの独立性、名前の変更でのリンク切れ、モデルの有限反例、証明義務の差分、生成された API を実装したときの差分を別々に計測する。

### Gaps

- CML の意味規則と近い追加例は OCL 付き UML プロファイル（Rademacher et al.）だが、業務遷移の証明という方向では UDML の方が比較に近い。 [OCL 付き UML プロファイル論文](https://doi.org/10.1007/978-3-319-74781-1_17)
