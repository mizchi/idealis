# idealis-cli

モデルの許容観測と実装の観測を比較する Rust CLI。標準入力から1行ずつ JSON を読み、同じ `idealis_connect::Engine` に渡して応答を標準出力へ返す。各言語のホストは実装を駆動し、観測を CLI に渡す。

```sh
echo '{"version":1,"request":{"kind":"compare","oracle":{"kind":"allowed","values":[1,2]},"targets":[{"id":"counter","outcome":{"kind":"value","value":3}}]}}' | cargo run --quiet --manifest-path crates/idealis-cli/Cargo.toml
```

この例では counter の観測値 3 が許容集合から外れるため、応答の `result.status` は `mismatch` になる。生成・縮小・再生ではプロセスを開いたままにし、各 `Evaluate` に対応する ID と判定を `resume` で返す。

[ライブラリとプロトコル](../idealis-connect/README.md) · [対応範囲](SUPPORT.md)

## TLA+ ファイルを検査する

`idealis` は Rust の単独バイナリとしてビルドできる。npm の `@mizchi/idealis` には同じバイナリの WASI 版を同梱する。どちらも Java は不要。

```sh
cargo install --path crates/idealis-cli --bin idealis --locked
idealis tla Counter.tla
idealis tlc -workers 1 -config Counter.cfg Counter.tla
```

- `tlc` はモジュール名に `.tla`、設定名に `.cfg` を補う。設定を省略するとモジュールと同じ場所の同名の設定を読む。`-deadlock` はデッドロック検査を無効にする。`-modelcheck` でモデル検査を明示でき、`-h` で対応オプションを表示する。native の `-workers` は 1〜256 または `auto`（利用可能な並列数、最大256）、npm 版は 1 または `auto`（1 worker）を受け付ける。
- 終了コードは探索完了が 0、ASSUME 違反が 10、デッドロックが 11、invariant 違反が 12、property 違反が 13。設定で指定した invariant / property 自体が定数 FALSE の場合は設定エラー 151。反例は既定で JSON 表示。`--fulltrace` は各状態の action と変数名・値を全表示する。idealis 独自の `--expect-violation` はデッドロック・invariant/property 違反で 0、違反なしで 1。入力エラーや予算切れは引き続き非ゼロになる。完了時の要約には状態数・深さ・検査時間を表示する。`-difftrace` は初期値と各ステップで変化した変数を表示する。v1.7.4 と同様に、深さ2以上の安全性違反では末尾の状態を全表示する。探索は深さ 1,000、状態 100,000、work 10,000,000、temporal node 100,000 を上限とし、打ち切りと初期状態なしは 255 で終了する。
- `tla` はファイルごとに構文を解析し、実行可能な言語部分の名前解決と level 検査を行う。`-s` は構文解析のみ、`-l` は level 検査を省く。公式 v1.7.4 に合わせ、未解決の識別子と検出した式の level 違反は標準出力の `Semantic errors:` で診断する。この診断が出ても終了コードは 0。`-stat` / `-STAT` は同版の空の統計フックに合わせ、追加出力を行わない。
- ローカルの直接の `EXTENDS` とモジュール直下の `INSTANCE` はルートファイルと同じディレクトリから読み込む。探索結果と一部の終了コードを公式 TLC/SANY v1.7.4 と比較している。定理・proof を含む入力は既定の意味解析と有限検査で対象外として拒否する。全オプション、全意味解析、診断文やトレース書式の完全互換は対象外。`-simulate`、`-dfid`、`-tool` など未対応のオプションはエラーになる。

開発時は `rustup target add wasm32-wasip1 wasm32-unknown-unknown` で npm ビルド用ターゲットを用意する。公式版との比較は `just cli-compare java /path/to/tla2tools.jar` で別途実行する。
