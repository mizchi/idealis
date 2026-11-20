# 再帰する代入action

Rust frontendの明示IR v2は、自己・相互再帰するInit/Nextの代入actionを実行する。`irVersion: 2`を指定する。省略時のv1と、既存TS frontendの再帰対応範囲は変えない。

```tla
---- MODULE Counter ----
EXTENDS Naturals
VARIABLE x
RECURSIVE Down(_)
Down(n) == IF n=0 THEN x'=(x+1)%3 ELSE Down(n-1)
Init == x=0
Next == Down(4)
Safe == x \in 0..2
====
```

この例は3状態を巡回する。再帰の深さごとに本体を展開せず、共有definitionを呼び出す。分岐、有限の存在量化、LETの捕捉、INSTANCE、高階operatorの特殊化、EXCEPT、ENABLEDの生成位置もStep callを使う。action propertyの値評価はexpression定義へ別にlowerする。引数・capture・caller binderのためframeが必要な非再帰actionも共有する。零引数でframe不要の呼出しはinlineを維持する。[非再帰templateの共有](frontend-semantics.md)を参照。

呼出し先の代入は、既に代入したslotなら値を照合する。矛盾する分岐を捨て、次の分岐へ戻るときはslotを元へ戻す。呼出し後の条件はcallerのbinderと引数環境で評価する。calleeが残した未代入変数を、完全な次状態として採用しない。

引数とcaptureはcallerの字句環境・EXCEPT oldを保持する。値は要求するたびに評価し、現在のnext状態とprimeを使う。call entryと引数要求へ各1 workを課金する。親環境の共有はimmutableな環境の所有を共有するもので、値や評価結果のcacheではない。

## IRと型の境界

v2の同じ`definitions`表で、`bodyKind: "expression"`の`body`は式、`bodyKind: "action"`の`body`はbranch配列となる。actionは`{kind: "call", definition, arguments, captures}`から呼ぶ。式のcallとactionのcallは相互に取り違えられない。native/Wasmのrequest envelopeとABIはversion 1を維持する。

Rustの`Definition.body`は`DefinitionBody::Expression`または`DefinitionBody::Action`で表す。既存のexpression struct literalは`body: expression.into()`へ変更する。TSの`SafetyDefinition`も`bodyKind`で絞り込めるunionとなる。

ID・body種・arity・capture・slot・phaseと循環を実行前に検査する。循環する強連結成分には全定義の`recursive: true`が必要となる。未評価の分岐にある不正な参照も拒否する。action定義はInit/Nextの生成phaseで使い、state invariantや定数の式callへ渡さない。

## 予算と対応範囲

動的な評価の上限は128段のまま。再帰経路で超えると`bounded / evaluation`、work枯渇は`bounded / work`となる。部分的に状態を生成しても、安全性の成功へ読み替えない。既存の式・値・binder・branchの上限も維持する。

TLCは再帰operator自身のformalへ`UNCHANGED`を適用する形を拒否する。Rustもこの形を拒否し、外側の変数を捕捉した有効な形とは区別する。全SANYのlevel検査、任意のaction構文への対応、無限列挙や無制限の再帰を保証するものではない。

## 回帰と測定

`just recursive-actions-test`はRustとnative/Wasmの共有caseを検査する。`just recursive-actions-oracle RUN JAVA JAR`はTLCの判定・状態数・生成数と強制traceを比較する。正常版、意図したinvariant故障、不正宣言、非停止を分けて保存する。

`just recursive-actions-freeze SNAPSHOT`でsourceと実行ファイルを固定し、`just recursive-actions-bench SNAPSHOT RUN`でCriterionを実行する。compile、fresh check、prepared checkを別々に測る。`just recursive-actions-profile SNAPSHOT RUN`のsamplingは通常測定と分ける。[計画](../evals/plans/0071-recursive-actions.md)と[評価記録](../evals/iterations/0071-recursive-actions.md)を参照。
