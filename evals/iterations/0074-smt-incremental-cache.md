# 0074: SMT contextの増分解析とcheck cache

0042で次の候補に残した、assertごとの全体再解析を除いた。assertは追加した式だけを解析・コンパイルし、popはpush時のsnapshotへ戻す。その上に、rtlcの状態展開cacheと同じ形の既定無効のcheck cacheを追加した。増分解析は応答を変えない変更として採用し、cacheは明示的に有効にした場合だけ使う任意機能として採用する。solver benchの最終測定（0074b）は`prepared-128`の1組で閾値を超え、`hold`のまま記録した。

## 経過

実装を先に進め、探索的なCriterionを1回測った後で、0042と同じ水準の評価を[plan](../plans/0074-smt-incremental-cache.md)として固定した。探索値は採否に使っていない。閾値も探索値に合わせず、0042と同じ形にした。保存runでは2回の後退を見つけ、そのたびに追記planを先に書いてから測り直した。

| 段階  | 変更                                 | solver bench                              | cache bench                            |
| ----- | ------------------------------------ | ----------------------------------------- | -------------------------------------- |
| 0074  | 増分解析とcache（32fcaca19）         | `hold`: `script-parse-ite-64` 3.52/3.71倍 | 合否なし                               |
| 0074a | keyを使う時だけ計算（fbbaf5f19）     | `adopt`: 全18ケース                       | warmがcacheなしの4.6〜4.8倍に後退      |
| 0074b | 初回に計算したkeyを保持（8c134bd7f） | `hold`: `prepared-128` 確認組1.235倍      | `adopt`: warmはcacheなしの0.06〜0.53倍 |

0074の後退は、cacheを使わない`Script::parse`でも各checkのsource digestとkeyのSHA-256を作っていたことによる。ite-64はassertが少なくcheckが64回あるため、この固定費が支配的になった。[0074a](../plans/0074a-smt-lazy-cache-keys.md)でkeyの計算を`solve_incremental`へ移すと、parseは戻った。しかし反復のたびにkeyを作り直すようになり、warmのcacheがcacheなしより遅くなった。0074はcache benchを合否にしていなかったので、[0074b](../plans/0074b-smt-memoized-cache-keys.md)でwarmをcacheなしの1.0倍以下とする条件を測定前に加え、queryごとに初回のkeyを保持した。

0074bの`prepared-128`は、開発組0.965倍、確認組1.235倍だった。確認組のcandidate runは中央値の95%区間が202〜256 µsと広く、最小値187.5 µsは開発組の中央値185.6 µsと同程度だった。0074bの変更は`Script`のkey保持だけで、`Problem::solve`の経路にはない。0074と0074aでは同じケースが0.94〜0.97倍だった。ただしplanで再測定しないと決めたので、この1組を後退として記録する。

## 条件

Criterion 0.8.2、20 samples、warmup 1秒、measurement 2秒、median、95%区間。Apple M5、macOS 26.6.2、rustc 1.98.0。専用ホストではない。baselineはbcf018be1のライブラリに今回と同じ`benches/solver.rs`とfixturesを組み合わせ、strip=noneのbench binaryを3段階すべてで再利用した。solver benchは両版でビルドできる18ケースで、baseline→candidate、candidate→baselineの順に測った。cacheの9ケースと対照の6ケースは`benches/cache.rs`に分け、candidateの開発・確認を測った。各runにraw samples、binary・source・lockのhash、負荷を保存した。

## 増分解析

無条件の整数定数は全assertionへ代入する。後から`(= y 3)`を足すと、それより前の`(<= (- x y) 2)`のコンパイル結果も変わる。新しい定数を束縛する式だけは、保持した構文木から全assertionをコンパイルし直す。tokenizeはやり直さない。

[0074bの比較](../runs/0074b-smt-comparison/result.json)の中央値（µs）と、candidate/baselineの比。

| ケース                   | baseline | candidate | 開発比 | 確認比 | 閾値 |
| ------------------------ | -------: | --------: | -----: | -----: | ---: |
| `context-assert-256`     | 48305.56 |    508.07 |  0.011 |  0.011 |  0.5 |
| `context-constant-256`   | 49101.75 |    636.55 |  0.013 |  0.013 |  0.5 |
| `context-scopes-32`      |  2702.55 |    113.70 |  0.042 |  0.042 |  0.8 |
| `script-parse-repeat-64` |   857.41 |     95.76 |  0.112 |  0.112 |  0.8 |
| `script-parse-scopes-32` |   175.20 |     52.30 |  0.298 |  0.301 |  0.9 |
| `context-assert-32`      |   716.08 |     55.01 |  0.077 |  0.078 |  1.1 |
| `context-constant-32`    |   772.44 |     70.45 |  0.091 |  0.096 |  1.1 |
| `script-parse-ite-64`    |    22.33 |     22.28 |  0.998 |  1.006 |  1.1 |
| `parse-solve-4`          |     5.43 |      5.77 |  1.062 |  1.029 |  1.1 |
| `prepared-128`           |   192.30 |    185.58 |  0.965 |  1.235 |  1.1 |

残る8ケースは0.961〜1.023倍。0074aでも全18ケースが閾値内で、`parse-solve-4`は1.037/1.037倍だった。単発の小さいparseは数%遅い傾向が3段階で続いた。

## check cache

