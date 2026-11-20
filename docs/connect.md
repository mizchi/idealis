# idealis connect の API 案

モデルに **実装に与える操作** と **仕様と比較する観測** を定める。プロジェクトが満たす契約はテスト時の振る舞いにある。モデルの操作を実装に対して実行でき、読み戻した観測が、そのテストで指定したモデルの性質を満たすこと。接続には TS / Rust の標準テンプレート、独自テンプレート、手書きの変換を選べる。

## テンプレートを使わないテスト

例えば `TypedChoice` の `storedSelection` は「初期化後の値は 0、`Put(x)` の直後に読み戻す値は x」という性質。保存した ITF をテスト側で `Trace` に変換し、既存の `checkTrace` に渡せる。以下の `choiceTrace` はこのモデル専用にテスト側で用意した変換関数。

```ts
const trace = choiceTrace(dump);
const result = await checkTrace(trace, {
  reset() {
    store.clear();
    return store.read().toString();
  },
  step(decimal) {
    store.write(BigInt(decimal));
    return store.read().toString();
  },
});
assert.equal(result.status, "matched");
```

変換は、入力の各状態に対応する操作と観測を復号し、ダンプ自体が指定した性質を満たすことを確かめる。実装には操作だけを渡す。実装を動かした後の読み取り値を、モデルに記録された期待値と毎回比較する。空の入力、変換できない操作、性質に反する期待値はテストを失敗させる。これを生成方法に共通の受け入れ条件とする。

```sh
just connect-conformance-test
```

このテストは TS の `Map` と Rust の独立プロセスに同じ 7 状態を再生する。コード生成の実行や生成済みファイルの用意は不要。両方に対して最初の `Put(1)` を取りこぼすと期待値 1・実測値 0 で失敗することも確認する。保証範囲は実行したトレースと観測した性質に限る。[手書きの変換](../tests/connect/choice-trace.ts)・[適合テスト](../tests/connect/conformance.test.ts)・[保存した ITF](../fixtures/connect/typed-choice.itf.json)

このダンプは以下の条件で生成した。通常の CI は保存済みの入力を再生する。

```sh
pnpm dlx @informalsystems/quint@0.33.1 run fixtures/connect/TypedChoice.qnt \
  --backend typescript --seed 42 --max-samples 1 --max-steps 6 \
  --invariant storedSelection --out-itf .artifacts/typed-choice.itf.json --verbosity 0
```

## モデルに書く契約

[TypedChoice.qnt](../fixtures/connect/TypedChoice.qnt) では、操作は直和型、観測はレコード型になる。

```quint
type Command = Init | Put({ selected: int })
type Observation = { value: int }

var command: Command
var observation: Observation
```

各 action は実装に渡す `command'` と、その結果として仕様が要求する `observation'` を同じ遷移で記録する。例えば `Put({ selected: 1 })` の次状態には `{ value: 1 }` が入る。実装の更新処理と読み取り処理は Driver の作者が書く。

## 標準テンプレートを使う場合

```sh
just connect-bindings fixtures/connect/TypedChoice.qnt TypedChoice ts > bindings.ts
just connect-bindings fixtures/connect/TypedChoice.qnt TypedChoice rs > bindings.rs
just connect-bindings-test
```

生成器は Quint 0.33.1 の `typecheck` が返したモジュールと型宣言を読む。一度も選ばれなかった variant も契約に含まれる。型の取得元はモデルなので、入力として ITF ダンプを用意する必要はない。TS の `Command` は次の discriminated union になる。

```ts
type Command =
  | { readonly tag: "Init" }
  | { readonly tag: "Put"; readonly value: { readonly selected: bigint } };
```

標準テンプレートを選んだ場合は、生成された `Driver` の各ハンドラと観測を実装する。TS では `satisfies Driver` がハンドラの過不足と引数・観測の型を検査する。`selected` は `bigint` に推論される。

```ts
import type { Driver } from "./bindings.js";

const driver = {
  actions: {
    Init: () => store.reset(),
    Put: ({ selected }) => store.set(selected),
  },
  observe: () => ({ value: store.get() }),
} satisfies Driver;
```

Rust は生成された `Command` enum、`Observation` struct、必須メソッドを持つ `Driver` trait を使う。生成された `Command::apply` が variant に対応するメソッドを呼ぶ。エラー型はストア側で選べる。モデルの `int` を使う crate は `num-bigint = "0.4"` を依存に加える。

