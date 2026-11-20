# 0070: TLA+ frontendの残る互換性

基準は328aeeef。依頼された残りを一つの進行表で管理し、各段階で独立SANY/TLC比較、native/Wasm再生、境界と公開契約を検査する。単に構文を読み飛ばすことや、AST受理を実行対応として数えない。proofは構文解析と構造保持を対象とし、証明の自動検証は対象外。有限モデル検査の探索範囲・work予算・bounded/unknownの契約を保持する。

## 進行表

- [x] ASCII operator語彙・alias・優先順位を固定1.7.4の全表へ合わせる。
- [x] Unicode operator/bullet/CASE/関数/量化aliasを固定1.8.0の表へ合わせる。
- [x] 1.7.4/1.8.0のsyntax選択を明示し、従来APIのdefaultを維持する。
- [x] 記号WITH左辺、label引数、一般のsubexpression selectorをASTへ保持し、参照を解決する。
- [x] 引数付きINSTANCE、LET内INSTANCE、nested moduleを解析・連結する。
- [x] LAMBDA・RECURSIVE宣言を解析する。
- [x] 一般の高階operator・再帰を予算付きで実行し、非停止を成功にしない。
- [x] proof構文とASSUME/PROVE等を解析して構造を保持する。
- [x] IR v2でモジュール連結・設定置換を実行する。
- [x] IR v2でSelectSeq/SortSeq/Assertのsourceをlowerする。
- [x] unsupported/syntax診断を対象構文別に整理する。
- [x] 公式corpusで受理だけでなくAST構造を正規化比較する。

## 評価

355公式構文caseと既存のprefix/記号対照に加え、各新機能の正常/異常/境界を先に固定する。参照jar、corpus revision、source/binary hash、raw出力を保存する。TLC 1.7.4と1.8.0の世代差と参照自身の例外を分け、失敗したrunを上書きしない。新しいAST要素はtyped契約に定義し、compiler/analysis/rewrite等の網羅性を更新する。標準operatorの再定義や名前の捕捉、誤ったarity、level、module境界も検査する。

実行対応は少なくとも正常/違反モデルをIR v1/v2とTLCへ照合し、状態数・判定・反例を再生する。再帰・高階はcaller/calleeのbinder、capture、prime、old、work、depthの境界を検査する。sourceの深さ制限やwork上限を黙って緩めず、非停止や予算切れは未確定として返す。

段階ごとにRed/Greenとsourceの変更を記録する。最後にSUPPORT/README、Jev live/offline/staged、check/browser、rtla/portable/compiler/safety/analysis、runtime build/test、evaluate/replay、配布consumerを確認する。進行中の段階と未達項目を残し、全体完了と個別段階の完了を区別する。

実装と独立比較の結果は[0070の記録](../iterations/0070-tla-coverage.md)へ保存する。終了時の全チェックは別のverification runで記録する。
