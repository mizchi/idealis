# 0075-abba-complete

[0075-audit](../0075-audit/) の ABBA の driver は、最初の sample が失敗した bench を `runs.json` だけに残し、`bench-table.py` と `bench-summary.py` はその bench の行を出さず、エラーにもしなかった。記録した 0075 の run が実際にこの欠落を含んでいたかを確かめ、失敗を落とさない driver と report で ABBA を流し直した。

## 保存した 0075 の run の確認

- [0075-abba-1](../0075-abba-1/)、[0075-abba-2](../0075-abba-2/)、[0075-abba-3](../0075-abba-3/) の `runs.json` は、含まれる全 bench（abba-1 と abba-3 は 6 本、abba-2 は 3 本）で before、after、after、before の 4 sample がすべて exit 0 だった。
- abba-1 と abba-3 の表は 6 本すべての行を持ち、bench ごとの関数の数（compatibility 27、frontend 21、frontend_semantics 21、nesting_chains 12、quint_generated 42、recursive_actions 21、合計 144）は、この run の完全な表と一致する。
- 保存した 0075 の run では、bench は落ちていない。driver と report の不備は事実だが、iteration が引用する数値は変わらない。

## 新しい driver と report

`evals/experiments/nesting-chains/abba/` に置いた。

- `run.ts`: before、after、after、before の順に bench を 1 つずつ流す。binary の sha256 を manifest と照合する。sample が 1 つでも exit 0 以外なら、`runs.json` に残して終了コード 1 で終わる。
- `report.ts`: 期待する bench の名前を引数で受ける。bench ごとに、4 つの記録が before、after、after、before の順で全て exit 0、4 つの sample すべてに Criterion の estimates があり、関数の集合が 4 つで一致することを要求する。満たさない bench は `INCOMPLETE <bench>: <理由>` として表示し、終了コード 1 にする。期待していない bench が記録に混ざっていても同じ。
- 判定は `samples.ts` の純関数で、`tests/nesting-chains/abba-report-test.ts` が、失敗した最初の sample、後の sample の失敗、順序違い、記録のない bench、estimates が空か関数の集合が違うもの、期待していない bench を検査する。

## 流し直し

条件は 0075 と同じ（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）。before は 9488851d6 に 0075 の `nesting_chains` bench を置いたもの、after はこの branch で、`cargo bench --no-run` で別々の target に作った（sha256 は [binaries-sha256.json](binaries-sha256.json)）。rtla-compiler の 6 本の bench、約 17 分。aarch64-apple-darwin、rustc 1.100.0。

- full: 6 本すべてが 4 sample 完了し、report は終了コード 0。144 関数。**10% 以上の後退は 17 件**（最大 +24.7%）。開始時の load average が 10 と高く、他の作業と並行していた。17 件は `nesting_chains` の compile と check の小さい n、`recursive_actions` の compile と check と prepared、`frontend-check_fold-recursive` に分布した。compile は frontend の source が before と after で同一なので、変更では説明できない。
- repeat: `nesting_chains` と `recursive_actions` を同じ binary で流し直した。33 関数、10% 以上の後退は 0 件、最大 +8.7%。full の 17 件のうち再現したものはない。
- null-control: before の binary を複製して after に置き、同じ 2 本を流した。33 関数、10% 以上は 0 件、最大 +5.3%。同じ binary の ABBA の揺れの大きさになる。

判断: full の 17 件は repeat で再現せず、load の高い測定の揺れとして扱う。0075-abba-3 の結論（修正後に 10% 以上の再現する後退なし）は変わらない。この run は、完全性を検査する driver と report の動作確認と、abba-3 に欠落がなかった確認である。full の表を最終の性能の数値には使わない。

- [full-table.txt](full-table.txt)、[repeat-table.txt](repeat-table.txt)、[null-control-table.txt](null-control-table.txt)
- 各 bench の `runs.json`、`reduced.json`（消した sample の数・バイト数・digest）
