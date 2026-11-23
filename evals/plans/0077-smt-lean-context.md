# 0077: 増分contextの固定費の削減

0074の考察で挙げたB（assertionをGitのcommitのような連鎖にする）は、実装前の探索profileで効果がほぼないと分かった。保存済みの0074b bench binaryをsamplyで取ったprofileでは、`context-scopes-32`のpopはほぼ0%で、時間の61%は最初の`Solver::parse`だった。profileで見えた費用は次のとおり。この数値は探索用で、採否には使わない。

| ケース                 | 費用                                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------------ |
| `context-assert-256`   | `parser::render`による文字列の生成23%（長さの計算とhash用）、SHA-256 4%、`Vec<Arc<Formula>>`のclone 4% |
| `context-constant-256` | 全assertionの再コンパイル（`compile_all`）18%、render 19%                                              |
| `parse-solve-4`        | 解析84%                                                                                                |

mizchiの判断で、Bを次の3つ（B'）に置き換える。応答・work/stats・cacheのkeyは変えない。

1. 長さの計算とhashで文字列を作らない。長さは構文木から数え、hashはrenderしたbyteを直接hasherへ流す。Sessionのsource hashは、cacheが初めて使う時まで計算しない。
2. `Problem`はcheckで初めて必要になった時に作る。assertごとにformulaのVecをcloneしない。
3. 新しい整数定数を束縛した時は、その変数名を含むassertionだけを再コンパイルする。名前の検査は構文木の全atomへの過大近似とする。

## 固定する入力と版

- baselineは3dd74cb5b（0076の記録まで）のライブラリ。candidateはこのplanを含むbranchの実装commit。
- `benches/solver.rs`の18ケースと`benches/cache.rs`の15ケースを、両版で同じsourceからビルドする。`CARGO_PROFILE_BENCH_STRIP=none`で、保存したbinaryを全測定で再利用する。
- Criterionの条件は0074と同じ（20 samples、warmup 1秒、measurement 2秒、median、95%区間）。
- 順序はbaseline→candidate（開発）、candidate→baseline（確認）。solverとcacheの両benchで行い、8 runを`0077-*`へ保存する。

## 採用条件（結果を見る前に固定）

両組で次を満たすこと。

- 主指標: `context-assert-256`と`context-constant-256`はcandidate/baselineの比0.85以下。
- 単発の小さい解析: `parse-solve-4`は1.0以下。0074から続く数%の後退を戻す。
- 後退: solver benchの残る15ケースと、cache benchの15ケースは1.1以下。

超えたケースは後退として記録し、閾値を変えない。再測定は追記planで1組だけ宣言できる。

## 正しさ

- Rust: 全再解析の参照実装との差分テスト（`tests/incremental.rs`、400系列）を変えずに通す。定数の束縛が一部のassertionだけに影響する列、`let`で同名の束縛を含む列、引用符付きsymbol（`|x|`）を含む列を追加し、参照実装と照合する。
- 人工故障: 再コンパイルの対象から名前の一致した式を外す、遅延hashで最後のassertを落とす、長さの計算で引用符の2 byteを数えない、の3件を検出すること。
- 保存run: 0074のharnessで、開発seed 0x77a1・確認seed 0x77c3（未使用）を実行する。baseline（3dd74cb5bのnative/Wasm）との全応答（work/statsとerror）の一致、Z3との照合、cacheの照合を行い、両runを再生する。

Wasmのサイズを報告する。Jevの`support-sync`を実行する。
