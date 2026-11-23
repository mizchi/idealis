# 0077: 増分contextの固定費の削減

0074の考察で挙げたB（assertionをGitのcommitのような連鎖にし、assert/popを定数時間にする）は、実装前のprofileで効果がほぼないと分かった。`context-scopes-32`のpopはほぼ0%で、時間の61%は最初の`Solver::parse`だった。profileで見えた費用に合わせて、mizchiの判断でB'に置き換えた。応答・work/stats・cacheのkeyは変えていない。

1. 長さは構文木から数え、hashはrenderしたbyteを直接hasherへ流す。文字列を作らない。
2. source hashは、assertごとに親へつないだリンク（`source.rs`）で持つ。cacheのkeyが必要になった時に、計算済みの最も近い祖先から計算する。Scriptのqueryもhash状態の代わりにリンクを保持する。連鎖はここだけで使い、keyの定義は変えていない。
3. `Problem`は最初のcheckで作る。
4. 新しい整数定数を束縛した時は、その名前を含むassertionだけを再コンパイルする。

solver benchは再測定（0077a）で全18ケースが閾値内になり、採用する。cache benchは`prepared-4`の1組が閾値を超え、後退として記録した。

## 経過

| 段階  | solver bench                                                | cache bench                                   |
| ----- | ----------------------------------------------------------- | --------------------------------------------- |
| 0077  | `hold`: 7ケースが片方の組で超過（変更していない経路を含む） | 不完全: candidateのbinaryのコピーに失敗した   |
| 0077a | `adopt`: 全18ケース                                         | `hold`: `prepared-4` 開発組1.127、確認組0.976 |

0077の開発組では、`prepared-32`（`Problem::solve`の経路で、変更していない）が1.378倍だった。candidateのrunだけ区間の幅が3.83 µsで、baselineは0.24 µsだった。`parse-solve-32/128`も同じrunだけ区間が広かった。確認組では`script-prepared-*`（`Script::solve`の経路で、変更していない）が1.11倍前後だった。どのケースも、両方の組で同じ向きには超えていない。負荷平均は2.5〜4.8で、WindowServerとChromeが動いていた。planで認めていた1組の再測定を、[0077a](../plans/0077a-smt-lean-remeasure.md)で宣言してから行った。

cache benchはcandidateのbench binaryのパスを取れず、baselineの2 runだけが残った。zshの変数`path`が`PATH`と連動していて、`cp`が見えなくなったのが原因。この2 runは比較に使っていない。

0077aのcache benchの`prepared-4`は、0.56 µsから0.64 µsだった。区間はどのrunでも幅0.01 µs以下で、確認組では0.976倍だった。同じケースはsolver benchでは0.953/0.864倍。`Problem`を`OnceLock`経由で取り出す分岐が加わったが、この経路でほかの差は見つけていない。planに従い、後退として記録して再測定しない。

## 条件

Criterion 0.8.2、20 samples、warmup 1秒、measurement 2秒、median、95%区間。baselineは3dd74cb5b（0076の記録まで）、candidateはaa48c557e。両benchとも同じsourceからstrip=noneでビルドし、保存したbinaryを再利用した。順序はbaseline→candidate、candidate→baseline。Apple M5で、専用ホストではない。

## 結果

[0077aのsolver比較](../runs/0077a-smt-solver-comparison/result.json)の中央値（µs）と、candidate/baselineの比。

| ケース                   | baseline | candidate | 開発比 | 確認比 | 閾値 |
| ------------------------ | -------: | --------: | -----: | -----: | ---: |
| `context-assert-256`     |   493.35 |    291.76 |  0.591 |  0.595 | 0.85 |
| `context-constant-256`   |   631.45 |    299.42 |  0.474 |  0.479 | 0.85 |
| `context-assert-32`      |    53.86 |     33.45 |  0.621 |  0.627 |  1.1 |
| `context-constant-32`    |    71.12 |     35.69 |  0.502 |  0.513 |  1.1 |
| `context-scopes-32`      |   111.36 |     72.52 |  0.651 |  0.637 |  1.1 |
| `script-parse-repeat-64` |    94.56 |     74.51 |  0.788 |  0.783 |  1.1 |
| `script-parse-scopes-32` |    51.91 |     47.47 |  0.915 |  0.927 |  1.1 |
| `script-parse-ite-64`    |    22.43 |     20.58 |  0.918 |  0.927 |  1.1 |
| `parse-solve-4`          |     5.57 |      5.51 |  0.989 |  0.995 |  1.0 |

残る9ケースは0.864〜1.032倍。`context-constant-256`は`context-assert-256`とほぼ同じ時間になった。定数を1つ束縛しても、その名前を含む1式だけを再コンパイルするため。0074から続いていた単発の小さいparseの後退（1.03〜1.06倍）はなくなった。

[0077aのcache比較](../runs/0077a-smt-cache-comparison/result.json)は、`prepared-4`の開発組を除き0.884〜1.070倍。

Wasmは520,271 bytesから515,044 bytes（-5,227）。

## 正しさ

- 全再解析の参照実装との差分テスト（400系列）に、引用符付きsymbol、`let`による同名の束縛、引用符付きの定数束縛を加えた。各checkで、全再解析のSolverが書いたentryを増分のSolverが読めること（keyの一致）も検査する（500件超）。
- 49,000回のassertの連鎖で、keyの一致と、256 KiBのstackでのdropを検査した。iterativeなDropを外すとstack overflowで落ちることを確かめた。
- 人工故障3件（名前の一致した式を再コンパイルしない、hashで最後のassertを落とす、長さで引用符の2 byteを数えない）をすべて検出した。
- 保存run: 開発seed 0x77a1、確認seed 0x77c3。3dd74cb5bのnative/Wasmとの全応答（work/statsとerror）が一致した（確認145要求、context 1536要求）。contextのZ3照合は207 status・146モデル、cacheのhit/missの照合は96/224件。両runを[開発](../runs/0077-smt-correctness-development/result.json)と[確認](../runs/0077-smt-correctness-confirmation/result.json)として再生した。

## 残り

- cache benchの`prepared-4`の後退（開発組1.127）。追加で判断するなら、新しい追記planで再測定する。
- Scriptの`assert`は、構文木をrenderした文字列をSessionで解析し直している。構文木を直接渡せば、`script-parse-*`の解析をさらに減らせる見込み。
- 0074の考察のC（祖先のUNSATやSATモデルの再利用）は、hitが新しい探索の回答と完全に一致するという約束を変えるため、別に判断する。

```sh
just rusmt-test
just smt-incremental-pair 0077-smt-lean-context solver <baseline> <candidate> <candidate-confirmation> <baseline-confirmation> <new-output>
just smt-incremental-replay evals/runs/0077-smt-correctness-confirmation
```
