# 0050: アクション単位のcoverage

## 採用するAPIと範囲

[計画](../plans/0050-action-coverage.md)の最初のcoverage機能を採用する。Rustの`program.check_with_coverage(bounds)`と、TSの`checkSafetyCoverage(core, program, bounds)`が`{ result, coverage }`を返す。native/Wasmの要求は`check-safety-coverage`、応答は`safety-coverage`。通常のSafetyResultとcheck-safetyの形は変えない。

coverageは逐次BFSのIR branch単位。initial/next配列はprogramの順を保ち、同名のbranchも統合しない。式・source location単位、並列coverage、TLCの全cost model、SYMMETRY/VIEWは未対応として残す。[rtlcの契約](../../crates/rtlc/README.md#アクションのcoverage)へ記録した。

| 項目             | 意味                                                      |
| ---------------- | --------------------------------------------------------- |
| attempts         | branchの評価を始めた回数                                  |
| enabled          | 完成した候補を1つ以上生成したattempt数                    |
| generated        | 重複・constraint除外も含む完成候補数                      |
| distinct         | 保存した新規state数。最初の発見branchへ帰属する           |
| work             | branch評価と受理時のconstraint/property検査を含むwork差分 |
| unattributedWork | 事前検査・時間論理など、行へ帰属させないwork              |

generated/distinctの行合計はresult.generated/statesに一致する。行のworkとunattributedWorkの和はresult.workに一致する。boundedや違反では途中の計数を返し、評価エラーは通常と同じエラーにする。workは時間やメモリ量ではない。

```ts
import { checkSafetyCoverage } from "@mizchi/idealis/portable";

const report = checkSafetyCoverage(core, program, bounds);
console.log(report.result.status);
console.table(report.coverage.next);
```

## 計数と判定の検証

[最終実行](../runs/0050-specialized/result.json)と[再実行](../runs/0050-specialized-replay/result.json)で162入力を検査した。140入力をcompileし、通常・小さいwork/state/depth/temporal予算の6条件で計840回、on/offとnative/Wasmを照合した。残る22入力は未対応拒否で、coverage成功には数えない。

[旧APIとの比較](../runs/0050-legacy-specialized/result.json)では、156ケース・各10要求で、開始時の85cf5da2のproduction native/Wasmと完全な応答が一致した。state/generated/work、型エラーと予算切れ、反例も含む。[最初の構成](../runs/0050-final/result.json)と最終構成のreportsも完全に一致した。

Rustの6回帰では、通常のring、無効branch、同名の重複branch、重複Init、branch途中のwork切れ、時間論理のwork帰属、評価エラーを検査した。wireのexact fields、負数・小数・過大count、矛盾した行と合計は共有TSテストで拒否する。nativeのstrictな要求と、Chromiumの実Wasmで14ケースも確認した。

## TLCとの照合で分かった違い

比較元はTLC2 2.19 revision 5a47802。ローカルcheckoutのWorker.addElement、ModelChecker、Tool.getInitStates、coverage.ActionWrapperとcoverageテストを読んだ。`-coverage 0`を使い、14仕様の判定と9仕様の行計数を検査した。invariant/action/temporal違反の3反例もTLCで再生した。deadlockはTLCの元仕様での判定を照合し、propertyへ変換した反例再生には含めない。

[最初の開発実行](../runs/0050-dev/error.json)では、重複するInitのcoverage行を誤解していた。TLC全体は生成3・新規2でも、Init行は`3:3`と表示する。さらに代入後のpredicateで候補を棄却すると、Initのsecondには棄却分も加わる。こちらのdistinctは保存した新規state数を保ち、TLCへ合わせて生成数に変えていない。

[次の実行](../runs/0050-dev2/error.json)では、活性違反の反例再構築によってTLCのInit/Next行が増えた。BFS全体が生成8でもInitが`2:2`、Advanceが`3:6`になる。こちらの行はBFSに限定し、時間論理のworkをunattributedWorkへ入れるため、TLCの再評価回数まで同一視しない。

最終の比較ではInitのfirstを完成候補数へ、Nextのfirst/secondを新規・完成候補数へ照合する。Initの新規数は全体のstate数とNextの新規数からも検査する。活性違反ではTLCの行数値を一致と数えず、判定・全体統計・反例を検査した。[修正後の開発実行](../runs/0050-dev3/result.json)も保存し、失敗runを編集していない。

## 計測しない経路と費用

最初はconst genericで計数処理を除いていたが、通常経路にも空のcoverage Vecと大きいReport返却型が残った。privateなgeneric recorderへ分け、無効時は状態領域を持たないNoCoverageとSafetyResult、有効時はWithCoverageとSafetyCoverageReportへ特殊化した。通常経路はcoverageの状態を保持しない。計測処理の有無で探索順やworkを変えない。

[最初のon/off測定](../runs/0050-on-off/result.json)と[最終構成の測定](../runs/0050-on-off-specialized/result.json)を保存した。下表は最終構成。prepared IRをnativeで反復し、20 sample、warmup 0.5 s、measurement 1 s。構文解析、IPC、JSON、process起動は含まない。測定中はこの作業のbuild/test/TLC/profileを並走させていない。

| ringの状態数 |          off平均（95%区間） |          on平均（95%区間） |
| ------------ | --------------------------: | -------------------------: |
| 4            |     0.610 µs（0.607-0.612） |    0.757 µs（0.710-0.815） |
| 1,000        | 98.600 µs（95.425-102.175） | 93.909 µs（93.060-94.789） |

小さいモデルでは約24%の追加費用になった。1,000状態のonが小さかったことを、coverageによる高速化とは判断しない。別の特殊化されたコードと共有機上の測定で、ゼロオーバーヘッドや一般の速度比は保証しない。Wasmの時間は測定していない。

通常経路の既存14本も[before](../runs/0050-controls-before/result.json)、[初期構成](../runs/0050-controls-after/result.json)、[最終構成](../runs/0050-controls-specialized/result.json)で確認した。[最終比較](../runs/0050-controls-specialized-comparison/result.json)には6件の3%以上の後退が残り、点推定の範囲は-11.3%から+19.6%だった。計測しない型を分けたことを、全体性能の改善や後退原因の特定へ読み替えない。

同じrelease設定のbinaryはnativeが2,008,048 → 2,024,608 bytes、Wasmが2,304,872 → 2,331,928 bytesになった。新しい依存は追加していない。

## 回帰と運用

crate fmt/clippy/test、just check/browser、portable-build/test、既存の安全性テスト213件、runtime-build/test、evaluate/replayを通した。coverageの14ケースをportable:testへ組み込んだ。npmの単独consumerは新しいhelperを実行し、型宣言も検査する。単独rtlc consumerも新しいRust APIを実行した。7 crateの配布検証も通した。

mutationは109件の保存結果の再利用で、新しいRust coverageのmutation検出力の測定ではない。SUPPORT.mdをaction coverageと式coverageへ分け、Jevの比較元へrecorderの実装を追加した。30比較で矛盾・未評価は0件。stagedとオフラインの鮮度も確認した。

Criterionは14本の3 runと4本の2 run、計215 artifactをdigestとestimatesへ照合した。以前の27本の保存runも同じ検証器で読める。source/binary hash、入力、raw TLC log、失敗と後退の結果を保持している。

```sh
just coverage-test
just coverage-eval evals/runs/new-coverage /path/to/java /path/to/tla2tools.jar
just coverage-replay evals/runs/new-coverage evals/runs/new-replay /path/to/java /path/to/tla2tools.jar
just coverage-controls new-controls evals/runs/new-controls
just coverage-bench new-cost evals/runs/new-cost
```

controlsとcostのstage名は分ける。出力先は毎回新しいdirectoryを指定する。
