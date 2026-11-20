# 0069: 記号operatorの定義・宣言・参照

rtlaの既存operator語彙でprefix/infix/postfix定義、記号の定数・formal宣言、operator引数、修飾したnonfix呼出しを実装した。Definition/Declaration/Apply/Identifierを使い、公開AST型は増やしていない。custom記号の有限モデルをIR v1/v2とTLCへ照合した。全operator語彙や一般の高階operator実行は未対応のまま。

[事前plan](../plans/0069-symbolic-operators.md)をコミットし、基準を6bdf0029へ固定した。TLC 1.7.4のOperatorOrFunctionDefinition、PrefixLHS/InfixLHS/PostfixLHS、SomeFixDecl、OpOrExpr、PrimitiveExp、BangExtとConfigConstantsを確認した。prefixの既定優先順位は変更していない。

## 定義と参照の表現

次の定義を通常名のoperatorと同じ契約へ格納する。

```tla
left %% right == IF right THEN ~left ELSE left
-. value == ~value
value ^+ == ~value
```

定義名は順に`%%`、`-.`、`^+`で、paramsに引数順・arity・位置を保持する。定数/formalの`_ %% _`はarity 2、`-. _`と`_ ^+`はarity 1。式中のunary `-`は既存どおり`-.`へなる。`M!N!%%(a, b)`はApply、引数中の記号名はIdentifierに保持する。TS parserのOperatorReferenceとはAST形式が異なる。

通常の関数・名前・文字列・bulleted junction・offsideを保持する。変数宣言はSANYと同じく識別子だけに限定し、`VARIABLE F(_)`と記号宣言を構文拒否する。builtinの記号でも定義の構文は受理するが、意味解析で合法だという保証はしない。

## 固定33対照

[初期対照](../runs/0069-before/result.json)では、記号単独の式/集合要素/index、括弧付きのinfix参照、修飾したCartesian productを構文拒否と想定していた。両SANYは5件とも構文受理した。PrimitiveExp/BangExtを読んで構文と意味解析の境界を修正し、初回をcontrols-failedとして残した。引数中の裸の`\X`はOpOrExprが拒否し、修飾した`M!\X(...)`とは異なる。

修正した[基準33対照](../runs/0069-baseline/result.json)は両SANYと期待が一致し、旧rtlaには23件の判定差があった。[採用候補](../runs/0069-parser/result.json)では33件とも一致し、native/WasmのAST・診断全体も一致した。[Rust回帰](../../crates/rtla/tests/symbolic.rs)はRedを先に保存し、引数順・arity・位置・文字列との区別を検査する。Chromiumでも記号の定義名と構文エラー後の再利用を確認する。

## 公式355ケースの変化

0068と同じcorpus revision d8fb231de4e4b5d02c1b2ed1ac759c086f694021の42ファイルを使う。下表は追加prefix対照13件を除き、SANYが構文受理した入力だけを数える。

| 参照             | SANY受理 | 旧rtla受理 | 新rtla受理 | 新rtla syntax | 新rtla unsupported |
| ---------------- | -------: | ---------: | ---------: | ------------: | -----------------: |
| 1.7.4            |      227 |        148 |        164 |            23 |                 40 |
| 1.8.0 prerelease |      325 |        160 |        177 |           107 |                 41 |

[1.7.4 run](../runs/0069-stable-audit/result.json)と[1.8.0 run](../runs/0069-preview-audit/result.json)を別に保存した。両参照が受理する16ケースが新たに一致する。もう1件はUnicode junctionを高階引数へ渡す構文で、1.8.0だけが受理する。1.7.4が拒否しrtlaが受理する公式caseは12から13へ増えるが、この追加caseも1.8.0では両方が受理する。公式355件で1.8.0が構文拒否する入力の誤受理は0のまま。

これは言語全体の対応率や全ASTの正しさを示さない。追加prefix対照には既存の版の差が残り、1.7.4の空ファイルは参照内部例外のためpartial-referenceになる。最終ソースのnative/Wasmは368入力の新しい全応答を再生する。0068のラベルの拒否と明示括弧の対照も維持する。

