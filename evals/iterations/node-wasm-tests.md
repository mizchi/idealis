# NodeでWasmの契約を検査する

通常CIとreleaseのWasm検査をNodeへ移した。[計画](../plans/node-wasm-tests.md)に従い、Playwrightで使うentryへassetBaseを渡せるようにした。同じ処理と期待値をNode test runnerとVitestから実行する。fetchは実HTTP serverへ接続し、WebAssemblyも実bytesを実行する。globalやDOMの模倣は使わない。Z3のNode初期化と終了も実APIを使う。

23件の共有契約には、Rustの9 Wasm成果物のhost import検査、ブラウザ向けbundle、Quint生成コード、portable frontendの有限対応・再帰・共有IR・symmetry、parserのエラー復旧、SMT、Alloy、PBT、SAT、native actor SUT、fuzzの保存応答、runtime、roracleのJSON ABIとtyped replay/clockを含む。完全探索、違反、bounded、unknown、拒否、反例を元の検査から引き継いだ。

NodeとVitestは各23件成功した。最終runの実行時間はNodeが約1.60秒、Vitestが約0.93秒で、Rust/TSのビルド時間は別。新しいworkflowのCI全体時間はまだ測定していない。

CI・release・portable・runtime workflowからブラウザのinstall/起動を外し、`just wasm-test`を通常の開発検査にした。Web Worker輸送とCOOP/COEPを含む実ブラウザ互換性は、手動の[`browser.yml`](../../.github/workflows/browser.yml)へ分けた。ブラウザ専用の試験は残し、今回のNode成功を実ブラウザの新しい実測結果には数えない。Vitest Browser Modeは採用していない。

検証結果:

- `just check`: Vitest/node:test各79件、lintと型検査が成功。
- `just wasm-test`: Node/Vitest各23件が成功。
- `just oracle-test`: Rust、Wasm、process、workspace catalog、旧snapshot名の読取りと新名のconsumerが成功。
- `just runtime-test`: native/Wasm/TSの比較とNodeへ移した実行経路が成功。
- `just package`: `roracle-0.1.0-alpha.0.crate`を含む8 crateとnpmの独立consumerが成功。
- 保存した評価の反例・scheduleを再生し、故障版の再現と修正版の成功を確認。
- 既存のmutation対象は95.4%。新しいWasmスイートの変異検出率ではない。
- Jevの67比較で矛盾・未評価0件。offline gateと8件の検査スクリプトのテストも成功。
- YAMLを解析し、ブラウザ実行はworkflow_dispatchだけのbrowser workflowへ集約したことを確認。

[`node-wasm-tests`](../runs/node-wasm-tests/evidence.json)へRed/Green、配布・再生のlogとsource digestを保存する。Redには相対URL等のNode未対応に加え、試験が指定した存在しないentryの失敗も含む。entry指定を修正してからGreenを確認した。最初の評価再生でディレクトリを渡したEISDIRも保存し、witnesses.jsonを指定した再生と区別した。

先行する`e02fa85a4`では`idealis-oracle`を`roracle`へ改名した。配置は`crates/oracle`を保ち、Cargo依存、Rust import、native/Wasmファイル名と配布consumerを更新した。過去のrunは編集せず、凍結済みsnapshotの旧Wasm名は読取り側で引き続き扱う。
