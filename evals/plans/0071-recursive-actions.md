# 0071: 再帰する代入actionの生成

基準は23131b74。明示IR v2で再帰するInit/Nextの代入actionを実行し、ENABLED・property・反例も検査する。v1の既存wireと拒否を維持し、有限探索と評価予算を成功へ読み替えない。

## 実装と比較

- [x] TLCのgetNextStates、getInitStates、operator contextと継続の処理を読む。
- [x] 共有definition表にaction本体とStepのcallを追加し、expressionとの区別・arity・capture・phase・SCCを検証する。
- [x] compilerは再帰actionを実行時の呼出しへlowerする。非再帰の既存inlineと省略時v1を維持する。
- [x] 自己・相互再帰、分岐、有限量化、LET/INSTANCE、高階特殊化、target、prime、EXCEPT oldとcaller/calleeの束縛を検査する。
- [x] 代入済みslotの照合、分岐間の巻き戻し、呼出し後の継続と未代入変数を検査する。
- [x] 非停止・work・評価深さの打ち切りをboundedとして返し、native/Wasm・TS decoder・browserで再現する。
- [x] TLCの独立実行と判定・状態数・生成数・強制反例を照合し、raw出力とhashを保存する。
- [x] Criterionの初期測定を固定し、profileで確認した費用を改善する。値のmemoizationで意味やwork課金を変えない。
- [x] 対応表・README・IR契約、Jev live/offline/staged、既存回帰と配布consumerを確認する。

## 測定条件

有限の尾再帰、分岐再帰、複数引数とcapture、静的に展開した同値モデル、非再帰の対照を測る。compileと準備済みcheck、cold processと再利用を分ける。20 samples、warmup 500 ms、measurement 1秒を初期条件とし、変更時は理由を記録する。入力・source・binary・toolchainのhashを測定前に固定する。

小さい固定入力でRed/Greenを確認し、未測定の速度改善は主張しない。TLCの非停止を安全性の対照として実行せず、idealisの予算境界と終了する対照を組にする。失敗runと最初の実装の測定を保持し、最適化後は新しいrunで再検査する。
