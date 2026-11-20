# Rust frontendの意味検査と共有template

明示IR v2では、Rust frontendがsource levelを検査し、有限関数を必要な位置で遅延評価する。引数やcaptureのためframeが必要な非再帰actionとtemporal helperは、同じbodyを共有する。既定IR v1とrequest envelopeは維持する。

## source level

constant、state、action、temporalのlevelと、formalが受け取れるlevelの上限を別に計算する。未使用のtop-level/LET定義も検査する。prime、UNCHANGED、ENABLED、action subscript、公平性、集合・比較などのoperand制約をformal/capture経由で伝播する。再帰定義は固定点で解析し、再帰formalをprime対象にする入力をSANYに従って拒否する。

例えば`P(a) == a'`に`x'`を渡す入力は拒否する。一方、`P(a) == TRUE`へtemporal式を渡しても、そのactualは戻り値のlevelを上げない。actual自身の不正な式は、未使用でも拒否する。

この検査は全SANYの名前・型・proof検査ではない。未使用の未知名やproofの正しさ、無限集合の実行可能性を保証しない。使用する式の対応判定とIR検証は別に行う。source-level検査はv1/v2共通で、今回の関数観測と共有化は明示v2が対象となる。

## 遅延する有限関数

関数選択とDOMAINでは、alias、operator actual、選択されたIF枝、record/tupleに含まれる関数を遅延したまま扱う。有限domainは宣言時に求め、bodyは要求したkeyで評価する。宣言時のcurrent/next、binder、old、primeとframeを所有するsnapshotに保持する。

```tla
F[n \in 0..7] == IF n=0 THEN 7 ELSE 1 \div 0
Alias == F
Id(value) == value
```

`Id(Alias)[0]`と`DOMAIN Id(Alias)`は未要求のkeyのゼロ除算を起こさない。key 1の選択、完全な関数の等値比較、状態への全関数の保存では、必要な値のエラーを返す。非停止するkeyを要求して動的な再帰上限を超えた場合は`bounded/evaluation`となる。

内部の遅延値は評価中だけ存在し、公開Scalarやwireに新しい値tagを追加しない。値のmemoizationも行わない。通常のScalarを要求する観測位置は全値を実体化する。EXCEPTのtarget、fold、membershipなど、すべての位置でTLCの遅延評価を再現する実装ではない。EXCEPTのreplacement内で宣言する関数のold保持は検査している。

関数選択の遅延化は一時的な全関数の構築を省くため、既存の小さい対照2件ではworkが13→12、11→10になる。過去の測定やgoldenは編集せず、今回の期待値として記録する。

## 非再帰actionとtemporalの共有

共有定義のkeyは宣言位置、phase、membership用途、target signature、body kindで決める。actionはStep call、temporalはTemporalFormula callから参照する。ID、arity、capture、phaseと循環を静的に検査する。temporalの循環は明示recursiveでも拒否する。

actual/captureはcallerのlexical frameを保持し、要求時に評価する。actionの継続とbacktrack、PROPERTYのatom/always抽出を保つ。入口と引数要求には各1 workを課金する。零引数でframe不要の呼出しはinlineを維持する。非再帰actionの共有表現にも、展開した場合の4096経路/256 stepsの上限を適用する。

TS frontendも非再帰actionを共有する。temporal templateの自動共有はRust frontendが行い、TSのtemporalはanonymous frameを使う。構造比較では、temporal callのkind・arity・循環を検証してframeへ展開した後に照合する。独立TLCとの判定・計数・反例照合はこの構造正規化を使わない。

共有はIRの重複を減らす変更で、分岐や評価仕事を削除しない。速度比較のbaselineにはlevel検査と遅延関数を既に含め、共有化の前後で完全応答とworkを照合する。固定した有限モデルでの測定を、全TLA+の互換性や一般的な速度改善とは解釈しない。

## 再現

```sh
just frontend-semantics-test
just frontend-semantics-runtime <new-snapshot>
just frontend-semantics-oracle <snapshot> <new-run> <java> <tla2tools.jar>
just frontend-semantics-freeze <new-benchmark-snapshot>
just frontend-semantics-bench <snapshot> <new-run>
just frontend-semantics-profile <snapshot> <new-run>
```

snapshotはsource.zip、toolchain、binary hashを保存し、runは上書きしない。Criterionのcompile/fresh/preparedとsampling profileは別に実行する。結果と条件修正は[0072の評価](../evals/iterations/0072-frontend-semantics.md)に記録する。
