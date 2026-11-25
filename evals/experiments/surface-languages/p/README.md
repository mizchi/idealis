# P

`check.sh` は各モデルを P 3.1.0 でコンパイルし、PChecker の random 戦略で seed 110、256 スケジュールを実行する。各モデルの任意操作列は12ステップで、checker の上限は256スケジューリング点。上限到達は失敗として扱う。実測では5題材で指定数のスケジュールを実行し、正常版は0件、service の対照は `EventuallyServed` の反例を検出した。探索された経路に対する結果で、全探索や帰納法は対象外。[PChecker の仕様](https://p-org.github.io/P/getstarted/usingP/)

- permissions は秘密文書を読めるユーザーを必ず cleared に入れ、可能な残りの cleared を選ぶ。各操作後に `NoLeak`、`OnlyGranted` を検査し、`SecretRead` は一手の具体例を実行する。
- bank は残高の非負性を毎ステップ検査し、`Rich` は初期状態からの入金で確認する。amount は −4〜4 で走らせる。
- order はタグと二つの payload 欄を持つ積型を使う。タグの検査後に対応する欄を読むため、タグと payload の整合は型で保証されない。発送前後の全レコードを比較し、Fraud による取消は一手の具体例で確認する。
- sessions は `hasLock` とユーザー ID で任意の参照を表す。lock 後の logout で `LockedIsActive` の反例を示した後、別ユーザー2人がログインして `Full` に到達する。任意操作列で容量と管理者の制約を検査する。
- service は `Disjoint` と、待ち列が空でなければ hot になる monitor を検査する。対照では最後にキューを空にする処理を外し、待ちが残ったまま終了することを checker が検出する。終了時の drain は有限のテスト環境で与え、`serve` の弱い公平性を課した無限経路は評価範囲外。[P の monitor](https://p-org.github.io/P/manual/monitors/)

DDD の根拠は identity 0.5（区別した enum に個数を固定）、config 0.5（初期化で有効な設定を生成し、以後は更新しない）、ints 0.5（P の有界整数）、adt 0.5（タグ付きの積）、typeExclusion 0.5（タグの排他性を型が持ち、payload の整合は持たない）、step 1（ガード付きの状態更新）、lone 0.5（bool と値の組）。関係の join はループで 0.5、個数は `sizeof` で 1、活性は有限の実行で 0.5 とした。

構文木はコンパイラ内部の型付き表現を使い、命令的な関数本体を含むため 0.5。実際に検査した種類は有界探索。実装コードの生成は0で、ここで生成した C# は PChecker の検査用。実装の照合 1 と実行時の検査 0.5 は、公式の PObserve がサービスログを P の monitor へ変換できることに基づく。この実験では PObserve の実装照合を実行していない。[PObserve の公式手順](https://p-org.github.io/P/advanced/pobserve/gettingstarted/)
