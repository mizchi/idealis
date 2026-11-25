# 制御自然言語から形式仕様と証明へ

## 自然言語に近い要件を時相論理へ落とした例はあるか

### Takeaway

NASA FRET は自然言語に似た入力、時相論理、図、例の実行を同じ要件から得る。PVS による翻訳の意味保存証明があり、rmodel の遷移・活性の表記に近い。

### Cited Findings

- Giannakopoulou ら、_Formal Requirements Elicitation with FRET_（REFSQ Workshops 2020）。FRETish の原型には適用区間、発火条件、対象コンポーネント、要求を示す固定語、時間制約、応答の6欄がある。未来／過去時相論理の式、英語の説明、図、対話シミュレーションを生成する。論文 §2 図2 の editor には Requirement ID と Parent Requirement ID があり、AP-002A が実例。適用区間は実行時系列上の範囲。 [NASA の一次論文](https://ntrs.nasa.gov/api/citations/20200001989/downloads/20200001989.pdf)
- Conrad ら、_A Compositional Proof Framework for FRETish Requirements_（CPP 2022）。PVS で FRETish の表示的意味論と生成された**過去時点の計量時相論理**の意味の一致を構成的に証明する。対象は自然数で刻む有限トレース。任意の長さの未来時相式や無限トレースへの拡張は §9 で今後の課題。保証の対象はこの範囲の式を生成するアルゴリズムと意味論の対応。 [出版社](https://doi.org/10.1145/3497775.3503685) · [本文](https://arxiv.org/abs/2201.03641)
- FRET は CoCoSim や Copilot と接続し、実現可能性検査、モデル照合、C プログラムの監視にも使える。これは翻訳証明とは別のツール機能。 [NASA の著者によるチュートリアル](https://ntrs.nasa.gov/citations/20220009659)

### Inferences

- rmodel では時相パターンを既存の transition と leads_to の上に置ける。意味の分岐は条件の立ち上がり／成立中、次の明示操作／内部待機、時間単位、適用区間の終端、活性の公平性。各例に正常・反例トレースを持たせると解釈を確認できる。
- 翻訳が意味を保つことと、モデルが仕様を満たすことは別の証明義務。有限な状態探索で後者を調べるなら、結果に状態・整数等の境界を記録する。

### Gaps

- FRET の要件の scope は時間区間。DDD の集約や意味境界間の翻訳は一次論文では確認できなかった。

## ドメインの語彙と契約、テスト、証明を一体にした例はあるか

### Takeaway

GE の ASSERT はドメイン語彙、自然言語風の要件、ACL2s、テスト生成を一つの作業系に統合した。KeY はオブジェクトのモデルと契約について、自然言語表示と実装の演繹的検証を組み合わせた。

### Cited Findings

- Moitra ら、_Towards Development of Complete and Conflict-Free Requirements_（IEEE RE 2018）。SADL オントロジーで概念（クラス）・関連（プロパティ）を宣言し、SRL で同じ語彙を使う構造化英語の要件を書く。名前のある要件（例 FilteredValue、HardFault）を、型付き defdata、監視／制御変数、前時点の値を含む ACL2s 表現へ落とす。競合・網羅性を定理証明で試み、Cgen は型由来の PBT と定理証明を組み合わせて反例を探す。どちらも得られなければ未確定。別経路で SMT を使い、要件カバレッジに基づくテストを生成し、センサー事例で SCADE 上で実行した。網羅性の範囲は、特定の制御変数に関連する要求と仮定。 [著者公開論文 §§II, IV–VII](https://www.khoury.northeastern.edu/~pete/pub/re-2018.pdf)
- Ahrendt ら、_The KeY Tool: Integrating Object Oriented Design and Formal Verification_（SoSyM 2005）。UML クラス図の OCL 不変条件・事前／事後条件、Java Card 用動的論理、対話的・自動的定理証明。§5.3 には GF によってドメイン型や語彙を生かし、OCL を英独瑞語へ線形化し、同時オーサリングで single source とする発想。§11 は言語表示を行うオーサリングツールの統合を報告。Heldal・Johannisson（2006）は問題領域の語彙を使う OCL 契約を顧客向けの英語に自動生成する。契約が本当に望む性質かは生成された表示を顧客がレビューする。 [KeY 一次論文 §§2, 5.3, 7, 11](https://formal.kastel.kit.edu/beckert/pub/sosym2005.pdf) · [Customer Validation of Formal Contracts](https://research.chalmers.se/en/publication/25597)
- Schwitter・Fuchs、_Attempto — From Specifications in Controlled Natural Language towards Executable Specifications_（1996）。文法を制約した英語とアプリ別語彙を使い、談話表現と Prolog へ変換して質問・シミュレーションに使う。Fuchs・Schwertel の RACE は一階論理の定理証明や、最小の矛盾文集合・証明に使った要件に戻る説明を扱う。 [1996 年論文](https://arxiv.org/abs/cmp-lg/9603004) · [Reasoning in Attempto Controlled English](https://attempto.ifi.uzh.ch/site/pubs/papers/fuchs2003reasoningIn.pdf)
- Merigoux・Chataing・Protzenko、_Catala: A Programming Language for the Law_（ICFP 2021）。法文と実行可能な規則を近接させる文芸的プログラミング。法律家と開発者が手動で意味を解釈し、F* がコア変換の正しさを保証。実行ログに原文の位置を結び付ける。ProLaLa 2022 の Delaët らの論文は、法プログラムの性質の証明を意味保存的な簡約で支援する提案。 [ICFP 論文](https://doi.org/10.1145/3473582) · [本文](https://arxiv.org/pdf/2103.03198) · [ProLaLa 論文](https://popl22.sigplan.org/details/prolala-2022-papers/12/Turning-Catala-into-a-Proof-Platform-for-the-Law)

### Inferences

- コンテキスト内の型・操作と要件語彙を紐づけ、未定義・多義な語を診断し、反例に要件 ID、変数、操作、入力を返すと自然言語とテストが一つの検討経路になる。手書きの承認文を残す使い方と、型付き契約から制御日本語を生成する使い方では、検出できるずれが違う。

### Gaps

- ASSERT と KeY は戦術的 DDD に似た語彙を持つ。Evans の bounded context や集約間の契約まで同じ仕組みで検証するという記述は確認していない。

## 自然言語と証明支援系の境界はどこか

### Takeaway

Lean 内の制御英語の変換を監査可能にした研究と、証明本文を読み物として書く研究は別の選択肢。rmodel が外部 Lean を維持しても解釈の由来を追える。

### Cited Findings

- Gordon・Matskevich、_Trustworthy Formal Natural Language Specifications_（Onward! 2023）。Lean 内の範疇文法と型クラスで、制御英語を Lean の命題へ合成する。辞書はライブラリ単位で拡張でき、証明書から語の意味と合成規則を監査できる。VFA の挿入ソート・多重集合の命題で試作し、§6 は量化の明示や曖昧な表現の未処理を報告。証明書が示すのは採用した辞書・文法での解釈。辞書が要求元の意図に一致するかは人の判断が残る。 [著者公開論文 §§4–6](https://csgordon.github.io/publications/onward23/onward23.pdf) · [出版社](https://doi.org/10.1145/3622758.3622890)
- Ranta、_Grammatical Framework_（JFP 2004）。意味を担う一つの型付き抽象構文と自然言語・形式言語の具体構文を分ける。Hallgren・Ranta（LPAR 2000）は Alfa の作業中の証明を、未解決の穴を含めて自然言語に表示。Pathak の _GFLean_（2024 プレプリント）は簡素化した ForTheL から Lean 式を生成した試作で、教科書62文中42文、軽微な言い換え込みが報告範囲。 [GF 論文](https://www.cambridge.org/core/journals/journal-of-functional-programming/article/grammatical-framework/09C4B3F43447ADDB0F632C64F195BC9C) · [Alfa 一次論文](https://cth.altocumulus.org/~hallgren/Papers/lpar2000.pdf) · [GFLean §6](https://arxiv.org/pdf/2404.01234)
- De Lon ら、_The Isabelle/Naproche Natural Language Proof Assistant_（CADE 2021）。ForTheL は数学の定義・定理・証明の制御自然言語。義務が残れば TPTP に出し自動定理証明器 E に委ねる。Isabelle/jEdit の編集環境と連続検査を統合している。§6 は Isabelle/HOL の論理との接続を将来に残す。Wenzel（TPHOLs 1999）の Isar は自然演繹を読める形式証明として書く別系統の文法。 [Naproche §§5.6, 6](https://doi.org/10.1007/978-3-030-79876-5_36) · [Isar 一次論文](https://www21.in.tum.de/~wenzelm/papers/Isar-TPHOLs99.pdf)
- Pessaux、_FoCaLiZe: Inside an F-IDE_（2014 公開版）。仕様・実装・構造化証明を一つの言語で持ち、再利用・段階的設計を支援する。証明側の出力は Coq で検査し、実行側は OCaml。 [FoCaLiZe 論文](https://arxiv.org/abs/1404.6607) · [FoCaLiZe 公式解説](https://focalize.ensta-paris.fr/)

### Inferences

- どの語句がどのドメイン記号に束縛され、どの構文から式ができたかをソース位置付きで出力する。次に同じ式から Lean 命題を生成し、既存の有限オラクルに加え、Lean カーネルが照合できる証明を接続する。
- 自由文＋既存式へのリンク、型付き IR を日本語に線形化する GF 型、制御日本語を型付き IR に落とす FRET 型を、出荷保持の隣接性と Raft の活性で比較する。観測の候補は量化・公平性の読み違い、参照の一意性、反例の説明、既存バックエンドの意味との一致。

### Gaps

- これらの一次論文で rmodel の DDD 用語・時相語彙を含む制御日本語の精度は測っていない。提案する構文は文献からの推論。
