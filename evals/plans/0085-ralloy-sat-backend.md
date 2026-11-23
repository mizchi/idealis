# 0085: ralloyのSAT backend（関係の中核）

[0084](0084-ralloy-incremental-symmetry.md)で、`nary`の`CompositionHolds`は総当たりでは予算の約51倍のwork（25.6億）が必要だと分かった。mizchiの判断でSATへ符号化し、ソルバーはralloyの中に小さなCDCLとして書く。crates/rusmtは使わない（単位伝播も学習もなく、SMT-LIB文字列だけの入口で、ralloyの「他crateに依存しない」と衝突する）。Varisatも使わない（外からworkの上限を与える口がなく、予算切れを`unknown`にする約束を守れない）。

## 範囲（0085）

- 回路: 否定付きの2入力ANDのグラフ（AIG）。同じゲートは共有し、定数とその場で畳み込む。関係はtupleからliteralへの疎な表で持ち、上限（sigのatomの区間、fieldの型）の外のtupleは最初から偽にする。
- 符号化:
  - sig: atomごとに変数。exactlyは定数の真。one/lone/someは式。
  - field: ownerのatomごとに、tuple空間の各tupleに変数。tupleが真なら、ownerと各列のsigに属する（型の制約）。多重度と`A m -> n B`は、ownerのatomごとの数の制約。
  - 関係の演算: 和・差・交差・積・join・domain/range制限・転置・閉包（繰り返し二乗）・反射閉包・等価・包含・`no/some/one/lone`。
  - 量化子: 上限のatomごとに展開し、所属を条件にする（`disj`を含む）。
  - pred/fun: 呼び出しをインライン展開する。
- runは`fact ∧ 本体`、checkは`fact ∧ ¬本体`を解く。checkが充足不能なら、factだけを解いて`no-counterexample`と`no-instance`を区別する。
- CNF: Tseitin変換。
- CDCL: 2-watched literals、1UIPの節学習、非時系列の後戻り、変数の活性度、restart。伝播と衝突をworkとして数え、予算を超えたら`unknown`。
- 見つけた解は、存在するatomだけで`Instance`にする。
- 対象外（今までどおり総当たりで解く）: `extends`・`in`・`abstract`のsig、整数式（`#`、算術、整数の比較）。
- 使い分け: SATで扱えるcommandで、総当たりが選ぶbitの合計（fieldのslotの幅の和）が24を超える時だけSATで解く。

## 正しさ（結果を見る前に固定）

- 5 groupの再生がAlloyと一致すること。`CompositionHolds`が予算5000万の中で`no-counterexample`になり、pendingの一覧から外れること。
- 差分テスト: SATで扱えるcorpusのcommandすべてで、SATを強制した答えが総当たりの答えと一致すること。SATが返したinstanceが`check_instance`を通ること（runは`matched`、checkの反例は`mismatch`）。
- CDCLの単体テスト: 小さいCNFの全代入列挙との一致（ランダムに生成した式）。
- 人工故障: Tseitinの節の符号を1つ逆にする、閉包の二乗を1回で止める、`all`の展開で所属の条件を外す、fieldの型の制約を外す、の4件を上のテストが検出すること。

## 速度（結果を見る前に固定）

0084aのcandidateの保存binaryをbaselineとして、`core`の13ケースを交互の順に測る。`core`はどれもbitの合計が24以下で、総当たりのまま。13ケースすべてで、両組の比が1.1以下。`CompositionHolds`の時間とworkは報告する。
