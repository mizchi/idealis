# 0065: SYMMETRYの行の連続配置

[#67](https://github.com/mizchi/idealis/issues/67)でnode行を連続配置し、現在stateのOption列コピーを省いた。keyの構築と代表選択を対象とし、arena.entriesのコピーやrecord schemaのcacheは加えていない。公開API・縮約条件・論理workは維持する。

[事前plan](../plans/0065-symmetry-allocation.md)は142cf22aでcommitした。基準はa42670fb。source境界のモデル・経路の詳細は、最初のkernel測定後、source境界の実行前に追記した。

## 行と具体状態の契約

Vec<Vec<Scalar>>の行をgroup数×column数のVec<Scalar>へ変えた。canonical keyは行indexを同じScalar順でsortし、同じ行sliceを並べる。代表選択はslice全体を比較する。複数関数の同じnodeを一行へまとめ、具体状態とtraceをkeyから分離する。

Int/Boolのglobal variable、node部分集合、flat recordの型・schema検証は従来どおり。column 0の状態は先頭nodeを代表にする。partial stateの拒否、workの課金と予算切れの順序も保つ。準備済みprogramへ探索の代表やArenaのRefをcacheしない。

## 21本のkernel比較

before/afterのCriterion binaryと入力を固定し、ABBAで各2反復を測った。各20 samples、warmup 0.5秒、measurement 1秒。下表は各側2つのmean推定の算術平均、単位ms。反復の区間をpoolした信頼区間ではない。

| モデル・option                 |  before |   after | 時間の変化 |
| ------------------------------ | ------: | ------: | ---------: |
| record N12 SYMMETRY、1 worker  | 176.881 | 150.103 |     -15.1% |
| record N12 SYMMETRY、8 workers | 166.927 | 138.485 |     -17.0% |
| record N12 代表展開、1 worker  |  85.571 |  71.800 |     -16.1% |
| record N12 代表展開、8 workers |  73.009 |  64.408 |     -11.8% |
| record N5 代表展開             |   1.475 |   1.348 |      -8.6% |
| 集合 N5 代表展開               |   0.262 |   0.233 |     -11.1% |
| joint N5 代表展開              |   0.218 |   0.194 |     -10.9% |
| counter N7 6値 代表展開        |   2.490 |   2.137 |     -14.2% |

SYMMETRYを使う14本は約8〜17%短縮した。縮約なし6本は約-1〜+2%、限定PORの1本は約-4%だった。[21本のraw](../runs/0065-symmetry-kernel/manifest.json)と[全推定・各反復の区間](../runs/0065-symmetry-summary/result.json)を保存する。初期のrecord N12の4本でも約12〜14%短縮しており、[そのraw](../runs/0065-record-kernel/manifest.json)と[集計](../runs/0065-record-summary/result.json)を別に保持する。

Apple M5 arm64、32 GiB、Node 24.22.2、非専用ホスト。build/testと時間測定は重ねていない。以前の0061やTLCの過去値から今回の比率を計算していない。

## source境界と短いqueryの再確認

常駐native JSONLとNode Wasmで、source compile要求とone-shot IR check要求の区間を合計した。照合・disk・process起動・TLCの時間は含まない。record N12/5、集合5、joint 5の代表展開off/on、通常small4の計12設定を使い、各1 warmup + 3 samples、経路順を回転・反転した。

record N12の代表展開の中央値はnative 1 workerで87.668→75.502 ms、8 workersで73.748→63.443 ms、Node Wasmで92.549→86.595 msだった。[完了runの288 sample](../runs/0065-boundary-options/result.json)には、compile/checkの各区間と全応答を保存した。

0.1〜1 msの短いqueryでは、初回に一部の8-worker要求が10%以上増えた。固定binaryのまま、N12以外を100 query/sampleへ増やして再確認した。各queryのcompile/check区間を測り、完全応答を照合する。初回とは混ぜず、N12は1 queryのまま保持した。

[再確認](../runs/0065-boundary-confirmation/result.json)の通常4対照では、native 1 workerが約-2〜+2%、8要求が約-13〜+4%となり、10%以上の後退は再現しなかった。SYMの小さいモデルはnativeで約5〜10%短縮した。288 aggregate sample・24,048 queryは固定入力の反復で、新しいモデルや故障の数ではない。

最初の失敗は、比較器が1 workerと8 workersのworkも同じと仮定したためだった。旧版N12の全展開でも27,743,292 / 27,749,563と異なり、新版はそれぞれ同値だった。worker数を揃えた旧版の完全応答へ比較し、workを照合から除外していない。[失敗run](../runs/0065-boundary/error.json)と診断を残した。

2回目はSYMMETRY宣言のない通常counterへsymmetryActions=trueを渡し、旧版のprotocol拒否で停止した。[そのraw](../runs/0065-boundary-fixed/error.json)も保持する。通常対照は既定optionだけを速度比較し、拒否をcompleteへ置き換えない。比較器の型エラーも開発logへ保存する。

## 通常kernelとTLCの対照

通常14本のABBAは約-9.2〜+2.4%で、10%以上の後退はなかった。SYMを使わない処理のため、この差を行配置による汎用の高速化とは扱わない。[通常kernelのraw](../runs/0065-safety-kernel/manifest.json)と[各推定](../runs/0065-safety-summary/result.json)を参照。

TLC 1.7.4との27モデル、代表展開の同じ27モデル、限定PORの10対照を新しく実行し、[SYM](../runs/0065-symmetry-tlc/result.json)・[代表展開](../runs/0065-actions-tlc/result.json)・[POR](../runs/0065-por-tlc/result.json)の全64比較で不一致0だった。正常・故障・empty・deadlockを区別し、具体反例も元の仕様へ照合した。小さいcorpusの8-worker要求とnative/Wasmも比較する。N12の大きいfrontierは上の旧版差分でworker数ごとに完全比較した。

## 検証と採用

変更前に追加した2つのcharacterization testは、旧実装と新実装で通った。column 0の先頭代表、global変数の位置、partial stateの拒否と課金、複数columnのkey構築workを検査した。既存のjoint row・完全key・record型・具体trace・worker remapも維持する。

[21検査](../runs/0065-verification/result.json)はすべて通り、検証中のcrate sourceは不変だった。check、analysis 4,408件、compiler、safety 1,185件、portable、実Chromium、fmt/clippy、runtime、evaluate/replay、3言語比較、packageの単独consumer、SUPPORT gateを含む。[旧版との差分](../runs/0065-parity/result.json)も156モデル・1,560要求でnative/Wasmの完全応答が一致した。Jevのlive/offline 47比較は矛盾・未評価0だった。mutationは既存TS対象のcacheを再利用し、今回のRustのmutation検証とは扱わない。

意味と論理workを保ち、SYMの14本とsource境界で短縮したため採用する。行ごとのVecを減らしたが、allocation回数やRSSの削減量を直接計測したとは主張しない。schemaのcache、一般のSYMMETRY、新しい縮約条件は別の仮説として残す。[source archive・driver・開発log](../runs/0065-audit/result.json)には失敗した比較器も保存する。
