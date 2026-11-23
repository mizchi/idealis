# roracleの対応範囲

## 対応

- [x] Rustの`#[test]`から使うgroup・case・reference・test_function登録とassert_matched。
- [x] genericな入力・観測、PartialEqによる等値と有限許容値集合、serdeのreport保存。
- [x] matched/mismatch/invalid/unknown/errorの区別、空suite・空許容値・重複IDの非成功扱い。
- [x] 比較の集約（重い順にerror・mismatch・unknown・invalid・matched、空はinvalid）を`spec/models/roracle-report.model`に記述し、比較0〜3件の156通りで実装がモデルの事実を満たすことをテストで照合する（0093）。
- [x] 保存入力と保存referenceを使い、reference callbackを呼ばないreplay。ID・件数・保存判定の整合性検査。
- [x] referenceと速度baselineの分離、preflight後のwarmup・全測定呼び出しの照合と失敗時の全速度比無効化。
- [x] 固定sample/iteration、予定呼び出し数予算、開始順回転と逆順、raw ns・実行順・mean/median/min/max・median比の保存。
- [x] nativeのInstantとWasmで使うhost clock。ゼロ・逆行・累積ns overflowをunknownへする。
- [x] 既定buildはserde/serde_jsonだけに依存する独立crate。native単独consumerとWasmの差分・再生・host clock経路。

- [x] version 1の収集済みJSON観測batchを同じOracleGroupへ通すdispatchと同じ全応答を返すdispatch_json、native JSONLとホストimport不要のWasm ABI。入力16 MiB・4096 cases・256 implementations、空・重複・観測数のずれを拒否する。

- [x] JSON観測の借用と直接bytes serialization、Wasm出力bufferの再利用。Value/bytes・単独/32 cases/入れ子/unknownのCriterionベンチ。JSON objectのkey順は保証しない。

- [x] wire専用visitorによる観測payloadの直接deserialize。公開Observationのserdeに対し、kind/field・順序・重複・null・数値・2要素sequenceの受理/拒否と保持した観測を検査する。公開型と全reportは保ち、不正variantの詳細診断文は固定しない。

- [x] dispatch_valueでcanonical JSON Valueを同じRequestとOracleGroupへ通す。16 MiBのcanonical serialization sizeとparser相当の入れ子制限を検査し、過深入力は再帰dropを避ける。dispatch_value_to_jsonは同じ検査を通し、全応答をJSON文字列へ直接出力する。bytesとの全report比較と拒否入力を検査する。

- [x] 開発用typescript featureのtypescript::definitionsでRustのrequest/report型からTS宣言を生成する。既定buildはts-rsやNode bindingを要求しない。生成型はobject形式を表し、serdeの実行時制約やlegacy sequenceの全受理範囲をTS型だけで表さない。

## 非対応・対象外

- [ ] PBTの生成器・縮小・seed互換。idealis-connectの役割であり、このcrateでは提供しない。
- [ ] solverやcheckerの内蔵、自動的な観測の正規化、任意言語のbinding。
- [ ] 信頼区間・統計検定・Criterion互換のrunner/proc macro、並列測定やsharding。
- [ ] callbackのtimeout・panic捕捉・プロセス隔離、共有可変入力の検出、保存データの真正性検証。

## 制約と根拠

- [x] referenceが正しいことや実装との独立性はホストが確認する。照合一致は実行した入力と射影した観測の範囲。
- [x] 測定は関数呼び出しと出力構築を含み、比較と出力破棄は含まない。初期化を含めるかはadapterが決めscopeへ記録する。
- [x] 集約はerror > mismatch > unknown > invalid > matched。非成功の保存値も保持し、replayは保存判定を再計算する。

[README](README.md)、[比較とreport](src/report.rs)、[groupとreplay](src/group.rs)、[測定](src/bench.rs)、[JSON境界](src/wire.rs)、[観測visitor](src/wire_observation.rs)、[Wasm ABI](src/wasm.rs)、[挙動テスト](tests/oracle.rs)、[bytes互換テスト](tests/bytes.rs)、[Criterion](benches/wire.rs)を参照。

Value境界の根拠は[Valueテスト](tests/value.rs)。TS宣言の原本は[型生成](src/typescript.rs)、再生成の入口は[example](examples/typescript.rs)。napi-rs bindingはworkspaceのeval試作で、このcrateの配布bindingには含まない。
