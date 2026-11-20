# 0061の追加測定: nativeの起動とRSS

prepared探索と別に、凍結した0061-finalのRust CLIをfresh processで測る。コンパイル済みIRのJSON入力・decode・探索・応答serialization・Rust起動を含む。Node hostとTLA+ frontendは含まない。macOSの/usr/bin/time -lで各Rust子processのmaximum resident set sizeを採取し、単位はbyteとする。

counter N7/K5のplain/symmetry/actions、record N5の3経路、record N12のsymmetry/actions各1/8 worker、monotone N10のplain/PORの12経路を固定する。1 warmup後3回、各回の開始経路を回し、奇数回は順序を反転する。全応答のcompleteと固定の状態・生成数を検査する。通常benchmarkやbuild/testと同時に実行しない。

入力、binary/fixture/driverのhash、全sampleのstdout/stderr・wall time・maximum RSSを別runへ保存する。起動を含む小さいsampleの倍率をkernelの倍率へ読み替えず、専有ホストでの測定や厳密なメモリ上限とも扱わない。

## 最終Criterionの変動の確認

0061-final-criterionは、record/集合の最初の反復が2回目の数倍遅かった。全sampleを残し、既存測定と回帰を終えてから、同じ最終binary・sample条件・逆順でrecordとsetsだけを再測定する。反復間の差と、1/8 workerの両方を報告する。通常14本の対照もABBAの全4反復を平均し、10%以上の後退を記録する。原因が特定できない差は未解決とし、都合のよい反復だけを採用しない。

通常14本の最終対照はcounterの8-worker要求で約18%の所要時間増があった。旧版と最終版の凍結binaryを、counter-productとlinear-ringのfilterごとにABBA/BAABの8反復で再測定する。20 samples、warmup 0.5秒、measurement 1秒を維持する。追加測定は既存runと併記し、独立seedの確認とは数えない。
