# node関数に限定したSYMMETRY

Rust frontendとrtlcは、同じ種類のnodeを入れ替えても仕様が変わらない場合に、状態を同一視できる。単一のモデル値集合に対するnode関数とnode部分集合を扱う。任意のTLA+仕様の対称性を自動証明する機能ではない。

## 宣言と条件

```tla
---- MODULE Nodes ----
EXTENDS Naturals, TLC
CONSTANT Members
VARIABLE local
Symm == Permutations(Members)
Init == local = [node \in Members |-> 0]
Next == \E node \in Members:
          local' = [local EXCEPT ![node] = IF @ < 2 THEN @ + 1 ELSE 0]
Safe == local \in [Members -> 0..2]
====
```

```text
CONSTANT Members = {a,b,c}
INIT Init
NEXT Next
INVARIANT Safe
SYMMETRY Symm
```

Rust frontendはflat moduleのゼロ引数定義`Symm == Permutations(Members)`だけを受理する。`TLC`のimportと、CONSTANTへ直接割り当てた2〜64個の異なるuntyped model valueが必要。`n_a`のように先頭1文字とunderscoreで型を持つ値は対象外。複数群、任意の置換集合、INSTANCEや追加module bundleは拒否する。通常の式としての`Permutations`は追加していない。既存TS frontendのSYMMETRY対応は追加していない。

状態変数はInt/Bool、Membersの部分集合、全Membersをdomainに持つ関数に限定する。関数の値はInt、Bool、またはflat record。各関数内で値の種類を揃え、recordのfield名と各fieldのInt/Bool型も揃える。`[n \in Members |-> [term |-> 0, ready |-> FALSE]]`を扱える。node参照を格納するleader/pointer、record内のnested値、sequence、部分domainは対象外。

## 宣言が要求する前提

SYMMETRY宣言は、指定したnodeの任意の入れ替えがInit、Next、検査するinvariantを保つという利用者の前提。この前提が成立しない仕様に対して、縮約後のcompleteから元の仕様の安全性を結論してはならない。TLCと同様、ここで仕様のequivarianceを証明してはいない。

rtlcは保守的なgateとして、群の個別要素を直接参照するIRを拒否する。群の全要素を含む定数集合としての使用だけを受理する。CHOOSE、fold、sort/select-seq、ENABLED、sequence操作等も対象外。gateを通ることと、宣言した前提の証明は区別する。安全性を判断する際は縮約なしの小さいモデルとの照合も行う。

invariantとdeadlockのsafety検査に限定する。temporal/fairness、state/action constraint、initial/action property、coverageとの併用を拒否する。VIEWは別機能で未対応。nativeは64状態以上のfrontierで並列展開を使い、workerの値をrootへ移送してから逐次にkeyを照合する。小さいfrontierとWasmは逐次実行する。

## 同一視と反例

各nodeについて、すべての関数の値と各集合への所属Boolを同じ行へまとめる。その行のmultisetをkeyにする。recordはschemaと値を含むintern済みの完全な値を使う。変数を独立にsortすると、同じnodeに属する値の相関を失うため行わない。globalなInt/Boolと、各変数が関数・集合・scalarのどれかもkeyへ含める。

この状態形状では、全N!置換の列挙を行わずにorbitを区別できる。hash衝突時はkey全体を比較する。TLCと同じfingerprintのbyte列を作る実装ではない。

探索・反例用には、元の親から生成した具体状態を保存する。同一視のためにnode名を書き換えた状態をtraceへ出さない。新しいsafety反例はSYMMETRYを外したNextへ再生して検証する。livenessと最短lassoは保証範囲に含めない。

## IRと計数

`SafetyProgram.symmetry`はoptionalな`{modelValues: string[]}`。Rustでは`rtlc::ir::SafetySymmetry`を使う。設定しないJSONではフィールドを省略する。RustでSafetyProgramをstruct literalから構築する場合は`symmetry: None`を追加する。

states・generated・depthは縮約後の探索の計数で、縮約なしと一致する値ではない。workには群の準備、keyの構築と行のsort用の固定費用も含める。workはCPU命令数や厳密なメモリ制限ではない。maxStates/maxDepth/maxWorkによるboundedをcompleteへ読み替えない。

## 遷移の代表化

`checkSafety(core, program, bounds, { symmetryActions: true })`を指定すると、現在状態で同じ行を持つnodeを代表だけ展開する。Rustは`program.check_with_options(bounds, SafetyOptions { symmetry_actions: true, workers: 8, ..Default::default() })`を使う。既定はfalseで、既存の全node展開を維持する。SYMMETRY宣言が必要。

先頭に単一のnode bindingを持つbranchだけを対象にする。複数binding、tuple binding、群全体以外のdomainは全候補を展開する。Initには適用しない。具体状態・反例と同じ行の相関を保ち、代表の構築もworkへ計上する。generatedは実際に生成した候補数なので、TLCの全node展開とは一致しない。

[0061の評価](../evals/iterations/0061-symmetry-next.md)にrecord・集合、代表展開と並列化の測定を残す。別の削減である[限定PORと実行option](reductions.md)も参照。

[対応チェックリスト](../crates/rtlc/SUPPORT.md)、[compiler](../crates/rtla-compiler/SUPPORT.md)、[0060の評価計画](../evals/plans/0060-restricted-symmetry.md)を参照。
