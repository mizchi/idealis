# CASE の TLC 対照

2026-11-16 に TLC v1.7.4、revision `5a47802b5c391f59ecdd44117981f4ff8c0656ba` で取得したログ。jar の SHA-256 は `expected.json` に記録し、既存の `tlc-strings` fixture と同じ jar を使った。

入力は `evals/tlc-differential/corpus.ts` の `goldenFrontendParity` を宣言した8件。TS / Rust frontend の IR を比較し、native / Wasm の結果を同じ TLC golden と照合する。

| ケース | TLC 判定 | 状態数 | 生成数 |
| --- | --- | ---: | ---: |
| `case-first-true` | success | 3 | 4 |
| `case-expression-no-match` | error | — | — |
| `case-action-no-match` | error | — | — |
| `case-unevaluated-branch` | success | 1 | 2 |
| `case-invariant-empty-init` | empty | 0 | 0 |
| `case-action-other` | success | 1 | 2 |
| `case-action-short-circuit` | success | 1 | 2 |
| `case-action-unevaluated-branch` | success | 1 | 2 |

source と config をそれぞれ隔離した scratch のケースディレクトリに `Differential.tla` / `Differential.cfg` としてそのまま保存し、各ディレクトリで次を実行した。`TLC_JAR` は pinned jar の絶対パス、`TLC_TMP` はケースごとに分離した一時ディレクトリ。JVM は1つずつ起動した。

```sh
java -Xmx256m -Djava.io.tmpdir="$TLC_TMP" -cp "$TLC_JAR" tlc2.TLC -workers 1 -nowarning Differential.tla
```

stdout と stderr を同じストリームへ保存し、次の置換だけを順に適用して `.log` に保存した。`temporary_directory` と `case_directory` は取得時に使った絶対パスで、パスの置換は Python の `str.replace`、残りは `re.sub` による。

| パターン | 置換後 |
| --- | --- |
| `str(temporary_directory)` | `<tlc-tmp>` |
| `str(case_directory)` | `<capture>` |
| `\[pid: \d+\]` | `[pid: <pid>]` |
| `\bon \d+ cores\b` | `on <cores> cores` |
| `\([^\n]*, (MSBDiskFPSet, DiskStateQueue)\)` | `(<host>, \1)` |
| `\b\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\b` | `<time>` |

ホスト説明の OS・architecture・JVM は置換し、FP set と queue の名前は残した。TLC のメッセージ、ソース位置、状態数・生成数、seed、fingerprint の行は変更していない。`expected.json` に source / config / 置換後 log の SHA-256、終了コード、判定、正常終了時の状態数・生成数、エラー時の文言と位置を記録する。式と action の未一致は終了コード75の評価エラーであり、正常終了や deadlock として扱わない。

最初の真の arm の選択、OTHER、後続 guard の短絡、未評価分岐は正常終了する。複数の真の arm がある action は後続 arm の状態を生成しない。空の Init では未一致 CASE の invariant を事前評価せず、TLC ハーネスと同じ `empty` を返す。これは `success` や非空の探索成功には読み替えない。[pinned TLC の CASE 評価](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/tool/impl/Tool.java)と同じ入力の実行結果を対照にする。
