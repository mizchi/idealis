# rtla

[対応範囲・非対応のチェックリスト](SUPPORT.md)を参照。

TLA+とTLC設定を型付きASTへ変換するRustパーサー。`idealis-core`と`rtlc`に依存しない。nativeと`wasm32-unknown-unknown`で同じコードを使い、解析中にファイル・ネットワーク・時計・スレッドを使わない。

## Rustから使う

ローカルcheckoutでは`rtla = { path = "../idealis/crates/rtla" }`を依存に追加する。

```rust
fn main() -> Result<(), rtla::ParseError> {
    let module = rtla::parse_module(
        "---- MODULE Counter ----\nVARIABLE x\nInit == x = 0\n===="
    )?;
    assert_eq!(module.name, "Counter");
    let config = rtla::parse_config("INIT Init")?;
    assert_eq!(config.init.as_deref(), Some("Init"));
    Ok(())
}
```

公開関数は`parse_module`、`parse_expression`、`parse_config`と、SyntaxVersionを指定する`parse_module_with_version` / `parse_expression_with_version`。`ast`に型付き契約を置く。ASTと`ParseError`はserdeでJSONに変換できる。`ErrorKind`は`syntax`と`unsupported`を区別する。位置の`start/end`はtokenのUTF-8 byte offsetで、endは含まない。line/columnは1始まり、columnはUTF-16単位、tab幅は8。

## 対応範囲

- モジュールheader、EXTENDS名、宣言、operator/function定義、assumption、ASSUME/PROVE、構造付きproofを持つtheorem
- 101 canonical operatorと有効aliasの記号prefix/infix/postfix定義、記号の定数・formal宣言、operator引数と`M!N!%%(a, b)`形式のnonfix呼出し
- IF、CASE、LET、有限/非有界量化、CHOOSE、集合・関数・record・tuple、EXCEPT、action/temporal/fairness構文
- 算術・比較・集合・論理operatorの対応範囲、prime、関数適用、field参照、bulleted junctionとoffside規則
- TLC設定のInit/Next/Spec、定数・集合・model value・置換、invariant/property、制約等の設定値
- 引数付きnamed/unnamed INSTANCE、LOCAL、記号WITH、LET内INSTANCE、nested module、RECURSIVE、LAMBDA、label引数と一般selector

`SyntaxVersion::Tlc174`が既定で、SUBSET/UNION/DOMAINの優先順位を10..13にする。`SyntaxVersion::Tlc180`は8/8・8/8・9/9を使う。JSONは`syntaxVersion: "1.7.4" | "1.8.0"`を指定する。文法とUnicode aliasは両方で共通に扱う。旧SANYのCASE/submoduleの不具合や廃止文法を再現する設定ではない。

名前解決、モジュールの読込、型/level検査、標準operatorの評価、ASTからIRへの変換、モデル検査は行わない。設定に制約や置換が存在してもASTとして保持するため、実行側は対応可否を別途検査する。

proofはtyped ASTとしてステップ階層・束縛・参照を保持し、証明の検証は行わない。整数は`2^53-1`以下をNumber、それ以上をInteger.textとして原文を保持する。実行側のsigned 32-bit制限と区別する。解析とASTの深さは64段までで、超過はunsupportedになる。

RustでのIR生成は`rtla-compiler`が担当し、rtlaのASTを`rtlc::SafetyProgram`へ変換する。`idealis-core::compile_tla`は解析と変換を接続する。既存のTS API `compileTlaSafety`は独立した比較対象としてtla-ts parserを使う。

## native/WasmのJSON境界

`rtla` binaryは1行に1つのJSON requestを受け、応答を1行返す。

```json
{ "kind": "module", "source": "---- MODULE M ----\nVARIABLE x\n====" }
```

kindは`module`、`expression`、`config`。成功時は同じkindとAST、失敗時は`{"kind":"error","error":{"kind":"syntax","message":"…","location":{…}}}`を返す。`Request`、`Response`、`dispatch`はこのadapter用の契約。構文の拒否をprocess失敗やWasm trapとして扱わない。

Wasm ABI v1は`rtla_abi_version()`、`rtla_input_resize(length)`、`rtla_dispatch()`、`rtla_output_ptr()`、`rtla_output_len()`。input領域へJSONのUTF-8を書き、dispatch後にoutputを読む。resize/dispatchでmemoryが拡張される可能性があるため、viewを取得し直す。outputは次の呼び出しまで有効。同じinstanceへの呼び出しは逐次実行する。host import、JVM、JS callback、cross-origin isolationは不要。

## 検証と配布

```sh
just rtla-test
just rtla-browser
cargo run --manifest-path crates/rtla/Cargo.toml --example inspect
just package
```

`rtla-test`はfmt/clippy、Rust全target、native/Wasm build、tla-ts 0.0.10との固定corpus比較を行う。文法と優先順位は固定TLC 1.7.4 / 1.8.0のSANYへ照合する。[評価計画](https://github.com/mizchi/idealis/blob/main/evals/plans/0026-rtla-parser.md)と`NOTICE`に出典を記録する。

`just package`は独立した`.crate`と`rtla.wasm`を作り、展開したartifactだけでconsumerを実行する。npm artifactは`@mizchi/idealis/rtla.wasm`としてWasmを同梱する。registryへの初回publishは別途行う。
