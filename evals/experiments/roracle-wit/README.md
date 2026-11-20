# 型付きoracle転送の試作

roracleの同じOracleGroupへ、WIT recordとpayload、WIT buffer、native bufferから入力を渡す。Rustはpayloadを借用し、汎用`serde_json::Value`やRequestへの再deserializeを作らない。出力は既存と同じJSON全reportで、共通decoderを使う。公開crate・npm API・既定backendには接続していない。

[測定前の計画](../../plans/typed-oracle.md)と[評価結果](../../iterations/typed-oracle.md)を参照。今回のJS objectからのpackingを含む比較では、JSON経路の方が速かった。WITの採用や他crateへの展開は保留する。

## ビルドと検査

Node 24+、pnpm、Rust、wasm32-unknown-unknown、wasm-tools 1.245.1を使う。jco-transpile 0.24.0は独立したtoolchain packageへ固定し、rootの依存には加えない。wit-bindgen 0.63.0の生成型をRustとTSで使う。

```sh
just oracle-typed-test
just oracle-typed-browser
```

`build.ts`はhost importのないcomponentを作り、jcoでJSとcore Wasmへ変換する。生成宣言はcheck-inし、buildは差分を拒否する。更新時だけ`node --experimental-strip-types evals/experiments/roracle-wit/build.ts`を使う。`.node`、生成JSとWasmはbuild成果物として扱う。実ブラウザの検査は任意のrecipeへ分けた。

共有契約は64組の観測、Unicodeと特殊key、200個の決定的な数値候補、depth境界と16 MiBの拒否をJSON/Wasmへ照合する。raw bufferの切断、余剰byte、不正tagも拒否する。unknown/invalid/errorをmatchedへ置き換えない。

## 転送契約

WITの原本は[`oracle.wit`](rust/wit/oracle.wit)。version・ID・case・観測variantとpayloadをbatchへまとめる。TS facadeの入口は既存のOracleRequestで、getter・symbol・疎なarray・非列挙field・非有限数・負のzero・不正Unicode・循環・未知fieldを拒否する。生成bindingへ直接不正なJS objectを渡す場合の検査は、このfacadeと同じ契約ではない。Proxyの隔離は提供しない。

payloadは値を連続して並べたforest。各nodeはlittle endianのu32全長とu8 tagから始まる。全長は5 byteのheaderも含む。

|       tag | 内容                                                   |
| --------: | ------------------------------------------------------ |
| 0 / 1 / 2 | null / false / true。追加payloadなし                   |
|         3 | JSON数値のUTF-8表記。Numberで検査・比較・serializeする |
|         4 | UTF-8 string本体。JSONのquoteなし                      |
|         5 | u32要素数と、長さ付き子nodeの列                        |
|         6 | u32 field数と、u32 key byte数・UTF-8 key・子nodeの列   |

objectのkeyはJSのUTF-16辞書順に揃え、Rustも順序と重複を検査する。key順に依存しない等値を保つための表現で、JSONのkey順を新たに保証するものではない。rootはpayload中のnode開始offsetを指す。Rustは全長・型・UTF-8・数値・子の範囲を検査してから借用する。子が必ず前進する形式なので、payload内部に循環参照は作れない。

Rustのcase/implementation上限は4096/256。canonical JSON sizeは16 MiB、入れ子は元のparserの制限、展開後ノード予算は1000000を使う。binary自体は64 MiBまで。共有したrootも展開後の予算へ数える。raw JSONの重複fieldとlegacy sequenceは元のJSON ABIの範囲に残す。

buffer形式はu32 `0x42524f49`とtransport version 1から始め、batch version、group、reference、implementation数と文字列、case数と各case、payload長とpayloadを並べる。文字列はu32 UTF-8 byte数を前置する。観測tagはvalue=0、allowed=1、unknown=2、invalid=3、error=4。nativeは同期呼出中だけ独立したbufferを借用する。ホストadapterは共有bufferを拒否する。

## 計測と再生

```sh
just oracle-typed-test
# build/testを終えてから単独で測る
just oracle-typed-eval .artifacts/my-typed-run
just oracle-typed-replay .artifacts/my-typed-run .artifacts/my-typed-replay
just oracle-typed-diagnose .artifacts/my-typed-run .artifacts/my-typed-phases
```

全経路はpacking、binding、Rust検査・照合・全report、decoderを含む。snapshot生成とSUTは含まない。診断区間のnative/WIT呼出はpacking済み入力を使うため、全経路の速度とは区別する。中央値の和を厳密な時間内訳にしない。

測定runへJS・source・生成binding・binary・入力・全reference・raw sample・hashを保存する。再生は同じplatform・arch・Node版で行い、新しいdirectoryへ出力する。hashは改変検知であり、外部archiveの真正性の証明ではない。
