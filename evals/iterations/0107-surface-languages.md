# 0107: モデルの書き方の比較

[plan](../plans/0107-surface-languages.md)のとおり、5つの題材を7つの書き方で書き、各toolchainで検査した。比較表と各書き方の詰まりどころは[evals/experiments/surface-languages/README.md](../experiments/surface-languages/README.md)。7つすべてで、書けた性質の判定はSPECと一致した。

## 計画からの差

- Isabelleはbrewのcaskが無効（Gatekeeper）だったので、mizchiの判断で公式の配布物（Isabelle2026、Apple Silicon）を`~/opt/isabelle2026`に展開した。`.app`のまま展開すると消えたので、中身だけを普通のdirectoryに置いた。
- Quintの活性は、書き手の環境にJavaがなく未検査だったので、筆者がrepoのTLC用のJREで`quint verify --backend tlc`を実行した（fairnessありで成り立ち、なしで反例）。
- 読みやすさ（新しい読み手）は、まだ測っていない。

## 分かったこと

1. **表現で詰む場所は、書き方の系統で決まる。**
   - 関係の言語（rmodel）は、直和型とmatch、二状態の性質、活性が書けない。
   - SMTに渡す言語（Veil）は、ホスト（Lean）が書けても、SMTの断片（`match`・`def`・`Option`・`#`）で詰む。書き手は、黙って誤ったtraceを返す例を報告した。
   - 証明系（Isabelle）は、すべて書けてすべての大きさで証明できたが、証明を書く手間がある（sessionsは73行、活性の証明は約30行）。自動の反例探し（nitpick）は当てにならない場面があった。
   - 依存型（Idris 2）は、型で性質を述べられるが、証明はほとんど穴で、行数が最も多い。
   - MLへの埋め込み（OCaml）は、直和型・match・網羅性の検査がそのまま使え、短い。
   - Pklは、joinがなく冗長だが、型付きのASTとしてそのままIRに写せる。
2. **ホストの関数で書くと、モデルが解析できなくなる。** OCamlとIdris 2では、guardと更新がホストの関数なので、検査器は実行するだけで中を見られない（BFSはできるが、SMTの帰納法、型の生成、実装の照合の入力にならない）。Veilはmeta programmingで中を見るが、見えない部分（`def`・`match`）で誤る。Pkl・Quint・rmodelは解析できる木を残す。
3. **直和型とmatch、二状態の性質、fairness付きの活性は、ML系・証明系・Quintのすべてで直接書けた。** 今の核に欠けているのは、この3つと`iff`。

## 次の仮説

表の構文はML系（直和型・`match`・`let`・有限のデータの上の関数）に寄せ、核は解析できる一階の木のまま保つ（ホストの任意の関数を許さない）。核に直和型と`match`・二状態の性質（`next`）・fairness付きの活性・`iff`を足す。読みやすさは、ML風の候補とQuint・rmodelを、新しい読み手で比べる。