## IR v1/v2とTLCの照合

[22個の固定入力](../symbolic-operators/models.ts)を[TLC 1.7.4と直接比較](../runs/0069-models-final/result.json)した。

- infix、prefix、postfix、nonfix、LET、LOCAL INSTANCEで隠された標準定義の6形式。それぞれ正常/違反のinvariantを持つ計12入力をIR v1/v2で実行し、24比較の判定・状態数・生成数・反例とTLCでの反例再生が一致した。
- builtin/import済み記号との衝突10入力。両IRでcompileを拒否し、TLCもinput-errorに分類した。未使用の定義、未使用operator内のLET、formal宣言の衝突を含む。

12入力は同じ2状態のBool toggleを使う構文対照であり、12種類の独立したモデルや未知バグ発見に数えない。一般的な速度改善も測定していない。標準moduleのLOCAL INSTANCEは定義をexportしないため、`EXTENDS Sequences`の下で独自の`+`を定義する正常対照も含めた。

新しいbinding検査はcompilerの単体・bundle入口でsourceの宣言/定義を調べる。既存の解析walkerで未使用のLET/formalまで確認し、builtin/import済み記号との衝突を拒否する。名前解決やlevel検査の全SANY互換は主張しない。builtin operator値のcallback、operator定数の一般的な実行、記号をWITH左辺に使う置換は今回の実行対応へ含めない。

## 構文対応表と回帰の更新

[独立source inventoryの更新](../runs/0069-inventory/result.json)では、operator表・AST・lowering roleは変わらず、lexerに`-.`が1個増えることを再抽出して確認した。既存38 nonfix文脈のうち37が新たにnative/Wasmで解析でき、nonfix `-.`の3つのlowering文脈も両frontendで通る。

[境界fixtureの更新](../runs/0069-boundary-review/result.json)では、以前の拒否39項目を新しいASTまたはcompile成功へ変更した。ASTのkind・operator・引数数、Identifierの名前を検査し、shared構造corpusの到達数や固定ID/routesは増やさない。[未対応infixの診断4件](../runs/0069-boundary-diagnostics/result.json)はsyntax拒否を維持し、記号を式として認識したことで変わった文言だけを更新した。初回の回帰失敗もlogへ残す。

## 失敗した参照・検査の前提

TLCの衝突検査を始めた試行では、nativeのcompile拒否codeをunsupportedと想定して停止した。既存のpublic境界ではprotocolであり、実装のcodeは変えず検査の期待を修正した。次の試行ではimport済み記号の再定義をwarningだけと想定したが、TLCは`Operator + already defined or declared`も返してinput-errorとなった。両試行の[最初のrun](../runs/0069-models/error.json)・[次のrun](../runs/0069-models-fixed/error.json)を保持し、最終runと分ける。

## 再実行

`just rtla-symbols <new-run> <jdk-home> <stable-jar> <preview-jar> <snapshot>`は33構文対照を外部SANYへ比較する。snapshotはnativeとWasmを`rtla`、`rtla.wasm`として保存したdirectory。判定差を必須で0にする場合は`IDEALIS_RTLA_SYMBOLS_REQUIRE_MATCH=1`を指定する。

`just rtla-symbols-tlc <new-run> <java> <jar>`は22入力とTLC、`just rtla-symbols-test`は保存した構文対照・TLCモデル・衝突をnative/Wasmで再生する。`just rtla-compat-test`は355公式caseとprefix対照、`just rtla-browser`はChromiumを使う。外部JDK/JVMは参照実行にだけ必要で、ライブラリの依存には加えない。

CIには、既存のnative/Wasm buildの直後へ両再生テストを追加する。保存したSANY/TLC結果を使い、CIでJDKや外部oracleの再実行を要求しない。

[全体の検証結果](../runs/0069-verification/result.json)と[基準/採用ソース・生出力の保管](../runs/0069-audit/result.json)を参照。構文の対応範囲は[rtla](../../crates/rtla/SUPPORT.md)、実行と拒否の範囲は[rtla-compiler](../../crates/rtla-compiler/SUPPORT.md)へ記録する。
