# rusmtの対応チェックリスト

旧rsmtの拡張を追跡する。未チェックは未対応・対象外で、実装予定を約束しない。チェック済みは、入力の受理だけでなくSAT/UNSAT・具体値・予算切れ・不正入力をnative/WasmとZ3で検査した範囲。試作と公開backendへの採用を分ける。対応済みの細部と資源上限は[対応表](../../docs/smt-subset.md)を参照。

## 対応済みの基礎

- [x] 任意精度Int、Bool、加減算、比較、等価・distinct。
- [x] 整数差分制約、無条件の整数定数代入。
- [x] Boolean式、Boolのite、同時束縛let。
- [x] SATの全変数モデル、UNSAT、work切れのunknown。
- [x] native/Wasm、JSONL、解析済みProblemのprepare/check/release。

## 0040で追加した範囲

- [x] 定数倍。正負・0・任意精度の係数を扱い、scaled differenceへ落ちる式に限定する。
- [x] `k * (x - y) <= c`を整数の丸めで正規化する。一般LIAとは区別する。
- [x] 整数のite。加減算・定数倍・比較・let内でも使い、展開数を制限する。
- [x] Rust/JSONでassertionの追加とpush/popを提供する。失敗した変更は既存contextを保つ。
- [x] Bool literalの一時assumptions。次のcheckへ持ち越さない。
- [x] 新機能のZ3差分、モデル照合、native/Wasm、ブラウザと単独consumerの検査。

確認用の140式と21件のcontext問い合わせをZ3に照合し、SATモデルとnative/Wasmの一致を検査した。[評価0040](../../evals/iterations/0040-smt-expansion.md)を参照。空and/orはZ3 5.4.1のSMT-LIB parserが拒否するため、既存の拒否動作を維持する。

## 0041で追加した範囲

- [x] SMT-LIB scriptのpush/pop、複数check-sat、Bool literalのcheck-sat-assuming。宣言とoptionは冒頭に固定する。
- [x] RustのScriptとJSON run-script。check順の結果列と、全checkで共有するwork予算。
- [x] 全コマンドの事前検査。後続エラーから部分的なSATを返さず、既存handleへ影響させない。
- [x] Z3の逐次実行と独立snapshotへの照合、モデル検査、native/Wasm、ブラウザ、単独consumer。

確認用67 scriptの588 checkでZ3と一致し、保存runを再生した。[評価0041](../../evals/iterations/0041-smt-scripts.md)を参照。checkなしの入力は空結果で、検査成功とは扱わない。

## 0074で追加した範囲

- [x] assertで追加した式だけを解析・コンパイルする増分context。新しい整数定数を束縛した場合は保持した構文木から全assertionを再コンパイルし、popはpush時のsnapshotへ戻す。
- [x] 累積sourceの1 MiB・20万token・入れ子64の上限を、全体の再解析と同じ順・同じ境界で検査する。
- [x] 既定無効のcheck cache。`sat`/`unsat`だけを保存し、build digest・保持上限・SHA-256付きsnapshot・merge・hit/miss/writeを扱う。
- [x] Rust/JSONの`check-incremental`と`run-script-incremental`、native/Wasmの完全応答一致とChromium。

全再解析の参照実装と400系列・1000回超のcheckで結果・work/stats・errorを照合し、cacheのhitは無制限の新しい探索の回答と照合した。人工故障12件を検出した。確認seedでは旧native/Wasmとの完全応答、contextの153 checkのZ3照合と107モデルの再検査、cacheのhitを照合し、runを再生した。solver benchは0074bで`prepared-128`の1組が閾値を超えた。同じbinaryの再測定（0074d）では18ケースすべてが閾値内だった。[評価0074](../../evals/iterations/0074-smt-incremental-cache.md)を参照。cacheは回答の再利用で、学習状態・理論状態の再利用ではない。

## 0076で追加した範囲

- [x] engine側のcheck cache store。`cache-open` / `cache-export` / `cache-close`と、incremental requestの`cacheHandle`。snapshotの経路は変えない。
- [x] snapshotを往復させた場合との結果・work/stats・cache stats・exportの一致（seedから生成した60系列・2400要求）。開く時のdigest検査、別build、容量、handleの独立、fieldの併用を拒否すること。native/Wasmの完全応答一致。

人工故障3件（writeの消失、開いた時のoptionの無視、古い状態のexport）を検出した。

## 非対応・対象外

- [ ] scope内や後続コマンドでの宣言、global-declarationsとoption変更。
- [ ] check-sat-assumingの任意Bool式。現在はBool literalのみ。
- [ ] reset/reset-assertions/exitなどのSMT-LIB scriptコマンド。
- [ ] define-funのmacro展開。
- [ ] SMT-LIBのget-model/get-value。モデル返却自体はRust/JSONで対応済み。
- [ ] Unicodeの引用識別子とSMT-LIB構文・optionの拡張。
- [ ] 一般LIA。例は自由変数を持つ`2*x + 3*y <= 7`。
- [ ] 探索途中の理論矛盾による剪定・理論伝播。
- [ ] CDCL、節学習、Boolean UNSATの改善。
- [ ] 学習状態・理論状態のincrementalな再利用。
- [ ] work切れからの再開と中断の契約。
- [ ] unsat core、unsat assumptions、proof certificate。
- [ ] 非線形Int、div/mod、abs、冪乗。
- [ ] Real/LRA、Real/Int混合。
- [ ] 引数付きUF、独自sort、理論の組み合わせ。
- [ ] BitVec。
- [ ] Array/select/store。
- [ ] String/正規表現。
- [ ] 代数的データ型。
- [ ] 浮動小数点。
- [ ] forall/existsと量化処理。
- [ ] 最適化・MaxSMT。
- [ ] 公開backend・CLIへの統合と、未対応/unknown時の明示的なZ3 fallback。

新しい理論は一括でチェックしない。演算・型・問い合わせ・返却モデルの範囲ごとに条件を固定し、[evals](../../evals/README.md)へ結果と制約を残す。