```rust
impl Driver for Store {
    type Error = StoreError;
    fn handle_init(&mut self) -> Result<(), Self::Error> { self.reset() }
    fn handle_put(&mut self, input: PutInput) -> Result<(), Self::Error> {
        self.set(input.selected)
    }
    fn observe(&self) -> Result<Observation, Self::Error> {
        Ok(Observation { value: self.get()? })
    }
}
```

モデルに `Clear` を足して再生成すると、既存の TS 実装は `actions.Clear` が欠け、Rust 実装は `handle_clear` が欠けてコンパイルに失敗する。運用ではモデルを変更したビルドで型を再生成し、実装と一緒にコンパイルする。上のテストは実際にモデルを変更して両コンパイラの診断を確認する。[TS の動く例](../fixtures/connect/consumer/store.ts)・[Rust の動く例](../fixtures/connect/consumer/store.rs)・[回帰テスト](../tests/connect/bindings.test.ts)

既定では `command` と `observation` の型注釈を読む。型名はモデル側で付けられる。引数なしの variant と、`int`・`bool`・`str` をフィールドに持つ閉じたレコードを扱う。未対応型では生成をエラーにする。TLA+ など別のフロントエンドを接続するときは、公開する操作・引数・観測型を同じ契約へ写す必要がある。

## プロジェクトのスタイルに合わせる

第三引数はテンプレート。組み込みの `ts` / `rs` のほか、プロジェクトに置いた ES モジュールのパスを渡せる。生成器は型検査したモデルをテンプレートの `render(contract)` に渡し、返った文字列をそのまま stdout に書く。生成先のファイルやフォーマッタは呼び出し側で選ぶ。[テンプレート入力の型](../scripts/connect-templates/contract.ts)・[TS 標準テンプレート](../scripts/connect-templates/typescript.ts)・[Rust 標準テンプレート](../scripts/connect-templates/rust.ts)

例えば、すでにあるストアをテストで接続したいなら、標準テンプレートでモデルの型を作り、プロジェクトのテンプレートで薄いアダプタを生成する。

```sh
just connect-bindings fixtures/connect/TypedChoice.qnt TypedChoice ts > bindings.ts
just connect-bindings fixtures/connect/TypedChoice.qnt TypedChoice fixtures/connect/templates/callbacks.mjs > callbacks.ts
```

```ts
import { bind } from "./callbacks";

const driver = bind({
  onInit: () => store.clear(),
  onPut: ({ selected }) => store.write(selected),
  read: () => ({ value: store.read() }),
});
```

この例で `onPut` や `read` という名前を決めるのはプロジェクトのテンプレート。既存のストアは自分のメソッド名や引数を保てる。生成するアダプタの戻り値に標準の `Driver` 型を指定し、各コールバックの型をそこから参照する。モデルに `Clear` を足すと、この書き方でもテスト側の `onClear` が必須になる。Rust でもテスト側のラッパーに生成された trait を実装できる。[テンプレートの全文](../fixtures/connect/templates/callbacks.mjs)・[独自アダプタを使うテスト用実装](../fixtures/connect/consumer/callbacks-consumer.ts)

テンプレートは `export function render(contract)` を提供する。非同期関数でもよい。入力の `version` は現在 1。`module` はモデルのモジュール名、`bindings` は入力に使う変数名、`variants` は各操作の名前と引数フィールド、`observation` は観測フィールド。フィールドにはモデルで宣言された名前と型が入る。既存モデルが `lastOp` と `currentView` という変数を使う場合は、末尾に `--command lastOp --observation currentView` を付ける。モデルの宣言順と名前はテンプレートに引き継がれる。

## 型付き ITF 再生の入口（案）

`replay(source, { model: typedChoice, open: () => createStoreDriver() })` を想定する。`typedChoice` は生成するデコーダと dispatcher の組。runner は state 0 も含め、次の順で検査する。

1. モデル契約に従って ITF の `command` と `observation` を検証・復号する。例えば `{"tag":"Put","value":{"selected":{"#bigint":"1"}}}` を TS の `{ tag: "Put", value: { selected: 1n } }` に変換する。未知のタグ、必須フィールドの欠落、型違いは位置付きの `invalid` にする。
2. トレースごとに新しい Driver を開き、生成された `apply(driver, command)` で実装を操作する。観測は同じ Driver の `observe()` で読み戻す。非同期操作は完了を待つ。
3. 仕様側の `observation` と実測値を照合し、不一致ならその state index、command、期待値、実測値を返す。終了時には Driver を閉じる。

