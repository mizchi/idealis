# 実行可能な形式仕様と外部証明

## 自然言語の要求、ドメイン条件、モデル検査と証明をどう結んだか

### Takeaway

最も近い開発事例は _ProR + iUML-B/Event-B + Rodin + ProB_ を実際に使った Hoang らの事例である。自然文と式の対応を追跡リンクに置き、同じモデルに対して要求の種類ごとに証明、モデル検査、シナリオ確認を使う。

### Cited Findings

- Hoang, Snook, Salehi, Butler, Ladenberger (2018), _Validating and Verifying the Requirements and Design of a Haemodialysis Machine using the Rodin Toolset_, Science of Computer Programming 158, 122–147。ProR で自然文の要求を分割・番号付けし、対応するモデル要素を参照する。iUML-B の状態機械・データ表現を Event-B へ統合する。§2 の方法では、対象の性質を表す不変条件を Rodin で証明し、順序付きプロセスはアニメーションで確認し、タイミング条件は ProB の LTL で検査する。§3.3 の具体例は `SF(Response) ⇒ G([Trigger] ⇒ F[Response])`。基本の Event-B refinement で活性を継承したとは扱わず、各 refinement ごとに ProB で再検査した。§3.4 はドメイン固有のアニメーション UI による確認。 [著者版 PDF](https://www.southampton.ac.uk/~tsh2n14/publications/journals/hdmachine-scp2017.pdf) · [DOI](https://doi.org/10.1016/j.scico.2017.11.002)
- Jastram ら（2011）の WRSPM + ProR は自由記述要求と Event-B を異なる成果物のまま維持し、`realize` を複数種類の追跡関係に分けた。証明義務はモデルを検査し、どの式がどの要求を実現するかは当時手動でリンクした。ProR のプロキシは対応する Event-B 要素に同調する。統合仕様では、リンクの片側が変更したら `changed` とし、検証済み状態へ戻すのは人間の操作である。 [要求トレーサビリティ論文 §4.1](https://stups.hhu-hosting.de/downloads/pdf/tr-JaGr2011.pdf) · [ProR/Rodin 統合仕様、Link Management](https://wiki.event-b.org/images/Pror-integration.pdf)
- Tueno, Laleau, Mammar, Frappier (2017 preprint), _Formal Representation of SysML/KAOS Domain Model_。ゴール階層は機械とイベントの骨格を構成し、ドメインオントロジーの概念、関連、多重度などから Event-B の構造部分を生成する。変換規則を Event-B/Rodin で表現・検証し、着陸装置をケースにした。当時のゴール変換はイベント本体を埋めず、オントロジー変換の主目的は状態・構造の補完。 [著者論文 §§2.3–2.4, 4](https://arxiv.org/abs/1712.07406)
- Bjørner (2017), _Manifest domains: analysis and description_。人間向けの叙述とモデルの形式化を対にし、存続するモノと、時間的なアクション・イベント・振る舞いを扱う Domain Engineering。RAISE の RSL を使う設計論で、Evans の DDD 戦略パターンや、自由文を形式式に自動変換するツールの実証ではない。 [著者論文書誌と全文](https://orbit.dtu.dk/en/publications/manifest-domainsanalysis-and-description/) · [RSL と叙述の説明](https://www.imm.dtu.dk/~dibj/domains/assc-2014-bjorner.pdf)
- NASA の FRET は FRETish という制限自然言語で scope, condition, component, timing, response を組み、未来・過去の時相論理と図・シミュレーションへ変換する。Conrad, Titolo, Giannakopoulou, Pressburger, Dutle (CPP 2022) は FRETish の表示的意味論を PVS で与え、対象断片の要求を満たすトレースと生成式を満たすトレースが一致することを証明した。論文は PVS の実行表現と FRET 実装の意味の比較にテストも使う。 [FRET 論文](https://ntrs.nasa.gov/api/citations/20200001989/downloads/20200001989.pdf) · [PVS 証明論文 §§4–7](https://arxiv.org/abs/2201.03641)

### Inferences

- idealis-spec の要件 ID と `ShippedStays` の結合を基礎に、根拠リンクへ「モデル定義」「検査対象」「参照トレース」「証明定理」を型付きで区別できる。リンク元の文章とリンク先のモデル宣言の版がどちらか変わると対応のレビューを失効させる方式は、文章の変更を検知する。FRETish のように限定したパターンなら、文章として書いた DSL の構文に決定的な意味を与え、同義の構文を正規化できる。いずれの場合も業務の意図を定義するのは読者であり、モデル満足や翻訳正当性から自由文との同値は導けない。
- SysML/KAOS の構造と振る舞いの非対称性は rmodel でも参考になる。型・用語と文書表示を先に生成し、コンテキスト間の権限・操作・イベント写像・公平性は明示のモデルとして書く。論文のドメインオントロジーと DDD bounded context は同じ概念ではない。
- 発送試作では、自由文＋確認済み参照・制限文＋決定的な正規化・モデルからの逆方向の自然文生成を、現行の定義を固定して比較する。対照は同義語、否定、空の前件、誤った条件の時相、単語の意味が異なる別コンテキスト。FRET にならい有効例と違反例を可視化／ITF に出し、証明済み判定の範囲とは別に読者に確認してもらう。

### Gaps

- 確認した開発事例に Evans の DDD 戦略パターン、自由な要求文、実行可能仕様、証明を単一表面文法で提供した例はなかった。ProR は統合ワークフロー、SysML/KAOS はドメイン知識の構造的変換、FRET は制限文の意味論と読み返しを実証する。

## 一つのモデルに有限探索、機械証明、実装との照合をどう与えたか

### Takeaway

TLA+ は擬似コードの PlusCal、モデル検査の TLC、階層的証明 TLAPS の機械意味を TLA+ で共有する。Veil は Lean に埋め込んだ DSL の検査・帰納証明に加え、VC 生成そのものの健全性を証明する。実装の適合にまで議論を伸ばすには別の refinement や観測射影が要る。

### Cited Findings

- Lamport (ICTAC 2009), _The PlusCal Algorithm Language_。擬似コードに近い PlusCal を TLA+ へ変換し、TLC は定数等を具体化した検査可能なモデルで安全性・条件付き活性を検査する。ラベル間が一つの原子的なステップで、変換が program counter を導入する。Cousineau ら (FM 2012), _TLA+ Proofs_ はその同じ TLA+ の式に、証明義務の分解と外部プローバを使う TLAPS を適用し Peterson の相互排他を例示する。初期 TLAPS の論文は活性を対象外とし、後続版は PTL/LS4 による命題時相論理を追加した。表明できる TLA+ の活性一般と、特定版 TLAPS が証明できる範囲は区別する。 [PlusCal 論文 §4](https://lamport.org/pubs/pluscal.pdf) · [TLAPS 論文](https://lamport.org/pubs/tlaps.pdf) · [初期論文](https://arxiv.org/abs/1011.2560) · [TLAPS 変更履歴](https://proofs.tlapl.us/doc/web/content/Download/What_s_new.html) · [TLA+2 ガイド、PTL と量化制限](https://lamport.azurewebsites.net/tla/tla2-guide.pdf)
- Pîrlea, Gladshtein, Kinsbruner, Zhao, Sergey (CAV 2025), _Veil: A Framework for Automated and Interactive Verification of Transition Systems_。Lean 4 上のガード付き状態遷移 DSL。特定の操作列が存在／不可能かを BMC で確認し、帰納不変条件を初期状態と操作別保存に分割、SMT を使い、難しい場合は同じ条件を Lean の個別証明に渡せる。VC 生成の仕様意味論に対する健全性を証明する。著者は Ivy と Rocq の Rabia の不変条件の差異を共通モデル上への移植中に発見した。SMT が失敗／不明を返せる。論文で実証された中心は安全性と BMC であり、一般の活性の自動検証を支えたとは言えない。 [Veil 論文 §§2–4](https://link.springer.com/chapter/10.1007/978-3-031-98682-6_2)
- Veil の後続ツールは、コンパイルされた有限具体モデルの検査とシンボリック検査、Lean カーネルまでチェックする Lean-SMT を紹介している。CAV 論文での当時のデフォルトは Lean-auto と書かれているため、すべてのコマンドで外部証明が同一の方法で再検証されるという意味ではない。rmodel の既存評価 0107 は特定スナップショットで SMT 断片に入らない構文等を観察した。 [Veil 作者の Lean use case](https://lean-lang.org/use-cases/veil/) · [CAV 論文 §3.3](https://link.springer.com/chapter/10.1007/978-3-031-98682-6_2) · [rmodel 既存評価](../../evals/iterations/0107-surface-languages.md)
- Woodcock, Cavalcanti, Foster らの Circus は、状態の Z、通信・並行の CSP、Dijkstra の guarded commands、精密化計算を共有する。Gomes (2019) の Circus2CSP は対応する断片を CSPm に翻訳して FDR で検査した。Foster, Hur, Woodcock (2021) は Isabelle/HOL 上の Interaction Trees を CSP/Circus の実行意味として形式化し、failures-divergences モデルに接続し、Isabelle のコード生成で実行可能なシミュレータを作った。各成果の適用断片と意味を明示した方式で、任意の Circus を全機能で検査するとの主張ではない。 [統合の著者論文](https://eprints.whiterose.ac.uk/id/eprint/207862/) · [Circus2CSP 博士論文](https://www.tara.tcd.ie/tara8/server/api/core/bitstreams/3d097c7f-e2dd-44ee-8c4c-ba753ce1e924/content) · [検証済み実行意味の著者論文](https://arxiv.org/abs/2105.05133)
- Sprenger et al. (OOPSLA 2020), _Igloo_。抽象イベントモデルを component と環境へ refinement・分解し、イベントモデルの I/O を分離論理の契約へ変換する健全性を Isabelle/HOL で形式化した。Java/VeriFast と Python/Nagini 実装例で同じ上位モデルへ結ぶ。保証は環境・コード検証器などの仮定およびトレースの安全性の範囲。ITF/PBT による有限列の再生から実装全体の trace refinement が従うわけではないことを考える比較対象になる。 [Igloo 論文 §§3.8–3.10, 5](https://arxiv.org/abs/2010.04749)
- Wilcox et al. (PLDI 2015) の Verdi は Rocq/Coq のネットワーク意味論を選び、safety 保証を fault model 間で移す検証済み system transformer を使った。Raft 複製の線形化可能性の例を報告し、活性は将来課題と明示した。Woos et al. (CPP 2016) は Raft state machine safety と変化に耐える証明の構造化を扱う。 [Verdi 論文](https://homes.cs.washington.edu/~mernst/pubs/verify-distsystem-pldi2015.pdf) · [Raft 論文](https://homes.cs.washington.edu/~mernst/pubs/raft-proof-cpp2016-abstract.html)

### Inferences

- 既存 `lean-oracle` は列挙した有限観測に対する Lean の命題評価である。次の証明統合では rmodel の同じ型付き AST から `Init`、`Step`、`property` を Lean に出し、初期化と保存の定理を同じ要件 ID に合流させる手がある。小さな実験で Coq/Lean の手書き前提と AST 自動生成の両方を固定し、現行モデルの正常ケース、操作順序の故障、空初期状態、状態符号化の故障を評価する。自動生成する Lean の式を証明しても、rmodel と Lean の間の意味対応は別の性質であり、再生による差分検査と、翻訳保存の証明を別ステータスにする。
- 証明方法に依らず、BDD 的なシナリオ／ITF には有限で実行可能な観測、`invariant` には到達状態の安全性、`transition` には二状態の性質、`progress` 等には公平性を含む無限トレースを割り当てる。Raft のリーダー存在を出荷ステップの保存条件と一つの `rule` 宣言にまとめても、意味 IR の種類と検査の範囲を消さない。
- DDD 文脈間の API に取り入れるなら、コンテキスト間の同名語や内部状態を無条件で同じとせず、イベントの公開面と観測の抽象化の向きを定義する。通常の近道は逐次シナリオでは projection した ITF とコードの出力比較、過程全体へ保証を強める場合は初期状態と各遷移の simulation 条件。

### Gaps

- 現行 Veil における一般 LTL／活性の機械証明の対応範囲を断定する資料は確認していない。
- テスト対象の検索は網羅的レビューではない。構文例を各ツールで実行していない。
