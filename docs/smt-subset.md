# RustでSMT-LIBの一部を解く実験

整数の差分制約とBoolean式を解く独立した[`rusmt` crate](../crates/rusmt)を提供する。実験用crateから移し、nativeとホストimport不要のWasmを同じソースから生成する。idealisの他crateへ依存せず、idealisの公開backendや既存のZ3 adapterには接続していない。

Z3を再利用する比較でも、小さいTTL・amount・schedulerのnative問い合わせは約3.0-3.4倍速かった。Boolean UNSATでは約34倍遅い例と予算切れが残る。[比較条件と全17入力](../evals/iterations/0031-smt-reuse.md)を確認してから用途を判断する。

## 対応する式

SMT-LIBは入力形式で、対応範囲はその一部。[SMT-LIBの定義](https://smt-lib.org/logics.shtml)ではQF_IDLは整数の差分制約、QF_LIAは一般の線形整数式を扱う。この試作は`set-logic QF_LIA`も受け付けるが、定数代入後に差分制約へ変換できる式だけを解く。定数倍は同じ係数の差分へ正規化できる範囲に対応する。[対応チェックリスト](smt-checklist.md)で残りを追跡する。

| 項目     | 対応範囲                                                                                     |
| -------- | -------------------------------------------------------------------------------------------- |
| 宣言     | `Int` / `Bool`の`declare-const`、引数なしの`declare-fun`                                     |
| コマンド | `set-logic QF_IDL` / `QF_LIA`、`assert`、末尾の任意の単一`check-sat`、`:produce-models true` |
| 整数     | 任意精度の整数、加減算、定数倍、`<` / `<=` / `>` / `>=` / `=` / `distinct`                   |
| Boolean  | `and` / `or`、`not`、`xor`、`=>`、等価、Bool / Intの`ite`                                    |
| 束縛     | 同時束縛の`let`、ASCIIの引用識別子、コメント                                                 |
| 正規化   | 無条件の整数定数等式を代入し、`x - y <= c`へ変換。`k * (x - y) <= c`も整数の丸めで正規化     |

例えば次の式は`other = 3`を代入できるため、`amount = 7`を返す。

```smt2
(set-logic QF_LIA)
(declare-const amount Int)
(declare-const other Int)
(assert (= other 3))
(assert (= (+ amount other) 10))
(assert (> amount 0))
(check-sat)
```

`x + y = 7`だけでは両方が自由変数なので拒否する。`2*x + 3*y <= 7`も一般LIAなので対象外。Real、BitVec、配列、量化、非線形乗算、除算、剰余、引数のある未解釈関数は対象外。SMT-LIB scriptの`get-model/get-value`やscope内の宣言は対象外。`push/pop`と複数checkは下記のScript APIで実行する。

整数の`ite`はguard付きの選択肢へ変換する。算術・比較で選択肢が256を超えるとresource errorになる。各選択肢が対応する差分制約へ落ちる必要がある。空の`(and)` / `(or)`はZ3 5.4.1のSMT-LIB parserと同じく拒否する。

入力には1 MiB、20万token、4096宣言などの上限がある。構文や型の検査もサブセットに限る。一般のSMT-LIB validatorとしては使わない。

## 求解と結果の契約

Boolean式を分岐し、各枝の整数制約をBellman-Fordの負閉路検査で解く。負閉路があればその枝は矛盾する。整数を有限範囲で列挙してUNSATを推測する方法ではない。

- `sat`は全宣言の整数値と真偽値を返す。整数値はJSONの精度を保つため10進文字列にする。
- `unsat`は探索したすべての枝が矛盾した場合に返す。
- `unknown`は指定した`maxWork`の消費で探索を止めた場合に返す。理由は`work`で、モデル値は返さない。
- `error`は未対応の式、不正な入力、型や資源上限、protocol違反を表す。

workは式の処理と制約の緩和を数える。parserの費用やwall clockの上限ではない。時間切れは追加していない。節学習や探索途中の理論伝播がないため、Booleanの矛盾証明は指数的に増え得る。

## 実験用のAPI

Rustは`rusmt::Problem::parse(source)`と`Problem::solve(max_work)`を持つ。`Problem::solve_assuming(&[Assumption], max_work)`は一時的なBool literalを加えて解く。`Assumption { symbol, positive }`は宣言済みBool名と極性を表す。nativeの`rusmt` binaryと`rusmt.wasm`は共通の`Engine`を使う。[TS側のwire型](../evals/experiments/smt-subset/wire.ts)がリクエストと応答を定義する。

```json
{
  "version": 1,
  "request": {
    "kind": "solve",
    "source": "(declare-const x Int)(assert (= x 7))",
    "maxWork": 2000000
  }
}
```

`Solver::parse(source)`は変更可能なcontextを作る。`assert(expression)`、`push(count)`、`pop(count)`、`check(max_work)`、`check_assuming(assumptions, max_work)`を使う。宣言は初期sourceに固定する。push/popはassertionだけを追加・撤回する。変更に失敗した場合は元のcontextを保つ。

JSONの`prepare {source}`はcontextのhandleを返す。次のリクエストを使う。

| kind           | 追加フィールド                  | 動作                                                          |
| -------------- | ------------------------------- | ------------------------------------------------------------- |
| `assert`       | `handle, expression`            | 単一Bool式を追加する。コマンドや複数式は拒否する。            |
| `push` / `pop` | `handle, count`                 | scopeを保存・撤回する。成功時は`updated`を返す。              |
| `check`        | `handle, maxWork, assumptions?` | 現在の式を求解する。assumptionsは`{symbol, positive}`の配列。 |
| `release`      | `handle`                        | contextを破棄する。解放済みhandleはerror。                    |

scopeとassumptionsはそれぞれ最大256件。pop underflow、未宣言/Intのassumption、不正shapeはerrorになる。assumptionsはそのcheckだけで有効。次のcheckや別handleへ持ち越さない。

assertは追加した式だけを解析・コンパイルする。無条件の整数定数は全assertionへ代入するので、新しい定数を束縛する式を追加した場合は、保持した構文木から全assertionをコンパイルし直す。popはpush時点の不変なsnapshotへ戻す。source長・token数・入れ子の上限は累積したsourceに対して、全体の再解析と同じ順で検査する。探索・学習状態は再利用しない。回答の再利用は既定無効の[check cache](#完了したcheckを再利用する)で行う。SMT-LIB scriptの問い合わせ列は次のScript APIで扱う。

### SMT-LIB scriptをまとめて実行する

`Script::parse(source)`は全コマンドを検査し、各check時点の式を保持する。制約が同じcheckは不変な解析結果を共有する。`Script::solve(max_work)`はcheckの順に`Vec<Solution>`を返す。JSONは`run-script {source,maxWork}`を受け、`script-results {results:[...]}`を返す。単発の`solve`や既存handleの契約は変えない。

```smt2
(declare-const b Bool)
(check-sat-assuming (b))
(push 1)
(assert (not b))
(check-sat-assuming (b))
(pop 1)
(check-sat)
```

この入力はSAT、UNSAT、SATの3結果を返す。`check-sat-assuming`は宣言済みBool symbolか`(not symbol)`だけに対応する。空のassumptionsは通常checkと同じ。[SMT-LIB 2.7](https://smt-lib.org/papers/smt-lib-reference-v2.7-r2025-07-07.pdf)の任意Bool式への対応は今後の対象。

Int/Boolの宣言と既存のlogic/optionは冒頭に固定する。assert/push/pop/check後の宣言やoption変更、define-fun、reset、exitは拒否する。各assertを追加した時点で対応する差分制約へ正規化できる必要がある。checkは最大256回、scopeとassumptionsは各256件。push/popのcountはu32のnumeralで、0はno-op。

workは全checkで共有し、各結果の`stats.work`を差し引く。残り0のcheckはunknown/workと空モデルを返す。構文や型の不正は予算0でもerror。後続コマンドが不正なら部分的なSATを返さず、既存handleにも影響させない。checkのない入力は空の結果列で、仕様の検査が成功したとは扱わない。

各checkは探索をやり直す。Scriptを再求解しても前回の消費やassumptionsを持ち越さず、学習状態や中断地点の再利用は行わない。

### 完了したcheckを再利用する

[rtlcの状態展開cache](safety-expansions.md)と同じく、明示的に有効にした場合だけ完了した回答を再利用する。`Solver::check_incremental`と`Script::solve_incremental`は`CheckCache`と`CacheOptions { enabled, build_digest, max_entries }`を受ける。`CacheOptions::default()`は無効で、通常のcheck/solveへ委譲し、cacheを参照・更新せず、hit/miss/writeを0で返す。

| 項目      | 契約                                                                                                 |
| --------- | ---------------------------------------------------------------------------------------------------- |
| key       | 正規化したsource（各コマンドのrenderと改行）のSHA-256と、順序・極性・symbolを含むassumptions         |
| 保存      | `sat`の全モデルと`unsat`。`unknown`・errorは保存しない                                               |
| hit       | 新しい探索の回答と同じstatus/values/booleans。今回の`stats`は0で、予算0でも回答する                  |
| namespace | format、version、`build_digest`。digestが変わると既存entryを捨て、`invalidated`を返す                |
| 容量      | `max_entries`件。0なら書き込まない。満杯なら新しいentryを書かず、小さい上限ではkey順の末尾から落とす |
| 検証      | entryごとのSHA-256と、問題の宣言に合うモデルの形を初回のlookupで検査する。不一致はprotocol error     |

sourceのdigestはSessionがassertごとに増分で更新し、popではsnapshotと一緒に戻す。同じ正規化sourceなら、`Solver::parse`の空白・コメント・末尾の`check-sat`、assertで積んだcontext、Scriptの各checkの間でhitする。式の意味が同じでも、表記が違えば別のkeyになる。

Scriptのwork予算は全checkで共有する。hitは予算を消費しないため、cacheなしの`solve`ではunknownになった後続checkが、cacheありでは解ける場合がある。これは今回実行した処理だけをworkに数える契約の結果で、hitしたcheckの探索費用を後から足さない。

`build_digest`は実際のbinaryのbytesなど、意味に影響する依存を網羅したSHA-256を呼び出し側が渡す。nativeとWasmでbyteのdigestを使う場合、互いにはhitしない。snapshotはserdeで保存・復元し、`merge(other, max_entries)`で同じbuildのentryを統合する。同じkeyで異なる回答や、digestの壊れたentryがあればmergeを拒否する。SHA-256は偶発的な改変の検出用で、生成者の署名ではない。keyもSHA-256で、source全文は保持しない。digestの衝突がないことは前提とする。

JSONは`run-script-incremental {source,maxWork,...}`と`check-incremental {handle,maxWork,assumptions?,...}`。共通の`cache?`、`enabled?`、`buildDigest?`、`maxEntries?`を受け、`incremental-script-results` / `incremental-solution`で`stats`と更新した`cache`を返す。`enabled`を省略した場合は無効で、渡したsnapshotをそのまま返す。

missは新しい探索に加えて、keyの計算と書き込みの固定費（4〜128変数の整数chainで1 checkあたり約2.4〜4 µs）がかかる。4変数の問題では約5.4倍、128変数では約5%遅くなる。小さい問い合わせでは固定費が上回るため既定は無効にする。[評価0074](../evals/iterations/0074-smt-incremental-cache.md)を参照。

[Wasm loader](../evals/experiments/smt-subset/wasm.ts)はNodeやZ3へ依存しない。実Chromiumで`crossOriginIsolated === false`のままSAT、UNSAT、unknown、未対応、再利用を検査した。0030で測定した生Wasmは330,749 bytesで、Z3 Wasmは39,205,260 bytesだった。対応機能が異なるため、汎用ソルバー同士の容量比較にはしない。

[評価0040](../evals/iterations/0040-smt-expansion.md)では、定数倍・整数iteの確認用140式と21件のcontext問い合わせをZ3へ照合した。旧版が受理した式は1/140、新版は140/140。新機能を狙った人工入力の対応率で、実用入力全体の対応率や速度の比較ではない。

[評価0041](../evals/iterations/0041-smt-scripts.md)では、確認用67 scriptの588 checkをZ3の逐次実行と独立したsnapshotへ照合し、全SATモデルを検査した。旧版の単発solveは同じsourceの1/67を受理した。既存のcontext APIの操作をSMT-LIB入力でも使えるようにした結果で、速度比較はしていない。

[評価0042](../evals/iterations/0042-smt-tuning.md)ではcheckごとの再解析を除き、同じライブラリのnative解析を反復checkで約4.0倍、scope付きで約1.7倍、整数iteで約6.7倍に改善した。既存6ケースを残してCriterionを14ケースに増やし、同じ保存binaryで逆順・再確認も測定した。各checkの探索とwork/statsは変えない。WasmやZ3との速度比較は今回測っていない。

## 再実行

Node.js 24+、pnpm、just、Rustと`wasm32-unknown-unknown` targetを用意する。確認済みrunのディレクトリは上書きできないので、新しいパスを指定する。

```sh
just rusmt-build
just rusmt-test
just rusmt-example
just build
node --test dist/tests/smt-subset/contract-test.js
just smt-browser
just smt-expansion-test
just smt-script-test
just rusmt-criterion
just smt-verify development .artifacts/smt-dev
just smt-verify confirmation .artifacts/smt-confirm .artifacts/smt-dev
just smt-bench .artifacts/smt-bench /absolute/path/to/z3
```

Z3 nativeは[公式5.4.1 release](https://github.com/Z3Prover/z3/releases/tag/z3-5.4.1)を使う。比較対象のnpm `z3-solver`も5.4.1。評価に使ったmacOS arm64 archiveのSHA-256は`a4a878121e81dfc297d027dd0b51dc4f08feb4296d6ecace42bfbc87a29d6d58`で、記録した実行binaryのhashとは区別する。

`smt-build` / `smt-test` / `smt-criterion`は既存の呼び出し用に残す。`just package`はrusmtのcrate archive、WasmとABI manifestも作り、archiveだけを展開したRust consumerで検査する。過去の0030/0031の性能値は移動前の測定として保持する。

## 採否

差分制約への限定経路は継続検討する。公開backendへの組み込みと自動fallbackは保留した。一般のLIAやBooleanが難しい問題でZ3を置き換える判断はしていない。context APIと増分解析、完了した回答の任意cacheは追加したが、学習状態の再利用は未実装。

次の実験は、既存のSMTオラクル用途から入力を集め、対応率と総実行時間を測る。サブセットの速さだけでなく、未対応やunknownでZ3へ戻る費用を含めて採否を決める。
