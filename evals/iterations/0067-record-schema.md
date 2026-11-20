# 0067: 同じrecord値のschema再検査

[#69](https://github.com/mizchi/idealis/issues/69)で、検査済みの先頭recordと同じScalarのschema再構築を省いた。異なるrecordのschema・型を引き続き検査し、論理workを同じ順で課金する。Arenaをまたぐcache、新しい共有metadata、縮約条件の拡張は加えていない。

[事前plan](../plans/0067-record-schema.md)は6ce7bb5cでcommitした。基準は4587a3ea。TLCのTLCStateMutとFcnRcdValue.takeExcept/snapshotを読み、代表と具体値の分離、EXCEPTが元の値を変えない扱いを確認した。RustのArenaへ同じrecordを持つnode関数を作り、旧版・新版で同じkeyと先頭代表を確認した。keyの11 work、代表の7 workも変えていない。

## 初回4本と固定binaryの再確認

record N12の全展開/代表展開、1/8 workerを同じfixtureで測った。ABBA、20 samples、warmup 0.5秒、measurement目標1秒。値は各側2つのCriterion mean推定の算術平均で、反復の区間をpoolした信頼区間ではない。

初回の短縮は代表展開が約4〜5%、全展開が約6%だった。境界に近いため、同じ固定binaryで4本をもう一度ABBAで測った。再確認では約8〜16%短縮した。初回との幅があり、大きい方だけを代表値にしない。[初回raw](../runs/0067-record-kernel/manifest.json)・[各推定](../runs/0067-record-summary/result.json)、[再確認raw](../runs/0067-record-confirmation/manifest.json)・[各推定](../runs/0067-confirmation-summary/result.json)を別々に保存する。

初回・再確認とも全4本を残し、実行前の追加条件に従って21本のSYMと通常14本へ比較を広げた。Apple M5 arm64、32 GiB、Node 24.22.2、非専用ホスト。build/test/profileは時間測定と重ねていない。

## 21本と通常14本の対照

SYMでrecordを使う6本は約6.5〜9.1%短縮した。単位ms。各側2つのCriterion mean推定の算術平均。

| recordとoption          |  before |   after | 時間の変化 |
| ----------------------- | ------: | ------: | ---------: |
| N12 全展開、1 worker    | 152.743 | 140.116 |      -8.3% |
| N12 全展開、8 workers   | 141.915 | 129.003 |      -9.1% |
| N12 代表展開、1 worker  |  73.403 |  67.945 |      -7.4% |
| N12 代表展開、8 workers |  63.886 |  59.486 |      -6.9% |
| N5 全展開               |   1.686 |   1.576 |      -6.5% |
| N5 代表展開             |   1.346 |   1.254 |      -6.8% |

残る15本は約-4〜+3%。[21本のraw](../runs/0067-symmetry-kernel/manifest.json)と[全推定・反復の区間](../runs/0067-symmetry-summary/result.json)へ保存した。通常14本は約-5〜+5%で、10%以上の後退はなかった。counter逐次の約+4.5%、channel逐次の約+3%も残し、全モデルの高速化とは扱わない。[通常kernelのraw](../runs/0067-safety-kernel/manifest.json)と[各推定](../runs/0067-safety-summary/result.json)を参照。

## source境界とwork

record N12/5、集合5、joint 5と通常small4を使用した。SYMモデルの代表展開off/onと通常の既定optionで計12設定。before/after native 1/8要求とNode Wasmの6経路を常駐させ、source compileとone-shot IR check要求のAPI区間を合計した。照合・disk・process起動・TLCの時間は含まない。

record N12は1 query、それ以外は100 query/sample、各1 warmup + 3 samples、順序を回転・反転した。[288 aggregate sample](../runs/0067-boundary/result.json)に24,048組のcompile/checkの時間と完全応答を保存した。同じworker設定の旧版へworkも含めて比較する。固定入力の反復であり、独立した故障やモデル24,048件とは数えない。

| N12 代表展開     | before ms | after ms |
| ---------------- | --------: | -------: |
| native 1 worker  |    77.523 |   75.507 |
| native 8 workers |    67.220 |   63.643 |
| Node Wasm        |    91.545 |   86.913 |

中央値。nativeは約3% / 5%、Wasmは約5%短縮した。通常4対照は約-3〜+4%。kernelの倍率をsource境界へ当てはめず、0065の時刻の値とも混ぜない。

## 今回の区切りと再開条件

この変更を最後の小さい仮説とする。次のshape cacheやborrowed entriesは、Refの所属・worker移送・エラー/予算の順序と所有権の変更を伴う。新しいprofileで割り当てやschema検査が支配費用と示せるまで保留する。小整数圧縮は既存0/1表現で足りない実モデルがRSSで困る場合、worker制御は広いfrontierで待機・merge・acceptの費用を示せる場合に再開する。

残る改善の可能性がなくなったという判断ではない。未確認の利得へ構造変更を増やさず、今回の4つの採用結果と[umbrella #63](https://github.com/mizchi/idealis/issues/63)へ再開条件を残す。

## 検証と採用

新しく実行したTLC対照は[SYM 27](../runs/0067-symmetry-tlc/result.json)、[代表展開27](../runs/0067-actions-tlc/result.json)、[POR 10](../runs/0067-por-tlc/result.json)で不一致0だった。[旧版156モデル・1,560要求](../runs/0067-parity/result.json)もnative/Wasmの完全応答が一致した。N12の大きいfrontierはsource境界のbefore/afterで同じworker設定ごとにworkも照合し、小さいTLC対照と区別する。既知の別のTLC乖離を一致へ変えていない。

[21種の検査](../runs/0067-verification/result.json)はすべて通り、検証中のcrate sourceは不変だった。check、analysis 4,408件、safety 1,185件、compiler/portable、実Chromium、fmt/clippy、runtime build/test、evaluate/replay、3言語比較、package単独consumer、SUPPORT gateを含む。Jevのlive/offline 48比較も矛盾・未評価0だった。mutationは既存TS対象のcacheで、今回のRustのmutation評価とは区別する。

初回・再確認・21本のrecordで短縮があり、通常対照に10%以上の後退がないため、この狭い省略だけを採用する。割り当て回数やRSSの削減量を直接測ったとは主張しない。[source・driver・開発log](../runs/0067-audit/result.json)に全段階を保存する。今回のmicro tuningはここで区切る。
