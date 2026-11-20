# 0048: 量化fairnessと標準演算

## 判断

[計画](../plans/0048-fairness-operators.md)の有限fragmentを採用する。Rust frontendにSPECIFICATION内の量化fairness、SelectSeq、SortSeq、Assertを追加し、native/Wasmとブラウザへ接続した。既存のTS frontendのソース対応範囲は変更していない。

fairnessのdomainは定数の有限集合に限定する。callbackは可視の非再帰operator名で、SelectSeqは1引数、SortSeqは2引数。一般の高階引数・LAMBDA・再帰・state依存の時間論理domain・量化したInit/Nextの一般的な分解は未対応。RandomElement、TLCGet、TLCSetも残す。

## 比較と反例

比較元はTLC2 2.19、revision `5a47802`。ローカルcheckoutの同revisionにあるSequences.SelectSeq、TLC.SortSeq/Assert、SpecProcessorとLivenessを読み、実行結果へ照合した。SortSeqは新要素と直前の要素を比較する挿入順を保持する。常に真の比較では入力を逆順にし、常に偽なら入力順を保持する。strict比較で同順位の順を保ち、nonstrict比較で同順位を移動する。標準sortへ置き換えない。AssertはTRUEの場合もmessageを評価する。

[固定入力](../../crates/rtla-compiler/tests/operators-support.json)の52ケースを[最終実行](../runs/0048-operators-final2/result.json)と[保存結果の再実行](../runs/0048-operators-replay/result.json)で検査した。native/Wasmはコンパイル結果・応答全体が一致する。対応する結果はTLCの判定と状態・生成数へ照合した。エラーは分類を比較し、診断文や拒否phaseの完全互換とはしない。

| native/Wasmの判定  | 件数 | 検査した範囲                                                                        |
| ------------------ | ---: | ----------------------------------------------------------------------------------- |
| complete           |   32 | 列の値・順序・重複、空列、関数表現、scope・prime、別module/INSTANCE、fairnessの対照 |
| liveness-violation |    4 | fairnessなし、existsの不足、空forall、間欠的な有効化のWF                            |
| model-error        |    8 | Bool以外のcallback、文字列の列、Assert、空existsのfairness                          |
| unsupported        |    8 | import・arity・再定義、state依存domain、一般の高階引数                              |

unsupportedのうち、高階operator引数の対照はTLCではcomplete、state依存domainの対照はTLCではmodel-error、他の6件はTLCでもinput-errorだった。未対応拒否を正常検査と数えていない。

4件の活性反例は、stateとloopを固定し、元のNextまたはstutteringであることをTLCで確認した。再生用propertyは元のSPECIFICATIONと性質の否定を併せて要求する。公平性のsource断片を正規表現で切り出す方式をやめ、quantifier・wrapper・別名を保った。探索のIRからfairnessを生成し直してはいない。

## 開発中に分かったこと

[最初のTLC先行比較](../runs/0048-reference/result.json)では、間欠的な有効化のSF対照も活性違反になった。Tick自体が進む公平性を仕様に置いていなかったためで、TickのWFを追加した。[修正途中の比較](../runs/0048-reference-tick/result.json)はFairTickを参照より後に定義してSANYに拒否された。[修正後の50ケース](../runs/0048-reference-fixed/result.json)は期待と一致した。以前のrunは編集していない。

空existsのfairnessについてTLCはtautologyのモデルエラーを返したが、実装はcompleteにしていた。時間論理poolのAnd/OrでBool定数を簡約し、偽のfairnessがこの診断へ到達するよう修正した。fair behaviorの存在を一般に別証明する機能ではない。

[開発実行](../runs/0048-operators-dev/error.json)では、TLCが標準AssertのLET再定義を拒否することが分かった。module解決側へ拒否を追加した後も、[次の実行](../runs/0048-operators-final/error.json)とChromiumでは単一module用のcompile経路が受理していた。両経路の拒否を回帰で固定し、最終実行で確認した。

## 費用

[Criterionの27本](../runs/0048-operators-criterion/result.json)は、既存17本と追加5モデルのcompile/check 10本。20 sample、warmup 0.5 s、measurement 1 s。計測中にこの作業のbuild・test・TLCを並走させていない。通常の開発用Macでの基準値で、専用測定機ではない。

| 固定モデル                |     AST → IRの平均（95%区間） | 生成済みIRのcheckの平均（95%区間） |
| ------------------------- | ----------------------------: | ---------------------------------: |
| SelectSeqの順序・重複     |    22.769 µs（22.600–22.925） |            0.901 µs（0.891–0.916） |
| SortSeqの昇順             |    24.072 µs（23.942–24.202） |            0.919 µs（0.918–0.921） |
| Assert(TRUE, message)     |    11.154 µs（11.074–11.232） |            0.481 µs（0.474–0.491） |
| forall WF                 |    96.516 µs（95.002–98.247） |         62.887 µs（62.138–63.792） |
| 間欠的な有効化のforall SF | 219.431 µs（217.485–221.346） |      929.097 µs（922.751–936.215） |

parser、IPC、process起動を含まない小さな固定モデルの費用で、TLCとの速度比や一般のモデルでの性能改善は主張しない。量化SFのtemporal検査は列演算より費用が大きく、今後のprofiling対象として残す。意味論を変える高速化は採用していない。

保存した113 artifactをdigestとestimatesへ照合した。比較実行とbenchmarkで共通の272 sourceはすべて同じdigestだった。以前の17本・12本の保存runも拡張した検証器で読める。

## 回帰と運用

callbackのslot、arityを含む40変数の上限、外側scopeでのsequence評価、未実行branchの不正IRを検査した。長い選択と常に真の挿入比較、fairnessの展開でworkが切れたときはboundedを返す。Assertのmessage付きエラーと、真偽どちらでもmessage内のゼロ除算が先に伝播することも確認した。

crateのfmt/clippy/test、native/Wasmの契約、52ケースを含むChromium、既存の安全性テスト213件、just check/browserを通した。runtimeのnative/Wasm/browser、既存評価の実行・再生、npmと7 crateの単独consumerも確認した。mutationは既存の109件を再利用した結果で、新演算のmutation検出力の評価ではない。

SUPPORT.mdとJevの比較対象を更新した。30比較で矛盾・未評価は0件。オフラインの鮮度検査と、staged記録の鮮度も確認した。対応の追加は有限fragmentの拡張であり、全TLA+互換や実システムでの未知バグ発見を示すものではない。
