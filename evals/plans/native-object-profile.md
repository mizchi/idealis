# native object入力の費用を分ける

前回のobject経路はJSON経路より約3倍遅かった。今回の目的は、JSの入力検査、N-APIの値変換、Rustのdispatchと出力生成のどこに費用があるかを確かめること。基準は`ce30a09b2`。最初にJSとnative binaryを凍結し、既存の6入力・全report・拒否条件を維持する。

測定前の条件:

- 入力検査、検査を除くnative object呼出、出力decoder、JSON経路、全requestを別々に測る。事前生成したJSONを使う区間は区間測定と明記し、全経路の高速化へ数えない。
- 30 samples、100 warmup、固定6入力と既存iterations、開始順回転、全応答の計時外比較を使う。区間ごとに得た中央値の和を全requestの厳密な内訳にしない。
- Node inspectorと可能ならsamplyで、通常時間の測定から分けてcall stackを保存する。sample比をwall timeの比に読み替えない。
- 候補は検査時のdescriptorや中間配列の削減、重複した走査とcloneの削減。getterを呼ぶ、符号付きzeroや不正Unicodeを失う、深さ・size予算を外す、可変objectの検査結果をcacheする最適化は採用しない。
- 改善候補には拒否条件の回帰を先に追加する。変更前後の全reportと拒否を照合し、同一hostで前後/後前の順で各30 samplesを測る。object全経路の幾何平均1.15倍以上、対照に10%以上の後退なしを採用の目安にする。
- 同じJSON値でも共有参照のない入力を6件追加し、元の6件と分けて保存する。可変objectの共有や検査結果のcacheでだけ改善していないことを確認する。変更前同士の正常対照でも同じ採用判定を計算する。
- 計測器・frozen source/binary・入力・raw samples・profile・失敗を新しいdirectoryへ保存する。結果の出たrunは編集しない。sourceのhashと全reportの再生を検査する。

Rust本体・生成型・公開ABIを変えずに改善できる場合はその範囲を優先する。残るN-API変換を外すには契約や表現の変更が必要かを区別し、objectをJSON経路へ置き換えた結果をobject変換の高速化と呼ばない。
