# 0068: rtlaの構文互換性

公式構文corpus 355件とprefix対照13件を、rtla native/Wasm、SANY 1.7.4、1.8.0 prereleaseで比較した。両版が拒否するラベルによる誤受理を1件修正した。既定のprefix優先順位は1.7.4のまま維持する。rtlaは有限fragmentのparserであり、SANY全体の代替ではない。

[事前plan](../plans/0068-rtla-compatibility.md)は04d33781、基準はa242dcbc。corpusはtlaplus/tlaplusのd8fb231de4e4b5d02c1b2ed1ac759c086f694021にある42 txtを全件読む。各入力・期待AST・属性と出典、上流MIT license、SANYの生tree、全応答を保存した。公式release APIと[1.7.4](https://github.com/tlaplus/tlaplus/releases/tag/v1.7.4)、[1.8.0](https://github.com/tlaplus/tlaplus/releases/tag/v1.8.0)でstable/pre-releaseを確認し、1.8.0 jarは公開SHA-256へ照合した。

## 修正後の355公式case

下表は追加13対照を除く。SANY受理は構文解析のみで、名前解決・level検査・評価・モデル検査の成功を意味しない。

| 参照             | SANYの結果   | rtla受理 | rtla syntax | rtla unsupported |
| ---------------- | ------------ | -------: | ----------: | ---------------: |
| 1.7.4            | 受理         |      148 |          39 |               40 |
| 1.7.4            | 構文拒否     |       12 |         113 |                2 |
| 1.7.4            | 参照内部例外 |        0 |           1 |                0 |
| 1.8.0 prerelease | 受理         |      160 |         124 |               41 |
| 1.8.0 prerelease | 構文拒否     |        0 |          29 |                1 |

1.7.4の12件の受理差は、1.8.0では両方が受理するCASE/bullet配置、backtickを含む文字列、Unicode文字列等だった。単純に12個のrtlaバグと数えない。1.7.4の空ファイルは参照の診断生成中のNullPointerExceptionで、syntaxや一致に置き換えずpartial-referenceとした。

[修正前1.7.4](../runs/0068-stable-audit/result.json)・[修正後](../runs/0068-stable-fixed/result.json)、[修正前1.8.0](../runs/0068-preview-audit/result.json)・[修正後](../runs/0068-preview-fixed/result.json)を別々に保持する。native/Wasmは368種類の固定入力について各runでAST・診断全体が一致した。同じ入力の反復を独立モデルに数えない。

公式corpusの表は受理/拒否の比較であり、受理した全ASTの結合が正しいことを示す比較ではない。SANYの既知の誤受理やcorpusとの世代差も残る。tla-ts 0.0.10の受理/拒否を診断用に保存したが、正解の基準には使わない。

## prefixの差は参照版の違い

1.7.4のConfigConstantsではSUBSET/UNION/DOMAINが10..13。1.8.0のOperatorsでは順に8..8、8..8、9..9である。rtlaは前者へ一致する。初期の「prefixを修正する」という仮説は、版を混同していたため撤回した。

| 式                | 1.7.4 / 現rtla      | 1.8.0            |
| ----------------- | ------------------- | ---------------- |
| `SUBSET 1..2`     | `(SUBSET 1)..2`     | `SUBSET (1..2)`  |
| `DOMAIN f + n`    | 構文拒否            | `DOMAIN (f + n)` |
| `SUBSET S \cup T` | `(SUBSET S) \cup T` | 構文拒否         |

13対照のうち、1.8.0が受理してrtlaが拒否するものが4件、逆が3件ある。上の表のように両方が受理してもASTが異なる例もある。「公式355件で1.8.0の拒否に対する誤受理0」を全sourceの互換性と解釈しない。参照版を選ぶpublic APIは今回加えていない。今後設計するなら、IR v1/v2とsyntaxの版を別の契約にする必要がある。

## ラベルの誤受理を修正

公式labels.txtの`a * lab :: b + c`は両SANYが拒否するが、rtlaは`a * (lab :: b + c)`として受理した。ラベルを消すと演算子の結合が変わる。TLCのlabelDoesNotChangeParseを読み、周囲のinfix/prefixより低い優先順位をlabelが包む場合を拒否する。

label自体またはbody全体の明示括弧、高い優先順位のbodyは維持する。単に先頭と末尾が括弧かを見るのではなく、body全体を囲う括弧かを確認する。名前解決や評価は追加しない。13個の追加probeを両jarで実行し、`[] lab :: x'`、同順位、bodyの一部だけに括弧がある場合も対照にした。nested labelやCartesian productについては、参照が受理する扱いを保持する。

[Rust回帰](../../crates/rtla/tests/labels.rs)のRedを先に保存し、修正後に通した。全368応答のbefore/after差はこの公式caseの1件だけだった。[snapshot再生](../rtla-compatibility/contract-test.ts)はABI/回帰検査で、保存したSUT出力を独立した意味論oracleとは呼ばない。Chromiumにも拒否後の再利用を追加した。tla-ts依存のTS parserにも同じ誤受理があり、今回の修正はRust frontendに適用する。

初回修正のレビューで、通常の論理積/論理和はApplyではなくAnd/Orに格納するため、検査から漏れることが分かった。[追加7対照](../runs/0068-label-junction-reference/result.json)を両SANYで先に確かめた。`a + lab :: b /\ c`等の4拒否と、`a => lab :: b /\ c`・body全体の括弧・bulleted junctionの3受理を区別する。追加Redを保存し、非bulletedのAnd/Orへ同じ優先順位検査を適用した。Rustとnative/Wasm再生へ追加し、bulleted junctionの受理は維持する。

初回の全体検証は、analysis inventoryのparser source hashが変わったため1件失敗した。演算子・AST・loweringの一覧を再抽出し、一覧の変更がないことを照合してhashだけを更新した。初回の失敗runは残し、最終ソースの検証は別runへ保存する。

## 残る不足と優先順位

1. 参照版の契約。1.7.4の互換性を保って1.8.0のprefixへ進む方法を、ASTとcompilerの契約まで決める。既定を黙って切り替えない。
2. 記号operatorの定義/宣言・nonfix参照と全operator語彙。lexerに記号を足すだけではなく、ASTでのbindingとcompilerがbuiltinとのshadowingをどう解決するかを検査する。
3. UnicodeのCASE矢印、bullet、集合/関数/量化alias。現状は一部のUnicodeだけを扱う。認識できる対応外構文のunsupported分類も整える。
4. labelの引数、一般のsubexpression selector、高階引数。既存ASTに格納できる部分と、後段が解釈できる範囲を分ける。
5. proof、RECURSIVE、nested module、ASSUME/PROVE。モデル検査向けのfragment拡張と、proofを扱うparserの拡張を別タスクにする。

今回の監査にTLC config全体の構文、SANYの名前解決/level検査、全ASTの構造比較は含めていない。ASTを返すだけでcompilerやcheckerも対応済みとは扱わない。

## 参照adapterの失敗

最初は1.7.4のoperator表を初期化していなかった。その後、現行のprefix期待を旧jarへ使って失敗し、版の差を特定した。空ファイルで正規parse wrapperの診断が例外となることも観測した。CompilationUnitを直接呼ぶ試作は、1.7.4固有のBracketStack初期化を省いて失敗した。

最終adapterは組込表を版に応じて初期化し、正規wrapperでrootだけを構文解析する。import先の読み込みや意味解析をしない。wrapperのRuntimeExceptionは専用markerとstack traceへ保存し、他の入力の収集を続ける。例外をsyntax拒否へ変換しない。JDK/JVMは外部参照の実行にだけ使い、rtlaのnative/Wasmの依存には加えない。失敗したrunと開発logも保持する。

## 再実行と記録

`just rtla-compat <new-run> <upstream-checkout> <jdk-home> <jar> <snapshot> <version>`で外部参照を実行する。snapshotは`just rtla-build`後のnative実行ファイルとWasmを、それぞれ`rtla`と`rtla.wasm`としてコピーしたdirectory。upstreamのcorpus revision、jar、JDKとsnapshotのhashをrunへ記録する。versionは`1.7.4`または`1.8.0`で、新しいrunを指定する。

`just rtla-compat-test`は保存した368入力のnative/Wasm全応答と、追加7対照の両SANY判定を再生する。外部JVMを起動せず、保存済みoracleの範囲を超える互換性を判定しない。[最初のsourceと生出力の保管](../runs/0068-audit/result.json)には、失敗adapter、修正前後のcrate source、SDK/release metadataとhashを含めた。JDK・jar・native/Wasmの実行バイナリはGitへ追加しない。

全体の検証ログは[初回](../runs/0068-verification/result.json)と[最終ソース](../runs/0068-final-verification/result.json)へ分ける。追加修正のsource、Red/Green、parser固有のnative/Wasm/Chromium検査は[最終audit](../runs/0068-final-audit/result.json)へ記録する。これらのテストの合格と、上表に残る構文互換性の不足は別々に扱う。