Go も同じ契約から variant ごとのメソッドと `Observe` を持つ interface を生成する設計とし、`var _ bindings.Driver = (*Store)(nil)` で適合を検査する。言語ごとの Driver に ITF のタグやモデルの期待値を渡す必要はない。

## Quint MBT 形式の既存ダンプと接続する場合

Quint Connect の Rust 版は各状態から `Step` を作り、初期状態を含めて `Driver::step` の後に仕様状態と実装状態を比較する。既定の抽出元は `mbt::actionTaken` と `mbt::nondetPicks`。上の型付きモデルは ITF の状態変数に command を直接記録する。既存の MBT ダンプを同じ契約に接続する場合は、モデル作者が action 名と picks を Command の variant・引数に対応付ける。以下は [Choice.qnt](../fixtures/quint-generated/Choice.qnt) と [保存済み MBT ダンプ](../fixtures/connect/choice.itf.json) をこの経路で読み込む例。操作と観測を一回の呼び出しにまとめると、別プロセスの Driver でも一往復で済む。[Rust の runner](https://github.com/quint-co/quint-connect/blob/4f018f54fc7dd4cef341d10111427bab59d3b307/connect/src/runner/mod.rs)・[Step の抽出](https://github.com/quint-co/quint-connect/blob/4f018f54fc7dd4cef341d10111427bab59d3b307/connect/src/driver/step.rs)

| state index | Driver に渡す action | Driver に渡す picks（整数を十進表記） | モデル側に保持する value |
| ----------- | -------------------- | ------------------------------------- | ------------------------ |
| 0           | init                 | {}                                    | 0                        |
| 1           | step                 | { selected = 1 }                      | 1                        |
| 2           | step                 | { selected = 0 }                      | 0                        |
| 3           | step                 | { selected = 1 }                      | 1                        |

1. ITF の `vars` と各 state の変数を検査する。比較前に state メタデータと上記の MBT 変数を取り除く。picks の `Some(x)` は x、`None` はキーの欠落として渡す。選択値の 0、false、空文字は保持する。
2. `statePath` はモデル側の record のキー列。省略時は残った状態全体を比較する。存在しないパスや観測対象変数が一つもないトレースは `invalid`。Driver は選んだ型の値を返す。
3. 等値は ITF の値に対して決める。例えば `{"#bigint":"9007199254740993"}` は Rust/Go/JS すべてで精度を保つ。集合は要素順、map はエントリ順、record はキー順に依存しない。列と tuple は区別する。解釈できないタグや比較対象の `#unserializable` は理由付きで `invalid` にする。[ITF の仕様](https://apalache-mc.org/docs/adr/015adr-trace.html)

素の ITF は操作を記録していなくても読める。MBT 経路での再生には action と必要な選択が要る。state だけの ITF は閲覧・保存できるが、Quint MBT としての再生は `invalid`。rtlc の反例を接続するときは、生成側で選択値も記録する。状態差分から SUT に与える操作を推測しない。

## ストリームとダンプ

入力形式はどちらも **1 トレース = 1 個の ITF JSON 文書**。生成器は `vars` に続けて `states` 配列を開き、各 state を確定時に書き出せる。reader は JSON トークン境界をまたぐチャンクを受け、state が一つ揃ったら検査して Driver を呼ぶ。次の state は観測を回収してから処理する。ファイル全体の読み込みや行単位の JSON 分割を前提にしない。

そのまま保存したバイト列が再生用ダンプになる。ファイル入力にも同じ reader を使う。JSON のキー順は任意なので、`states` が `vars` に先行する入力は上限付きで保留し、上限を超えたら資源エラーにする。文書の終端と EOF まで確認して初めて `matched` を確定する。途中 EOF、末尾に別文書がある場合、空の states は `invalid`。入力元の I/O エラーや中断は実行エラーとして返す。

閲覧や保存だけなら Driver は不要。下の reader / writer を runner とモデル生成器でも共有する。SDK の event はメモリ上の API で、ファイルやパイプ上では標準の ITF を使う。

| 用途                   | Rust                            | TypeScript                                   | Go                                            |
| ---------------------- | ------------------------------- | -------------------------------------------- | --------------------------------------------- |
| 読みながら調べる       | `itf::read(reader)` の iterator | `for await (const event of readItf(source))` | `reader := itf.NewReader(r)`、`reader.Next()` |
| モデルの出力を保存する | `itf::write(writer, events)`    | `await writeItf(sink, events)`               | `itf.Write(ctx, w, events)`                   |

reader はトップレベルの各フィールド（`vars`、`#meta` など）、個々の state、文書全体の検査後の end を通知する。書き戻す際にはメタデータと値のタグを保持する。元のバイト列も保存したい場合は reader の入力を分岐する。Quint Connect 自体はファイルを順に開き、各ファイルから一つの Trace をデシリアライズする。state ごとの逐次読み取りは idealis 側で追加する機能となる。[上流の読み取り実装](https://github.com/quint-co/quint-connect/blob/4f018f54fc7dd4cef341d10111427bab59d3b307/connect/src/trace/iter.rs)

CLI を追加する場合の呼び方は以下を想定する。`producer` は一つの ITF を stdout に書き、`tee` が同じバイト列を保存する。検査レポートは ITF ダンプと別の出力になる。

```sh
producer | tee choice.itf.json | idealis connect replay --state-path value - -- ./store-driver
idealis connect replay --state-path value choice.itf.json -- ./store-driver
```

別プロセスへは index・action・picks を含む要求と、観測またはエラーを含む応答だけを JSON Lines で往復させる。これはホストと Driver の通信に閉じる。入力と期待値はホスト、実装の状態は Driver が所有する。非同期の TS、キャンセル可能な Go はホスト側の責任にする。Rust に置く値の検査・比較は native と Wasm から共有し、子プロセスは native CLI または Node ホストで起動する。

## 判定と受け入れ条件

- `matched` は選んだ投影でこのトレースの全状態が一致したことを表す。別の非決定選択まで含むモデル全体の適合性は、元のモデルを使う検査で扱う。
- `mismatch` は最初に異なった index、action、picks、投影パス、expected、actual を返す。確認済みの状態数は一致した先頭部分の長さ。fixture の更新を無視する実装なら index 1 の `step`、expected 1、actual 0、確認済み 1 になる。
- `invalid` は ITF または再生に必要なモデル情報の不備を表す。Driver の起動・観測・後始末の失敗は言語別のエラー経路で理由と位置を返す。いずれもテストを合格にしない。

設計時の動作確認では、この fixture の整数に絞った reader と、独立した Rust・TS・Go の Driver を使った。各 Driver は action と picks からストアを更新し、ストアの現在値を返した。ファイル入力と 3 バイトずつ書き込むパイプ入力の双方で、正常版は 4 状態一致し、更新を無視する版は index 1 で期待値 1・実測値 0 になった。終端を切った入力と空の states は全言語で `invalid` になった。この確認は API 案の試作に限る。

公開 SDK の最初の受け入れ条件は、上の fixture を Rust・TS・Go の正常版と更新を無視する版で再生し、ファイルと細かく分割した pipe の双方で同じ判定・位置・値を得ること。続いて初期状態の不一致、初期化時の picks、大整数、順序を入れ替えた集合・map、壊れた末尾、空の states、action 欠落、Driver 切断を共通ベクトルで検査する。

既存の TS / Go の有限モデル Driver は `reset()` が初期観測を返す。新しい入口で初期 action をそのまま渡し、既存 API からは必要な場合だけ adapter で接続する。既存の portable 値には tuple の区別がなく、idealis-connect の JSON 比較にも ITF の集合・map の等値はない。この二点を固定してから、通常のテストランナー向け SDK と CLI を公開する。[既存の契約](../packages/idealis/src/contracts.ts)・[既存の Rust 比較器](../crates/idealis-connect/src/compare.rs)

fixture の生成条件は Quint 0.33.1、TypeScript backend、seed 42。再生成時は新しいファイルへ出力する。時刻などのメタデータは生成ごとに変わる。

```sh
pnpm dlx @informalsystems/quint@0.33.1 run fixtures/quint-generated/Choice.qnt \
  --backend typescript --mbt --seed 42 --max-samples 1 --max-steps 3 \
  --out-itf .artifacts/choice-regenerated.itf.json --verbosity 0
```
