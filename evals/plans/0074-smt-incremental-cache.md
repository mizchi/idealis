# 0074: SMT contextの増分解析とcheck cacheの評価

79a124113（増分解析）と32fcaca19（check cache）を、0042と同じ水準で評価する。両commitは実装済みで、開発中に探索的なCriterionを1回測り、その中央値を[記録](../iterations/0074-smt-incremental-cache.md)に載せた。採用条件は数値を見た後に決めるので、閾値は探索値に合わせず、0042と同じ形の保守的な値に固定する。探索値は採否の根拠に使わず、以下の保存runだけで判断する。

## 固定する入力と版

- baselineはbcf018be1のライブラリ。candidateはこのplanを含むcommitのライブラリ。
- `benches/solver.rs`は両版でビルドできる18ケースに限る。既存14ケースと、増分assertの4ケース（`context-assert-32/256`、`context-constant-32/256`）。cacheの9ケース（`cache-miss-4/32/128`、3 fixtureの`script-cache-cold/warm`）は旧版でビルドできないので`benches/cache.rs`へ分け、candidateだけで測る。比を同じbinary内で取るため、対照の`prepared-4/32/128`と`script-prepared-*`の6ケースも同じbenchに置く。
- bench binaryは`CARGO_PROFILE_BENCH_STRIP=none`で両版をビルドし、保存したbinaryを全測定で再利用する。baselineは同じ`benches/`とfixturesを旧ライブラリと組み合わせる。
- Criterion 0.8.2、20 samples、warmup 1秒、measurement 2秒、median、95%区間。raw samplesを各runへ保存する。他の重い検査と同時に測らない。専用ホストではない。

## 測定順と採用条件

solver benchはbaseline→candidate（開発）、candidate→baseline（逆順の確認）の4 runを独立ディレクトリへ保存する。cache benchはcandidateの開発・確認の2 runを保存する。

- 主指標: `context-assert-256`と`context-constant-256`は両組でcandidate/baselineの中央値比0.5以下。`context-scopes-32`と`script-parse-repeat-64`は0.8以下、`script-parse-scopes-32`は0.9以下。
- 後退: 残る13ケースは両組で1.1以下。超えた場合は1組だけ再測定を追記planで宣言し、それでも超えれば後退として記録する。好都合な組を選び直さない。
- cache: `cache-miss-N`/`prepared-N`の比を固定費として報告し、合否にしない。warmはcache-coldより遅くないことだけを確認する。小さい問い合わせの後退を理由に既定無効を維持する。

未達は未達として残し、閾値を下げない。

## 正しさ

開発seed 0x74a1、確認seed 0x74c3。どちらも過去のrunやテストで使っていない。同じcandidate source/binaryで両phaseを実行する。

1. 0042の検査器（smt-scripts corpus 32/64 script、4 fixture、Z3の逐次scriptとSATモデル照合）を新seedで実行し、旧native/Wasmと全応答（work/statsとerror）を照合する。
2. context要求列: seedから宣言・式のpool・assert/push/pop/check/check-assumingの列を生成する（開発32系列、確認64系列、各24操作）。新旧のnativeとWasmの全応答を照合する。完了したcheckはZ3でstatusを照合し、SATモデルを束縛してZ3で再検査する。後から整数定数を束縛する式、未対応式、型エラー、予算0を含める。
3. check cache: 同じscriptと要求列を1つのcacheで実行する。cold/missは通常の応答と一致し、hitはworkが0で、同じqueryの無制限な新しい探索の回答と一致する。無効時は渡したsnapshotを変更しない。native/Wasmで全応答が一致する。

入力・全応答・Z3出力・モデル検査を保存し、両phaseを再生する。Z3が別のmodelを選ぶ差は再生で無視し、Rustのモデルは毎回再検査する。

## 範囲外

Z3・Wasmとの速度倍率、実用入力全体への外挿、学習状態の再利用はしない。Jevの`support-plan`/`support-sync`は助言として実行し、判定を台帳へ追記する。mergeとregistry publishは行わない。古いrunを上書きせず、失敗したrunも残す。
