# 0072: 意味・level検査、遅延関数、template共有

基準は3fec1611f。Rust frontendの有限fragmentを対象に、SANY/TLCの実行とソースを対照として次の順に進める。全SANYの意味解析・proof検証や無限集合の完全探索とは区別する。

## 実装と対照

- [x] sourceのlevel制約を独立した解析へ分離し、未使用定義も含めprime、ENABLED、UNCHANGED、temporal/action operands、formal・capture・LET・再帰への制約伝播を検査する。正常・不正の対をSANYへ渡す。
- [x] 再帰関数はalias・operator actualを通ってもDOMAINと必要なkeyだけを評価する。宣言時のstate、binder、oldを保持し、等値・状態保存など完全な値が必要な境界では有限domainを実体化する。未使用要素のエラー・非停止を先に起こさず、要求した要素のエラーを隠さない。
- [x] 非再帰actionとtemporal templateを共有し、kind・phase・actual/capture・SCC・反例名を検査する。生成順・lexical scopeと現在のwork契約を保持する。省略時v1の受理・wire契約は維持する。
- [x] native/Wasm完全応答、Chromium、独立TLCの判定・計数・強制反例と予算境界を照合する。
- [x] Criterionでcompile/fresh/preparedを分けて固定source・binaryで比較し、改善しない最適化を採用と数えない。
- [x] SUPPORT/README/IR契約とJev live/offline/staged、既存回帰・配布consumerを検査する。

## 実験条件

正常と意図した故障を分け、development seed 1072と未観測のconfirmation seed 2072を使う。生成器と入力範囲を観測前に固定する。固定ケースのRed/Greenを先に行う。TLCの非停止を成功対照へ使わず、idealisのboundedを独立した打切りへ分類する。

Criterionは20 samples、warmup 500 ms、measurement 1秒。順序をABBA/BAABとしてcompile、fresh check、prepared checkを分ける。入力・source・binary・toolchainを固定し、profileとtimingの実行を重ねない。失敗run、条件の修正、最初の実装の測定を残す。

条件修正は0072-frontend-properties-2.jsonへ追記せず別planとして保存した。旧seed 1072/2072の結果と入力も残し、修正後の確認には1172/2172を使う。関数選択の遅延化によるwork減少と、共有化前後のwork一致を分けて評価する。
