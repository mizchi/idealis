# 0074: SMT contextの増分解析とcheck cache

0042で次の候補に残した、assertごとの全体再解析を除いた。assertは追加した式だけを解析・コンパイルし、popはpush時のsnapshotへ戻す。その上に、rtlcの状態展開cacheと同じ形の既定無効のcheck cacheを追加した。前者は応答を変えない変更として採用し、後者は明示的に有効にした場合だけ使う任意機能として採用する。

## 仮説と条件

増分解析は、全再解析と同じstatus・モデル・work/stats・errorを返したまま、assertを重ねるcontextとscriptの解析費用を下げる。check cacheは、同じ正規化sourceとassumptionsのcheckで探索を省き、hitを新しい探索の回答と一致させる。missでは固定費が増える。

比較は0042と同じCriterion設定（20 samples、warmup 1秒、measurement 2秒）。Apple M5、macOS 26.6.2、rustc 1.98.0で測った。改善前はbcf018be1のライブラリで、両版とも同じbenchを使う。benchには増分assertの4ケースとcacheの9ケースを足して27ケースにした。専用ホストではない。0042のようなplanの事前保存、run directoryへのraw samples保存、逆順の再測定はしていない。以下の数値は1回の測定の中央値で、外挿しない。

## 増分解析

無条件の整数定数は全assertionへ代入する。後から`(= y 3)`を足すと、それより前の`(<= (- x y) 2)`のコンパイル結果も変わる。新しい定数を束縛する式だけは、保持した構文木から全assertionをコンパイルし直す。tokenizeはやり直さない。

| ケース                   | 改善前 µs | 改善後 µs |  倍率 |
| ------------------------ | --------: | --------: | ----: |
| `context-assert-32`      |    717.05 |     45.90 | 15.62 |
| `context-assert-256`     |  47883.48 |    437.24 | 109.5 |
| `context-constant-32`    |    743.91 |     62.15 | 11.97 |
| `context-constant-256`   |  48044.54 |    570.36 | 84.23 |
| `context-scopes-32`      |   2720.61 |     92.97 | 29.26 |
| `script-parse-repeat-64` |    862.12 |     87.14 |  9.89 |
| `script-parse-scopes-32` |    174.28 |     49.19 |  3.54 |
| `script-parse-ite-64`    |     22.40 |     20.85 |  1.07 |
| `parse-solve-4`          |      5.41 |      5.50 |  0.98 |
| `parse-solve-128`        |    355.10 |    365.39 |  0.97 |
| `prepared-128`           |    184.52 |    187.18 |  0.99 |

単発のparse+solveは1〜3%遅い。最初の版では変数表を`Problem`用に複製しており、共有へ変えた後の値を載せた。解析済みの`prepared`も0.98〜1.02で揺れ、0042の後退許容10%に収まる。差が測定のばらつきかどうかは、この1回の測定では区別していない。

正しさは全再解析の参照実装との差分で検査した。参照は旧Solverと同じく、正規化したsourceへassertを連結して毎回`Problem::parse`する。固定seedの400系列、各24操作で、assert・push・pop・未宣言/Intのassumptions・予算0/7/40/100000のcheckを1000回以上照合した。累積sourceのtoken上限、1 MiBちょうどの受理と1 byte超過の拒否、入れ子65段も同じ回・同じメッセージで失敗する。このテストは旧実装でも通ることを確認した。

人工故障5件（定数追加時の再コンパイル省略、入れ子の境界、token数のずれ、改行1 byteのずれ、1 MiBの比較演算子）はすべて検出した。最初の版では、token数と改行1 byteのずれを見逃した。上限に達することだけを検査していたためで、参照と同じ回に同じerrorになることを検査する形へ直した。直した後の最初の実行では、参照側が宣言を改行なしで連結していて1 byteずれていた。旧Solverの正規化に合わせたのは参照側で、実装は変えていない。

## check cache

keyは正規化sourceのSHA-256と、順序・極性・symbolを含むassumptions。sourceのdigestはassertごとに増分で更新し、popではsnapshotと一緒に戻す。保存するのは`sat`/`unsat`だけ。namespaceはformat・version・build digestで、entryごとのSHA-256と、宣言に合うモデルの形を初回のlookupで検査する。hitは今回の探索をしないため、workは0。

| ケース                                    |                     µs | 比較                              |
| ----------------------------------------- | ---------------------: | --------------------------------- |
| `prepared-4` / `cache-miss-4`             |            0.55 / 2.99 | missは約5.4倍                     |
| `prepared-32` / `cache-miss-32`           |          13.54 / 17.55 | 約1.30倍                          |
| `prepared-128` / `cache-miss-128`         |        176.34 / 185.58 | 約1.05倍                          |
| `script-prepared-repeat-64` / cold / warm | 896.36 / 73.56 / 56.74 | 同じqueryの反復で同じrun内でもhit |
| `script-prepared-scopes-32` / cold / warm |    13.17 / 8.08 / 5.00 |                                   |
| `script-prepared-ite-64` / cold / warm    |   13.65 / 11.44 / 7.32 |                                   |

missの固定費は1 checkあたり約2.4〜4 µs。小さい問い合わせでは探索より大きいので、既定は無効にする。3つのscript fixtureは同じqueryを繰り返すため、coldでも同じrun内でhitする。miss専用の費用はcache-missのケースで測った。

`sha2`を加え、Wasmは401,224から479,837 bytes（+78,613、+19.6%）になった。

差分テストの400系列で1つのcacheを共有し、hit 200回以上で、無制限の新しい探索の回答と一致した。missでは結果とwork/statsが参照と一致した。Rustのテストは既定無効、unknown/errorの非保存、build digestの形式と無効化、保持上限0、snapshotの復元・改ざん・未知field、merge・容量・namespace違い・同じkeyの異なる回答、Scriptの共有予算、ScriptとSolverの間のhit、JSON requestを検査する。

人工故障7件（assumptionの極性をkeyから除く、unknownを保存、digest検査の除去、モデル形状の検査の除去、build digestによる無効化の除去、容量の無視、assertでsource digestを更新しない）はすべて検出した。最初はモデル形状の検査の除去を見逃した。改ざんしたentryは先にdigestで止まるためで、正しく再署名した形の合わないentryと、合う対照を単体テストに加えた。

native/Wasmは`tests/smt-cache`で同じrequest列（固定入力とseed 0x74c1の生成script）を流し、cold・warm・無効の全応答が一致した。coldは通常の`run-script`と、warmは回答が一致してworkは0、無効時は渡したsnapshotを変更しない。Chromiumでも`crossOriginIsolated === false`のまま、cold/warm/無効の統計と回答を確認した。

## 採用範囲

増分解析は既定の経路として採用する。check cacheは任意機能として追加し、既定は無効にする。回答の再利用で、学習状態・理論状態の再利用や探索の中断・再開ではない。keyはSHA-256のみでsource全文を保持しないため、衝突がないことは前提とする。entryのdigestは破損検出で、署名ではない。公開backendへの接続、永続storeとCAS、Z3との新しい照合はしていない。既存のZ3照合（0040〜0042）の入力に対する、新しい版での再実行もしていない。

```sh
just rusmt-test
just rusmt-criterion
just smt-cache-test
just smt-browser
```
