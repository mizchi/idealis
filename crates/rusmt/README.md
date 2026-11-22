# rusmt

[対応範囲・非対応のチェックリスト](SUPPORT.md)を参照。

任意精度整数の差分制約とBoolean式を解く、SMT-LIBサブセットのRustライブラリ。idealisの他crate、Node、Z3には依存しない。nativeとホストimport不要のWasmを同じソースから生成する。

`QF_IDL`と、無条件の定数代入や同じ係数の定数倍を正規化して差分制約へ変換できる`QF_LIA`の一部を扱う。Boolean分岐とBellman-Fordの負閉路検査を使う。一般の線形整数式、Real、BitVec、配列、量化は対象外。[対応範囲・制約・実測結果](https://github.com/mizchi/idealis/blob/main/docs/smt-subset.md)を参照。

## Rustから使う

ローカルcheckoutでは依存に`rusmt = { path = "../idealis/crates/rusmt" }`を追加する。registryへは未公開。

```rust
use rusmt::Problem;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let problem = Problem::parse(
        "(declare-const x Int)(assert (> x 10))(assert (< x 20))",
    )?;
    let result = problem.solve(100_000);
    assert_eq!(result.status, "sat");
    println!("{}", result.values["x"]);
    Ok(())
}
```

公開境界は`Problem`、`Solver`、`Script`、`Assumption`、`Solution`、`Stats`、`Error`とJSON protocolの`Engine`。`Solver`はassertion scopeを保持し、`Assumption`は一時的なBool literalを指定する。parser、式、探索状態はprivate。`Solution`はserdeでJSONへ変換でき、整数値は精度を保つため10進文字列にする。

- `sat`は全宣言の具体値を返す。
- `unsat`はすべての枝が矛盾した場合に返す。
- `unknown`はwork予算切れで、モデルを返さない。
- `Problem::parse`の`Error`は未対応、不正入力、型や資源上限を表す。

## assertion scopeと一時assumptions

```rust
use rusmt::{Assumption, Solver};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut solver = Solver::parse("(declare-const x Int)(declare-const b Bool)")?;
    solver.push(1)?;
    solver.assert("(= (* 2 x) (ite b 6 8))")?;
    let assumptions = [Assumption { symbol: "b".into(), positive: true }];
    assert_eq!(solver.check_assuming(&assumptions, 100_000)?.values["x"], "3");
    solver.pop(1)?;
    solver.assert("(= x 9)")?;
    assert_eq!(solver.check(100_000).values["x"], "9");
    Ok(())
}
```

宣言は初期sourceに固定し、scopeとassumptionsは各256件まで。失敗したassert/popは元のcontextを保つ。assertは追加した式だけを解析・コンパイルする。新しい整数定数を束縛する式は前のassertionの代入を変えるため、保持した構文木から全assertionをコンパイルし直す。popはpush時点のsnapshotへ戻す。結果・work・statsとerrorは全体の再解析と同じ。学習状態の再利用は行わない。整数iteは最大256選択肢まで展開する。一般LIAや非線形乗算は未対応。

## SMT-LIB scriptの問い合わせ列

同じ制約のcheckは不変な解析結果を共有する。`Script::parse(source)`で全コマンドを検査し、`Script::solve(max_work)`で各checkの`Solution`を順番に返す。push/pop、複数check-sat、Bool literalのcheck-sat-assumingに対応する。宣言・logic・optionは冒頭に固定し、各assertが追加時点で対応式になる必要がある。scope内の宣言、define-fun、get-model/get-value、reset、exitは未対応。

work予算はスクリプト全体で共有する。残り0はunknown/workと空モデル。不正な後続コマンドは全体をerrorにし、部分的なSATを返さない。checkのない入力は空の結果列で、検査成功として扱わない。checkは最大256回。Scriptを再求解すると予算と探索を初めから始める。

## 完了したcheckの再利用

`Solver::check_incremental(assumptions, max_work, &mut cache, options)`と`Script::solve_incremental(max_work, &mut cache, options)`は、完了した回答を`CheckCache`から再利用する。`CacheOptions { enabled, build_digest, max_entries }`の既定値は無効で、通常のcheckへ委譲し、cacheを参照・更新せず、統計を0で返す。

```rust
use rusmt::{CacheOptions, CheckCache, Solver};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let solver = Solver::parse("(declare-const x Int)(assert (> x 2))")?;
    let mut cache = CheckCache::default();
    let options = CacheOptions {
        enabled: true,
        build_digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        max_entries: 1024,
    };
    let cold = solver.check_incremental(&[], 100_000, &mut cache, options)?;
    let warm = solver.check_incremental(&[], 0, &mut cache, options)?;
    assert_eq!((cold.stats.writes, warm.stats.hits), (1, 1));
    assert_eq!(warm.result.values, cold.result.values);
    Ok(())
}
```

keyは正規化したsourceのSHA-256と、順序・極性を含むassumptions。sourceのdigestはassertごとに増分で更新し、popではsnapshotと一緒に戻す。保存するのは`sat`のモデルと`unsat`だけで、`unknown`とerrorは保存しない。hitは今回の探索をしないので`stats.work`は0で、予算0でも回答を返す。Scriptでは残りの共有予算が後続のcheckへ回るため、`solve`とは後続のunknownが変わり得る。

`build_digest`は実際のbinaryと、意味に影響する依存を固定したSHA-256を呼び出し側が渡す。違うdigestで呼ぶと既存entryを捨てる。`max_entries`は保持数の上限で、0なら書き込まない。`CheckCache`はserdeで保存・復元でき、`merge(other, max_entries)`で同じbuildのsnapshotを統合する。entryのSHA-256は偶発的な破損の検出用で、署名ではない。digestの不一致、問題と合わないモデル、同じkeyで異なる回答はprotocol errorにする。

## nativeとWasm

`rusmt` binaryはstdin/stdoutでJSONLを扱う。requestにはversion 1と、`run-script` / `solve` / `prepare` / `assert` / `push` / `pop` / `check` / `release`、`run-script-incremental` / `check-incremental`を指定する。prepareは解析結果を保持し、回答はcacheしない。incremental requestは`cache`・`enabled`・`buildDigest`・`maxEntries`を受け、更新したsnapshotを`cache`で返す。run-scriptは`{source,maxWork}`を受け、`{kind:"script-results",results:[...]}`を返す。結果の整数値は文字列。

```sh
just rusmt-build
just rusmt-test
just rusmt-example
just smt-browser
just package
```

Wasmは`wasm32-unknown-unknown`向けの`rusmt.wasm`。既存の`smt_*` ABI v1を保ち、SharedArrayBufferを要求しない。読み書きする関数は`smt_input_resize`、`smt_dispatch`、`smt_output_ptr`、`smt_output_len`。[TS loader](https://github.com/mizchi/idealis/blob/main/evals/experiments/smt-subset/wasm.ts)で呼び出せる。

`just package`は`rusmt-<version>.crate`、`rusmt.wasm`とSHA-256/ABI manifestを作る。展開したrusmtのarchiveだけで別consumerをビルドし、SAT、UNSAT、unknown、未対応、scope・assumptionsと依存の独立性を検査する。registryへのpublishは行わない。Criterionの測定は`just rusmt-criterion`。

Criterionは既存の整数チェーンと整数ite、scriptのparse/prepared solve、context操作に、増分assertとcheck cacheを加えた27ケースを持つ。[改善前後の計測](https://github.com/mizchi/idealis/blob/main/evals/iterations/0042-smt-tuning.md)に保存binaryと測定順、raw samplesを記録した。
