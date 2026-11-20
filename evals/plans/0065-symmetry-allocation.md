# 0065: SYMMETRYのnode行の割り当て

[#67](https://github.com/mizchi/idealis/issues/67)。基準はa42670fb。準備済みAPIの次に、nodeごとのVecとcurrent stateのOption列コピーを減らす。縮約条件・joint rowの相関・完全key比較・具体反例・論理workは維持する。

## 仮説

NodeRowsのVec<Vec<Scalar>>を、group数×column数の平坦なVecへ変える。canonical keyはrow indexを同じScalar順でsortし、各sliceを同じ順で並べる。代表選択はsliceの完全比較を使う。currentとnextをiteratorで読み、currentのSome列を作らない。

columnが0のglobal scalarだけの状態では、元の実装と同じく先頭のnodeを代表にする。partial state、subset、flat recordのschemaや型違い、typed/未対応形の拒否と、予算切れのphaseを変えない。arena.entriesのコピーとshapeのcacheは混ぜず、最初の変更だけを評価する。

## Redと対照

既存のjoint-row相関、完全keyの衝突、concrete trace、worker remapとTLC対照を使う。追加でcolumn 0の代表、Refのcolumn数が複数の場合、global variableの位置を跨ぐ場合、エラーと小予算を検査する。見た目のallocation回数だけを性能改善と数えない。

## 計測と採否

beforeのsource/native/Wasmとsymmetry Criterion binaryを固定する。既存21本のsymmetry/reduction fixtureを同じcompiled inputでABBA、20 samples、warmup 0.5秒、measurement 1秒で比較する。初期の対象をrecord-12のsymmetry/actionsの1/8 workerに固定し、有用なら全21本と通常14本へ進む。

約5%以上のnative短縮または明確な割り当て・RSS削減を採用の根拠とする。10%以上の後退は固定binaryのABBAで再確認し、再現する汎用変更は戻す。source/IR境界を含むrecord/set/jointと通常caseも1 warmup + 3 samplesで別に測り、state/generated/work/verdictの一致を要求する。native 1/8要求とWasmを区別する。logical workは実際のsort回数に合わせて減らさない。

rawは追記のみ。build/testと通常計測を重ねず、source/binary/input/順序/全sampleを保存する。小さい差だけならshape cacheや一般の対称性へ広げず、他の実モデルで効果を示せる候補を優先する。

終了前にcheck/browser、safety/portable、analysis、fmt/clippy、旧版差分とTLC対照、package、SUPPORT/Jevを確認する。準備済みAPIの不変な共有と、探索ごとのsymmetry代表が独立する契約も保つ。結果をiteration/#67/#63へ戻して次の採否を決める。

source境界の計測はrecord 12/5、集合5、joint 5、通常small4モデルを固定し、代表展開off/onを同じ入力で比較する。旧版/新版native 1/8要求とNode Wasmを常駐させ、コンパイル要求とone-shot IR check要求の合計時間を測る。起動・TLCの時間は含めず、照合とdiskを除く。各応答は旧版の同じoptionsへ完全比較し、8要求もworkを一致させる。順序を回転・反転し、1 warmup + 3 samplesを保存する。

初回source境界は1 workerと8 workerのworkも同じと仮定したため停止した。旧版binaryのN12でもworkは27,743,292 / 27,749,563（代表展開off）で異なり、新版も各々同値だった。workerごとのArena準備・mergeを含む既存の課金を確認し、workers=1/8それぞれの旧版完全応答を基準とする。workを除外せず、同じworker設定の変更前後で一致を要求する。失敗runと診断を保存し、修正後は新規runを使う。

2回目は通常counterへsymmetryActions=trueを渡したため、既存のprotocol拒否で停止した。SYMMETRY宣言のない4通常対照は既定optionのみを時間比較する。宣言のある4モデルはoff/onを維持する。拒否を安全な完了にせず、失敗runを保持して新規runへ分ける。

初回完了の境界測定には、0.1〜1 msの一部native 8要求で10%以上の増加があった。固定binaryのまま、N12以外は1 sampleへ同じ問い合わせを100回まとめ、各compile/check区間の合計を別runへ保存する。N12は1回のまま。入力・1 warmup + 3 samples・回転/反転・各queryの完全応答照合は維持する。反復数を増やした値を初回と混ぜず、独立した故障100件と数えない。
