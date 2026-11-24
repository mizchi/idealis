# 0085: ralloyのSAT backend

[plan](../plans/0085-ralloy-sat-backend.md)のとおり、関係の中核をSATへ符号化し、ralloyの中に書いた小さなCDCLで解くbackendを加えた。

- **`CompositionHolds`**: 0079から予算切れだったcheckが、work 1,910（1 ms未満）で解け、Alloyと一致した。総当たりでは0084の時点でもwork 25.6億、51秒かかっていた。corpusは5 group・174 commandすべてでAlloyと一致し、pendingはなくなった。
- **速度**: `core`はすべて総当たりのままで、`adopt`。
- **Wasm**: 459,882 bytesから551,171 bytes（+19.9%）。

## 選んだ方法

mizchiの判断で、ソルバーはralloyの中に書いた。

- crates/rusmtは使わなかった。Boolean探索が分岐ごとに状態を複製する方式で、単位伝播も節の学習もない。入口はSMT-LIB文字列だけで、入れ子とtokenの上限がある。ralloyの「他crateに依存しない」とも衝突する。
- Varisatも使わなかった。0037の評価では、外からworkの上限を与える口がなく、予算切れを`unknown`にするralloyの約束を守れない。

## 実装

- **回路（`sat/circuit.rs`）**: 否定付きの2入力ANDのグラフ。同じゲートは共有し、定数・同じ入力・否定し合う入力をその場で畳み込む。Tseitin変換でCNFにする。
- **符号化（`sat/mod.rs`）**:
  - 関係は、atom番号のtupleからゲートへの疎な表で持つ。
  - sigはatomごとの変数で、exactlyは定数の真。fieldは、ownerのatomとtuple空間のtupleごとの変数。
  - fieldの型（tupleが真ならownerと各列のsigに属する）と多重度・`A m -> n B`を制約にする。
  - 演算はすべて扱う。閉包は繰り返し二乗、量化子は上限のatomごとに所属を条件として展開、pred/funはインライン展開。
  - runは`宣言 ∧ fact ∧ 本体`、checkは`宣言 ∧ fact ∧ ¬本体`を解く。checkが充足不能なら、`宣言 ∧ fact`を解いて`no-counterexample`と`no-instance`を区別する。
- **CDCL（`sat/cdcl.rs`）**: 2-watched literals、1UIPの節学習、非時系列の後戻り、活性度順の決定と位相の保存、Luby restart。伝播1つと衝突1つごとにworkを1使い、予算を超えたら`unknown`。
- **使い分け**: sigの階層も整数式もないモデルで、総当たりが選ぶfieldのslotの幅の合計が24 bitを超えるcommandだけをSATで解く。それ以外は今までどおり総当たり。

`CompositionHolds`は、fieldの型の制約から`q.r.(q.r)`のtupleが`P -> P`の外に出られないため、CDCLの伝播だけで矛盾に至る。

## 正しさ

- CDCLは、ランダムな2000個のCNF（変数1〜10個、節0〜44個）で全代入の列挙と一致し、返したモデルが全節を満たした（充足可能・不能がそれぞれ300個超）。
- 回路は、ランダムな1000個の回路で、全入力の評価と一致した。
- SATで扱えるcorpusの全commandで、SATを強制した答えが総当たりの答えと一致した。返したinstanceは`check_instance`を通った（runは`matched`、checkの反例は`mismatch`）。総当たりが予算内に解けないcommandは比べずに数えるだけにした。
- 人工故障4件（Tseitinの節の符号を1つ逆にする、閉包の二乗を1回で止める、`all`の展開で所属の条件を外す、fieldの型の制約を外す）をすべて検出した。Alloyの再生でSATに回るのは幅が24 bitを超えるcommandだけなので、主に上の差分テストが捕まえた。
- Rust・TS・Chromium・`just check`・`wasm-test`・`package`・`evaluate`・`replay-evaluation`が通った。

## 速度

[比較](../runs/0085-ralloy-comparison/result.json)。baselineは0084aのcandidate。`core`の13ケースはどれも幅が24 bit以下で総当たりのまま。両組とも0.925〜1.079で`adopt`。負荷平均は約4〜5で、専用ホストではない。

## 範囲と残り

- SATは、sigの階層（`extends`・`in`・`abstract`）と整数式（`#`、算術、比較）を含むモデルを扱わず、総当たりへ回す。整数はbitベクトルの回路、階層はsigの関係の制約で符号化できる。
- 24 bitの境界は、corpusで速さを比べて決めたものではない。小さい探索でどちらが速いかは測っていない。
- SATのinstanceは、総当たりと別のinstanceを返す。`stats`の`candidates`は解の有無（0か1）、`factInstances`は0。
