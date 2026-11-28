# 自動保存の応答を照合する

タブごとに一件の自動保存を送信できる。送信後に文書が編集されても応答は届く。応答を受信したタブは、次の自動保存を送信できる。[自然言語の仕様](spec.json) · [モデル](autosave.ideal)

### AUTOSAVE-STALE-RESPONSE

送信時の版が編集中の版と異なる応答を受け取っても、保存済みの版は変わらない。

`finish` は送信中の要求を取り除き、版が一致するときだけ `saved` を更新する。`StaleKeepsSaved` は古い応答で保存済みの版が変わらないかを検査し、`CurrentMarksSaved` は現行版の応答で保存済みの版が更新されるかを検査する。

## 規則から実装の差分を引く

リポジトリのルートで実行する。

```sh
cargo build --locked --manifest-path crates/idealis-check/Cargo.toml
checker=crates/idealis-check/target/debug/idealis-check
dir=examples/model-patterns/autosave

"$checker" traceability "$dir/spec.json" --rule AUTOSAVE-STALE-RESPONSE \
  --document "$dir/README.md"
"$checker" traceability "$dir/spec.json" --check "$dir/autosave.ideal" StaleKeepsSaved
"$checker" witness-step "$dir/autosave.ideal" finish \
  'before(tab.pending) != before(revision) and before(saved) = before(revision)'
just build
node --test dist/tests/model-patterns/autosave.test.js
```

モデルの版比較を外すと、送信・編集・旧応答の3手で違反する。さらに、正常モデルの `witness-step` は、新しい版を保存してから旧応答が届く5手の経路を返す。Node.js のテストは両経路を正常版と故障版に渡す。後者の経路では正常版が保存済みの版1を保ち、故障版は0へ戻す。採取した観測を `record-itf` で保存し、`replay-itf` で同じステップ・値の差分を再現する。[接続試験](../../../tests/model-patterns/autosave.test.ts)

`pending` のような `Tab -> lone Nat` は、観測 JSON に各タブのキーを含め、未送信のタブに `null` を入れる。実装の Map は未送信の行を持たないため、観測時にこの形へ写す。
