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

jev-lint 0.8.0のdocuments APIで、各READMEとチェックリスト、選択した実装境界の記述を比較する。比較箇所と全Rust source/test/bench・同梱JSON/TLA/CFG fixture・Cargo manifest/lock等の内容digestは`.jev-lint/checklists/*.manifest.json`と`*.record.json`へ保存する。モデルはjev-1.14.0に固定する。

## 変更したとき

```sh
just support-plan
just support-sync
git add crates .jev-lint/checklists
just support-check-staged
```

`support-plan`は送信する比較の件数・文字数を表示し、APIを呼ばない。`support-sync`は作業ツリーの対象箇所をJevへ送り、判定と使用量を保存する。TYPESAFE_API_KEYまたはTYPESAFEAI_API_KEYが未設定なら警告してスキップし、判定やmanifestを書き換えない。再判定を実行するときは環境へキーを設定する。キーは文書や記録へ書かない。

判定はsupports / contradicts / unrelated / insufficient_context。矛盾の候補は根拠を読んで修正し、未回答や文脈不足を整合へ読み替えない。記録を手で合格へ変更しない。ここで比較するのは宣言した箇所の意味であり、全ての実装の正しさや形式言語への完全互換性を証明しない。

部分stageを行う場合は、先に変更をstageしてから`just support-sync-staged`を使う。これはindexのblobからsnapshotを作るため、未stageの訂正でコミット内容の不整合を隠せない。新しいmanifestと判定記録もstageした後、`support-check-staged`で確認する。sourceの追加・削除と新しいcrateも一覧へ反映する。

## コミット前とCI

```sh
just support-hooks
just support-check
just support-test
```

`support-hooks`はGitのpre-commitへshimを設置し、追跡している`.jev-lint/hooks/pre-commit`を呼ぶ。他のhookがある場合は上書きせず、そこからこのhookを呼ぶ行を追加するよう案内する。別ブランチに追跡hookがない場合、shimは何も実行しない。

pre-commitは両方のAPIキーが未設定なら警告してスキップし、コミットを妨げない。キーがある場合はstagedのチェックリストと証拠をオフラインで検査する。スキップを検査済みとは表示しない。

明示的な`support-check` / `support-check-staged`とCIのオフライン再生は、APIキーなしでも実行する。CIはcheckoutしたファイルを対象にする。sourceやREADMEの変更、古い記録、判定の欠落・未確定・矛盾は非zero終了にし、再確認を求める。リファクタやコメントだけの変更でも、入力digestが変われば新しい記録が必要になる。全crateを無条件に対応済みとする検査ではない。

新しいcrateを追加するときはSUPPORT.mdとREADMEを作り、`scripts/check-support.ts`のcatalogへ比較する実装箇所を追加する。チェックリストの欠落・参照切れや宣言一覧のずれは、Jevを呼ぶ前に決定的に検査する。
