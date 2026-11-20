# Alloyモデルと実装の構造を照合する

`@mizchi/idealis/alloy`は、独立crate `ralloy`のWasmを使う。モデルから構造を探索する`analyze`と、実装の観測を固定する`checkInstance`を提供する。Node、JVM、Z3への依存はなく、同じAPIをブラウザ・Vitest・node:testで使える。配布物は未公開のalpha版。[対応構文](../crates/ralloy/README.md)を確認する。

## node:testから使う

`just package`で作ったnpm archiveを導入し、同梱したWasmを読む。

```ts
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { createAlloyWasm } from "@mizchi/idealis/alloy";

test("observed graph conforms", async () => {
  const runtime = await createAlloyWasm(
    await readFile(new URL(import.meta.resolve("@mizchi/idealis/ralloy.wasm"))),
  );
  const input = {
    source: `sig N { edge: set N }
      fact { no iden & edge }
      assert Safe { no iden & ^edge }
      check Safe for 2 but exactly 2 N, 0 Int`,
    command: 0,
    maxWork: 100_000,
  };
  const observed = {
    signatures: { N: ["server-a", "server-b"] },
    fields: { edge: [["server-a", "server-b"]] as const },
  };
  assert.equal(runtime.checkInstance(input, observed).status, "matched");
});
```

Vitestでは`test`とassertionをVitestから読み込める。初期化したruntimeは同じテストプロセス内で再利用できる。実装からの観測を期待値へ変換せず、モデルを独立した基準にする。

ブラウザでは同梱Wasmを静的ファイルとして配置し、`createAlloyWasm(new Uint8Array(await response.arrayBuffer()))`へ渡す。コンパイル済み`WebAssembly.Module`も受け取れる。ホストimportとcross-origin isolationは不要。

## 観測する値と判定

全sigを`signatures`へ、全fieldを`fields`へ明示する。fieldが空なら空配列を渡す。欠落したfieldは空関係へ補完しない。atom名は実装のIDでよく、sig間とbuiltin Intのatom名に重複してはならない。fieldのtupleは宣言されたowner/rangeに属するatomを使う。

| checkInstanceのstatus | 意味                                                      |
| --------------------- | --------------------------------------------------------- |
| matched               | 指定した構造がscope・宣言・fact・commandの式を満たす      |
| mismatch              | 宣言とfactを満たす構造で、propertyが偽                    |
| invalid               | scope、宣言のmultiplicity/range、factのいずれかに違反する |
| unknown               | work予算が不足し、適合性を判定できない                    |

reasonはscope、declaration、facts、property、work、またはmatchedのnull。未知の宣言・atom、重複atom/tuple、field欠落、壊れたschemaは`AlloyError`をthrowし、kindにprotocol等の分類を持つ。判定結果はimmutableなスナップショットとして返す。

自己ループをfactで除外した上のモデルでは、自己ループの観測はinvalid/factsになる。2-cycleはfactを満たしてassertを破るためmismatch/propertyになる。テストで適合を要求する場合はmatchedをassertし、invalidやunknownを通過させない。

## 探索と照合の違い

`analyze(input)`は有限scope内の構造を列挙し、instance、counterexample、no-instance、no-counterexample、unknownを返す。`checkInstance(input, observed)`は渡した構造だけを評価する。check commandに対してもassertの正方向の真偽を返すため、探索で得たcounterexampleを照合するとmismatchになる。

1つの観測構造の一致は、実装全体や時間的な振る舞いの証明ではない。workは構造の検査と式の評価を数え、JSON復号とparserの費用を含めない。TSのcommand番号は0から始まり、maxWorkは非負のsafe integerを使う。汎用backend契約とCLIへの統合はまだ行っていない。

## 検証を実行する

```sh
just ralloy-test
just alloy-test
just alloy-browser
just package
```

`alloy-test`は同じ契約をnode:testとVitestで実行し、3ノードの全512グラフをTSの独立した循環判定、native、Wasmで照合する。公式Alloy Analyzerへの構造固定比較は[0035の計画](../evals/plans/0035-alloy-instance.md)と[評価ハーネス](../evals/experiments/alloy-instance/README.md)を参照。
