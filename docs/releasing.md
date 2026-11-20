# 配布と公開

バージョンは npm / Cargo とも `0.1.0-alpha.0`。registry への公開はまだ行っていない。

```sh
just package
```

`.artifacts/packages/` に npm `.tgz`、Cargo `.crate`、standalone Wasm、Wasm の SHA256/ABI manifest を作る。package-test は一時的な別 consumer に archive を導入し、ソルバーを入れない TS/Wasm、CLI の終了コード0/1/2、Cargo の offline build/run を検査する。consumer-result.json に結果を残す。

npm の files はライブラリ、CLI、Wasm、docs に限定する。eval の原本・run・test や開発用 binary を同梱しない。`fast-check`、`tla-ts`、`z3-solver`、`fflate` は optional peer dependency。コアの導入には不要で、使用する adapter に応じて導入する。

## 初回の registry 登録

初回 npm publish と Trusted Publisher の登録はアカウント所有者が行う。pack と consumer 検証を済ませてから実行する。

```sh
npm login --auth-type=web
npm publish .artifacts/packages/mizchi-idealis-0.1.0-alpha.0.tgz --access public --tag next
npm trust github @mizchi/idealis --repo mizchi/idealis --file release.yml --allow-publish -y
npm trust list @mizchi/idealis
```

`npm trust` は npm 11.15+ と2FAが必要。公開と OIDC の仕組みは [npm の公式説明](https://docs.npmjs.com/trusted-publishers/) を参照。`v*` タグの保護は repo 管理者が ruleset で設定する。設定変更と初回 registry 書き込みは今回自動実行していない。

Cargo の初回公開はアカウントと crate 名の確認後に行う。

```sh
cargo login
cargo publish --locked --manifest-path crates/rtla/Cargo.toml
cargo publish --locked --manifest-path crates/rtlc/Cargo.toml
cargo publish --locked --manifest-path crates/rtla-compiler/Cargo.toml
cargo publish --locked --manifest-path crates/idealis-core/Cargo.toml
cargo publish --locked --manifest-path crates/rusmt/Cargo.toml
cargo publish --locked --manifest-path crates/ralloy/Cargo.toml
cargo publish --locked --manifest-path crates/idealis-connect/Cargo.toml
cargo publish --locked --manifest-path crates/idealis-cli/Cargo.toml
```

この手順は crate registry への公開を行う。Cargo の自動公開は未設定で、GitHub Release に `.crate` を添付する構成。

## 次回以降

`packages/idealis/package.json` / Cargo.toml の version を同じ新しい値にし、Cargo.lock と CHANGELOG を更新してチェックを通す。初回に `0.1.0-alpha.0` を公開した後は、同じ version のタグで再 publish しない。次の `0.1.0-alpha.1` 等へ上げる。

`v<version>` の push は release.yml を起動する。tag / npm / Cargo の version 一致、型・テスト、pack、単独 consumer、ブラウザを検査してから OIDC で npm publish し、Wasm と archives を GitHub Release へ添付する。pre-release は next、安定版は latest。NPM_TOKEN は使わない。

registry へ公開済みの version は使い回さない。途中まで公開して失敗した場合は状態を確認し、次の version で再リリースする。

## CI の分割粒度

PR は1ジョブで TS / Rust / ブラウザ / mutation / 実バグ評価 / consumer を順に検証する。push の自動実行は main に限り、PR branch の push と pull_request の二重起動を避ける。

4800件の portable 評価、runtime 性能評価、4 runner の cloud sharding はそれぞれ workflow_dispatch で残す。重い評価を明示的に実行でき、sharding の manifest / 欠落・衝突検出も保持する。PR で runner を prepare → shard → merge と3段階待つ構造を外した。GitHub の割当待ちそのものを短縮する保証ではない。

`rtlc`の分離後は`rtlc-<version>.crate`と`idealis-core-<version>.crate`を同梱する。packageは一時workspaceで両者を同時検証し、Cargoの一時registryで依存を解決する。展開したarchiveのconsumerは未公開のrtlcを`[patch.crates-io]`でローカルへ結び、rtlc単独の依存graphにidealis-coreが入らないことも確認する。通常のrepoは独立manifestのままとし、native/Wasm targetの位置を変更しない。この処理はCargo registryへのpublishではない。

`rtla`は独立した`rtla-<version>.crate`、`rtla.wasm`、SHA256/ABI manifestを追加する。同じ一時workspaceで4つのcrateを検証し、parser単独consumerの依存graphにrtlc/idealis-coreが入らないことを確認する。npmでは`@mizchi/idealis/rtla.wasm`を解決し、solverがない別consumerで解析する。registry初回登録は上の手動手順を使い、package検証からpublishは実行しない。

`rtla-compiler`のarchiveはrtlaのASTをrtlcのIRへ変換する。idealis-core consumerは未公開のrtla・rtla-compiler・rtlcを`[patch.crates-io]`で解決し、ソースからのコンパイルと検査も実行する。parserとcheckerの単独consumerは従来の独立性を検査する。

`rusmt`は5つ目の独立crate。`rusmt-<version>.crate`、`rusmt.wasm`、`rusmt-manifest.json`を配布物に加える。rusmtのarchiveだけを別ディレクトリへ展開し、SAT、UNSAT、unknown、未対応を検査する。依存graphへidealis-core、rtla、rtlc、Z3が入らないことも確認する。npmのSMT backendへの接続は追加していない。

`ralloy`は6つ目の独立crate。`ralloy-<version>.crate`、`ralloy.wasm`、`ralloy-manifest.json`を加える。archiveだけを別consumerへ展開し、2-cycleの反例、予算切れ、不整合なfact、未対応構文を検査する。依存graphに他のidealis crateやZ3が含まれないこと、Wasmにホストimportがないことも確認する。npmには`@mizchi/idealis/alloy`と`@mizchi/idealis/ralloy.wasm`を含め、別consumerで探索と固定instanceの照合を検査する。汎用backendやCLIにはまだ接続していない。

`idealis-cli` は `idealis-connect` を使う JSONL 実行口で、それぞれの crate archive を配布する。`idealis-connect` はarchiveと`idealis-connect.wasm`、SHA256/ABI manifestを配布し、npmへ`./testing`と`./testing.wasm`を加える。別consumerで生成・縮小・再生・差分比較、Rustのstateful counter、依存graphの独立性とホストimport不要を検査する。registry publishはpackage検証から実行しない。
