# idealis-cli の対応範囲

- [x] 標準入力の各行を同一の idealis-connect Engine へ渡し、各応答を JSON と改行で標準出力へ書き、毎回 flush する。
- [x] モデルの許容値と各実装の観測を渡し、応答から対象ごとの照合結果を取得できる。
- [x] `idealis tlc` は .tla と .cfg を読み、有限検査を実行する。`-workers auto` と明示の `-modelcheck`、反例の変数差分表示 `-difftrace`、`-h` を扱う。探索完了は0、違反は種類別に10〜13。定数 FALSE の invariant / property は設定エラー151。予算切れと空の初期状態はそれぞれ bounded / empty と出力して255で終了する。--fulltrace は変数名と値を全状態について表示する。--expect-violation は通常完了を1、デッドロックと invariant/property 違反を0にする。
- [x] `idealis tla` は構文解析と限定した名前・level の検査を行う。検出した式の level 違反を診断し、`-s`、`-l` と v1.7.4 の `-stat` を受け、未対応オプションを拒否する。
- [ ] TLC/SANY の全オプション・全意味解析・出力形式との完全互換、Java へのフォールバック。
- [ ] SUT プロセスの起動、観測の自動収集。

[実行口](src/main.rs)と[差分の入出力テスト](tests/jsonl.rs)を参照。
