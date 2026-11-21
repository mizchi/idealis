# crateのチェックリストを同期する

各crateのSUPPORT.mdを対応範囲の一覧とする。`[x]`は記載条件内で対応、`[ ]`は未対応・対象外。部分的な対応は条件を同じ行へ書く。未チェックは実装予定の約束ではない。

SUPPORT.mdはRustのcrate archiveにも同梱する。配布検査では9つのarchiveすべてにチェックリストがあることを確認する。

| crate           | チェックリスト                                                                 |
| --------------- | ------------------------------------------------------------------------------ |
| rtla            | [構文解析とAST](../crates/rtla/SUPPORT.md)                                     |
| rtla-compiler   | [ASTから有限IRへの変換](../crates/rtla-compiler/SUPPORT.md)                    |
| rtlc            | [有限IRの探索・性質・反例](../crates/rtlc/SUPPORT.md)                          |
| idealis-core    | [値・Model/Driver・履歴・ホスト・protocol](../crates/idealis-core/SUPPORT.md)  |
| rusmt           | [SMT-LIBと求解](../crates/rusmt/SUPPORT.md)                                    |
| ralloy          | [静的Alloyと固定instance](../crates/ralloy/SUPPORT.md)                         |
| idealis-cli     | [JSONLによるモデル差分セッションの実行](../crates/idealis-cli/SUPPORT.md)      |
| idealis-connect | [生成・縮小・再生・差分比較](../crates/idealis-connect/SUPPORT.md)             |
| roracle         | [実装登録・差分検査・保存oracle再生・比較ベンチ](../crates/roracle/SUPPORT.md) |

## コミット前と CI

`just support-check` は作業ツリーの crate 一覧、対応・未対応の宣言、リンク先を検査する。CI と同じ検査を stage 済みの内容に対して行うには `just support-check-staged` を使う。`just support-hooks` でこの検査を pre-commit に設定できる。いずれも API キーは不要。

## 任意の Jev レビュー

README や実装とチェックリストの記述を照合したいときは、次のコマンドを使う。

```sh
just support-plan
just support-sync
just support-review
```

`support-plan` は送信予定の比較件数と文字数を表示する。`support-sync` は、比較した抜粋・質問・モデルが同じ判定を台帳から再利用し、残りだけを Jev に問い合わせる。新しい判定は crate ごとの台帳 `.jev-lint/checklists/<crate>/` に1ファイルとして追記する。モデルは `jev-1.14.0`。API キー（`TYPESAFE_API_KEY` または `TYPESAFEAI_API_KEY`）が未設定なら、警告して生成をスキップする。

`support-review` は台帳だけを使ってオフラインで再生し、判定のない比較は `unrecorded` として合格にしない。部分 stage のレビューには `just support-sync-staged` と `just support-review --staged` を組み合わせる。台帳は commit する。ファイル名が一意で既存ファイルを書き換えないので、ブランチ同士のマージで衝突しない。今のチェックリストが使わなくなった判定は、main で `just support-compact` を実行して1ファイルにまとめる。コミットやマージに必要なのは人が管理するチェックリストで、Jev の判定は必須ではない。Jev の結果は助言として読み、矛盾の候補や未確定の判定は根拠を確認する。明示的にレビューした際は、古い記録や未完了の検査を終了コードで識別できる。

新しい crate を追加するときは SUPPORT.md と README を作り、`scripts/check-support.ts` の catalog に比較する実装箇所を追加する。
