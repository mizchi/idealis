# TLCへの差分検査

TLC、native、Wasmの判定・状態数・生成数を比較し、検出したtraceを元の仕様のNextとpropertyで再実行する。unknown、予算切れ、unsupportedや参照環境の不足を一致へ変換しない。

```sh
just tlc-diff-test /path/to/java /path/to/tla2tools.jar
just tlc-diff /path/to/java /path/to/tla2tools.jar development evals/runs/new-development
just tlc-diff /path/to/java /path/to/tla2tools.jar confirmation evals/runs/new-confirmation
just tlc-diff-replay /path/to/java /path/to/tla2tools.jar evals/runs/new-development evals/runs/new-replay
```

各出力先には新しいdirectoryを指定する。dedicated testはmodule不足・改変の拒否、空白を含む検索path、明示moduleの優先を実TLCで確認する。

## Apalache標準moduleの準備

上のjust recipeは先に`just tlc-modules`を実行する。公式[v0.56.1のApalache.tla](https://raw.githubusercontent.com/apalache-mc/apalache/v0.56.1/src/tla/Apalache.tla)を取得し、既存fold対照と同じSHA-256 `a9115b404e72c58fd085f81bc59cc918f8adac49feabfb57f8e285bc7ab5cfac`を検証する。検証したファイルだけを`.artifacts/tlc/standard-modules`へ保存する。既存cacheが改変されていれば停止し、黙って取り替えない。

TLCは`-DTLA-Library=<絶対path>`でこのdirectoryを検索する。元のTLC jar、仕様のsource/configと明示したinput.modulesは変更しない。モデルdirectoryのmoduleはTLC本来の検索順どおり優先する。標準moduleの原本はgitへ追加しない。

準備後の検査・replayはnetworkを使わない。直接nodeからrun.jsを呼ぶ場合は先に`just tlc-modules`が必要。manifestは標準moduleのURL・path・digestを保存し、開始・終了・replayで検証する。標準moduleの変更は別の参照条件として新しいrunで扱う。

155ケースの回帰は同じ仕様・seed・期待判定を保つ。新しいモデルの検出力比較とは区別する。過去のrunは記録したsourceへ戻して再生し、修正後の結果で上書きしない。
