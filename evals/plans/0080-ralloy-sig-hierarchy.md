# 0080: ralloyのsig階層

[0078](0078-ralloy-alloy-oracle.md)の3段目。今のralloyは、トップレベルのsigだけを扱い、一括宣言もできない。

## 変更

- `abstract sig`、`sig B extends A`、`sig C in A`（親が1つのsubset sig。`in A + B`は対象外）、`sig A, B {}`の一括宣言、signature fact（`sig A {...} { 式 }`）。
- signature factは`all this: A | 式`として扱い、式の中でAとその祖先のfield名を`this.field`へ書き換える。量化子で同名を束縛した内側は書き換えない。
- atomの割り当て: トップレベルのsigごとにatomの連続した区間を持ち、extendsの子はその中の連続した部分区間を持つ。abstractの親で子がある場合、親のatomは子の和集合と等しい。subset sigは親のatomの任意の部分集合で、探索で全部分集合を列挙する。one/lone/someは子とsubset sigにも適用する。
- scope: 明示されたsigはその値。トップレベルで明示のないものは`for N`のN。`for`のないcommandは3（Intのbit幅は4）。extendsの子で明示のないものは親の上限。子に明示した値は親の上限を超えない（記録したAlloyの答え`ChildScope`、`ChildFillsParent`による）。宇宙のatom数はトップレベルのsigの合計。
- sigの上限を8から16に上げる。fieldの上限8は変えない。

## 正しさ（結果を見る前に固定）

- group `hierarchy`（8モデル・40 command、記録済み）がAlloyの答えと全一致すること。`sig-facts`は、最初の記録でAlloyがfield名`next`を`util/integer`の`next`と曖昧だとして拒否したため、`succ`に変えて記録し直した。答えを見て変えたのではなく、Alloyで成り立たないモデルを直したもの。
- `core`と`nary`の再生が、0079と同じ状態（`nary`の1件だけが予算切れ）のままであること。
- 人工故障: abstractの親に子以外のatomを許す、extendsの兄弟のatom区間を重ねる、signature factの`this.`への書き換えをしない、子の明示scopeが親を超えることを許す、の4件を再生テストが検出すること。

## 速度（結果を見る前に固定）

0079で記録した版（dcaa48067）をbaselineとして、`core`の13ケースを交互の順に測る。13ケースすべてで、両組のcandidate/baselineの比が1.1以下。階層のないモデルに固定費を足さないことの確認。超えたケースは後退として記録し、閾値を変えない。
