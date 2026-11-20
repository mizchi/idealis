# Nodeを通常のWasm検査環境にする

Wasmの意味論・ABI・反例再生・エラー復旧は、Nodeの実WebAssemblyで検査する。Playwrightの既存entryをNodeからも呼び、同じ入力と期待値を使う。fetchは実HTTP serverへ接続し、globalの差し替えやDOMの模倣は行わない。

通常CIと配布前検査からブラウザのinstall/起動を外す。実ブラウザ固有のWorker輸送・COOP/COEPと対応環境の確認は、明示的な手動チェックにする。Vitest Browser Modeは採用せず、既存のNode/Vitestの共有契約を使う。

- Rustのhost import不要のABIを実bytesから検査する。
- core、parser、SMT、Alloy、PBT、SAT、runtime、roracleとtyped oracleの既存の判定・未知・拒否・反例を維持する。
- portable frontendの完了・再帰・共有IR・symmetryを同じfixtureで検査する。
- ブラウザ向けbundleは引き続きesbuildで検査し、Node APIの意図しない混入を検出する。
- Node test runnerとVitestから同じ検査を実行し、`just check`、Wasm検査、mutationと配布consumer、Jev同期を確認する。

既存のbrowser実測記録は更新しない。Nodeの成功を実ブラウザ互換の新しい実測結果には数えない。移行前にNodeでの失敗を記録し、移行後の結果・コマンド・実行時間を別runへ保存する。CI時間の短縮は実測なしに倍率を主張しない。
