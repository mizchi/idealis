# 0077への追記: 1組の再測定とcache benchの組の完成

[0077のsolver比較](../runs/0077-smt-solver-comparison/result.json)は`hold`になった。主指標は条件を大きく満たした（`context-assert-256` 0.592/0.629、`context-constant-256` 0.451/0.488）。一方、次の7ケースが片方の組で閾値を超えた。

- 開発組: `parse-solve-32` 1.222、`prepared-32` 1.378、`parse-solve-128` 1.212。candidateのrunだけ中央値の95%区間が広い（それぞれ幅21.15、3.83、25.37 µs。ほかの3 runは0.24〜5.26 µs）。`prepared-32`は`Problem::solve`の経路で、0077は変更していない。確認組では0.925、0.968、0.916。
- 確認組: `script-prepared-scopes-32` 1.110、`script-parse-ite-64` 1.134、`script-prepared-ite-64` 1.108。開発組では1.010、0.926、1.013。`script-prepared-*`は`Script::solve`の経路で、変更していない。
- `parse-solve-4`は0.984/1.007で、条件の1.0以下を0.7%超えた。

測定中の負荷平均は2.5〜4.8で、WindowServerとChromeが動いていた。どの組でも同じ向きに超えたケースはない。0077のplanに従い、1組だけ再測定する。

cache benchは、candidateのbench binaryのコピーに失敗した（grepでパスを取れなかった）。baselineの2 run（`0077-smt-cache-baseline-*`）だけが残り、組になっていない。この2 runは不完全な記録として残し、比較には使わない。

## 固定する条件

- bench binaryは0077で保存したもの（`.artifacts/smt-lean/baseline/`、`.artifacts/smt-lean/candidate/solver-bench`）を、provenanceのhashで確認して使う。cache benchのcandidateは、aa48c557eから同じ手順でビルドし、パスを直接指定してコピーする。
- 条件・ケース・閾値・順序は0077と同じ。solverとcacheの両benchで4 runずつ、`0077a-*`へ保存する。
- 判断はこの8 runだけで行う。0077のrunと組み合わせて、都合のよい組を選ばない。

超えたケースは後退として記録し、さらに再測定しない。