keyは正規化sourceのSHA-256と、順序・極性・symbolを含むassumptions。sourceのhash状態はassertごとに更新し、popではsnapshotと一緒に戻す。Scriptのqueryは最初のcached solveでkeyを作って保持し、parseと通常のsolveはkeyを作らない。保存するのは`sat`/`unsat`だけ。namespaceはformat・version・build digestで、entryごとのSHA-256と、宣言に合うモデルの形を初回のlookupで検査する。hitは今回の探索をしないため、workは0。

[0074bのcache比較](../runs/0074b-smt-cache-comparison/result.json)。開発/確認の中央値（µs）と、cacheなしに対する比。

| ケース                        | cacheなし       | cacheあり       | 比            |
| ----------------------------- | --------------- | --------------- | ------------- |
| `cache-miss-4`                | 0.56 / 0.58     | 3.06 / 3.02     | 5.432 / 5.221 |
| `cache-miss-32`               | 14.09 / 14.05   | 18.07 / 18.07   | 1.283 / 1.286 |
| `cache-miss-128`              | 187.34 / 184.02 | 194.71 / 196.44 | 1.039 / 1.067 |
| `script-cache-warm-repeat-64` | 915.03 / 918.40 | 56.23 / 56.11   | 0.061 / 0.061 |
| `script-cache-warm-scopes-32` | 13.22 / 13.32   | 5.09 / 5.29     | 0.385 / 0.397 |
| `script-cache-warm-ite-64`    | 14.14 / 13.85   | 7.36 / 7.41     | 0.521 / 0.535 |

missの固定費は1 checkあたり約2.5〜12 µsで、小さい問い合わせでは探索より大きい。既定を無効にした理由はこれ。3つのscript fixtureは同じqueryを繰り返すので、coldでも同じrun内でhitする（cold比0.081〜0.840）。

`sha2`を加え、Wasmはbcf018be1の393,832 bytesから482,031 bytes（+88,199、+22.4%）になった。

## 正しさ

0074bのcandidateで、開発seed 0x74a1と確認seed 0x74c3を実行し、両runを再生した。

| 検査                                           | 開発      | 確認      |
| ---------------------------------------------- | --------- | --------- |
| smt-scripts（script / check）                  | 35 / 300  | 67 / 588  |
| 旧native・旧Wasmとの完全応答（work/stats含む） | 113要求   | 145要求   |
| 4 fixtureのZ3照合とSATモデル                   | 193 query | 193 query |
| context系列 / 要求                             | 32 / 768  | 64 / 1536 |
| contextのZ3 status / モデル再検査              | 78 / 51   | 153 / 107 |
| contextのerror / unknown                       | 193 / 62  | 407 / 142 |
| contextのcache hit / miss                      | 30 / 110  | 50 / 245  |
| scriptのcache（warm hit）                      | 300       | 588       |

context系列は宣言・式のpool・assert/push/pop/check/check-assumingをseedから生成し、新旧のnative・Wasmの4つで全応答を照合した。後から整数定数を束縛する式、未対応式、型エラー、未宣言のassumption、予算0を含む。cacheのhitは同じqueryの無制限な新しい探索の回答と一致し、missは通常の応答と一致した。runは[開発](../runs/0074b-smt-correctness-development/result.json)と[確認](../runs/0074b-smt-correctness-confirmation/result.json)。

Rustでは、全再解析の参照実装と固定seedの400系列、1000回超のcheckで結果・work/stats・errorを照合した。累積sourceのtoken上限、1 MiBちょうどの受理と1 byte超過の拒否、入れ子65段も同じ回・同じメッセージで失敗する。この差分テストは旧実装でも通ることを確認した。

人工故障は増分解析5件、cache 7件をすべて検出した。最初は、token数のずれ、改行1 byteのずれ、モデル形状の検査の除去を見逃した。上限に達することだけを見ていた検査を参照と同じ回・同じerrorの照合へ直し、再署名した形の合わないentryと合う対照を単体テストへ加えた。参照側が宣言を改行なしで連結していた1 byteのずれは、旧Solverの正規化に合わせて参照側を直した。

native/WasmとChromiumでは、cold・warm・無効の統計と回答を確認した。

## 採用範囲と残り

増分解析は既定の経路として採用する。check cacheは任意機能として既定無効で採用する。回答の再利用で、学習状態・理論状態の再利用や探索の中断・再開ではない。keyはSHA-256のみでsource全文を保持しないため、衝突がないことは前提とする。entryのdigestは破損検出で、署名ではない。公開backendへの接続、永続storeとCASは扱っていない。

- 0074bの`prepared-128`は後退として記録したまま。追加で判断するなら、新しい追記planで再測定する。
- Jevのrusmt 4比較は未記録。API keyのない環境で実行したため`support-sync`は判定を作らず、offlineの再生は0/4だった。
- 確認runは19 MB、開発runは5.7 MB。check-incrementalの各応答が成長するcache snapshotを含むためで、0042の確認run（1.5 MB）より大きい。

```sh
just rusmt-test
just smt-incremental-test
just smt-incremental-eval confirmation <new-run> .artifacts/smt-incremental/baseline <development-run>
just smt-incremental-replay evals/runs/0074b-smt-correctness-confirmation
just smt-incremental-compare solver <baseline> <candidate> <candidate-confirmation> <baseline-confirmation> <new-output>
```
