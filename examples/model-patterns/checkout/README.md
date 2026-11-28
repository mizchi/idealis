# 注文から決済まで

注文には正の金額を指定する。決済の承認は注文の金額と一致し、同じ承認が再送されても計上は一度。拒否には理由があり、拒否した注文は計上しない。注文1件、金額0〜2の有限モデルでこの規則を検査する。[モデル](checkout.ideal)

## 用語から型を生成する

リポジトリのルートで実行する。

```sh
cargo build --locked --manifest-path crates/idealis-check/Cargo.toml
checker=crates/idealis-check/target/debug/idealis-check
model=examples/model-patterns/checkout/checkout.ideal

$idealis-check generate "$model" glossary
$idealis-check generate "$model" typescript
$idealis-check generate "$model" pbt-cases
```

`entity Order` の説明は用語集と生成された TypeScript の型コメントに入り、決済の `variant Payment` はタグ付きの共用体になる。注文額 `Amount` と決済受領 `Receipt` は別の値型で、どちらも `units` を公開する。境界ケースとして整数、操作の継続、遷移の6組を得た。[生成された型](../../../tests/idealis-check/generated/checkout.ts)

## 検査した操作を実装で再生する

```sh
$idealis-check "$model" --checker all
$idealis-check witness "$model" CanCapture
$idealis-check witness "$model" CanReject
just build
node --test dist/tests/model-patterns/checkout.test.js
```

HTTP の実装は生成された `Domain` を実装する。再生側は各コマンドの直前に `idealis-check validate` を呼び、実行後に `/state` から注文金額、決済結果、計上回数を取得する。同じ承認をもう一度送ると正常版では1回のまま。人工故障版は3手目で2回になり、最初の差分が返る。金額を変えた承認は送信前に拒否する。HTTP で採取した中間状態を保存すると、実装を立ち上げ直さずに照合できる。[実装とテスト](../../../tests/model-patterns/checkout.test.ts)

`OrderDecides` は、注文の受付と承認が有効なままならやがて実行されるという弱公平性のもとで成立する。明示探索と TLC で確認し、公平性を外したモデルでは初期状態に留まる反例を得た。

## ITF ファイルから再照合する

`record-itf` は設定、初期状態、予定した操作、採取した全状態の JSON を ITF に変換する。記録元と異なるディレクトリでも、モデルを指定すると再照合できる。

```sh
"$checker" record-itf "$model" observations.json > checkout.itf.json
"$checker" replay-itf "$model" checkout.itf.json
```

入力のキーは `config`、`initial`、`commands`、`observations`。観測には実行前の状態を最初に入れ、その後に各操作直後の状態を並べる。照合の結果は一致が終了コード0、差分が1、記録不足や不正な操作が2。[CLI の仕様](../../../crates/idealis-check/README.md#save-and-replay-implementation-observations)

`recordReplay` の `recording` を `toReplayItf` に渡すと、各フレームに設定、実行した操作、実測した中間状態が入る。保存したファイルは `fromReplayItf` と `replayRecording` で読み直せる。[変換と復元](../replay-itf.ts)

```ts
await writeFile(file, JSON.stringify(toReplayItf(result.recording), null, 2));
const saved = fromReplayItf(JSON.parse(await readFile(file, "utf8")));
const verdict = await replayRecording(checkerPath, saved, (state) => state);
```

型付きモデルの JSON 表現を ITF のレコードに格納する。整数は `#bigint`、未設定の単値は空の `#tup` に写す。再照合用のメタデータには期待する初期状態と予定した操作列を含める。最初の差分で取得を止めた記録も、正常な観測が途中で欠けた記録もこの情報で区別できる。適用範囲は有限の状態再生プロファイル。[ITF の仕様](https://apalache-mc.org/docs/adr/015adr-trace.html) · [回帰試験](../../../tests/model-patterns/checkout.test.ts)

## Lean で状態を検査する

```sh
$idealis-check lean-oracle "$model" examples/model-patterns/checkout/Checkout.lean
```

Lean は参照評価器から受け取る各到達状態で、承認額が注文額と等しく、計上が1回であることを独立に検査する。正常な9状態で成立し、再送で2回計上するモデルでは違反を返した。[Lean の命題](Checkout.lean)
