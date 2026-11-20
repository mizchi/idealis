# modeltからidealisへの移行

プロジェクト、GitHub repository、CLIをidealisへ変更した。registryへの初回publishは行っていない。

| 対象       | 旧名                                   | 新名                                    |
| ---------- | -------------------------------------- | --------------------------------------- |
| GitHub     | `mizchi/modelt`                        | `mizchi/idealis`                        |
| npm        | `@mizchi/modelt`                       | `@mizchi/idealis`                       |
| CLI        | `modelt`                               | `idealis`                               |
| Rustホスト | `modelt-core` / `modelt_core`          | `idealis-core` / `idealis_core`         |
| Rust SMT   | `rsmt` → `idealis-smt` / `idealis_smt` | `rusmt`                                 |
| Go module  | `github.com/mizchi/modelt/adapters/go` | `github.com/mizchi/idealis/adapters/go` |
| Go package | `modelt`                               | `idealis`                               |

TSのimportは`@mizchi/idealis/portable`などへ変更する。RustはCargo.tomlの依存名とuseのmodule名を変更する。rtla、rtla-compiler、rtlcの名前は維持した。

```sh
git remote set-url origin https://github.com/mizchi/idealis.git
idealis model model.json
idealis check model.json history.json
```

保存済みのversion 1のformat IDは`modelt.portable-trace`、`modelt.finite-model`、`modelt.safety-program`などを維持する。Wasm ABI v1の`modelt_*` export名とSMTの`smt_*`も維持する。新しいWasmファイル名は`idealis-core.wasm` / `rusmt.wasm`で、保存済みモデルやトレースを同じloaderで扱える。

過去のplan、iteration、runとvendor/fixture原本は、その時点の名前とパスを残す。過去の速度測定を今回の名前変更で測り直した結果として扱わない。現在のビルドと利用方法はREADMEと各crateのガイドを使う。

`just package`は新名のnpm/Cargo/Wasm archiveを生成し、展開した別consumerからTS/Rustの利用とインストール済みCLIを検査する。Goの単独consumerは`just go-test`で検査する。CLIの適合、違反、不正な操作に対する終了コード0/1/2は維持する。
