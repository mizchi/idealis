# RustとWasmでPBT・差分テストを駆動する

`idealis-connect`は生成・縮小・再生の状態をRustで管理し、ホストへcaseの評価を要求する。TSでは`@mizchi/idealis/testing`から使う。SUTの初期化・操作・障害注入・観測は各言語のDriverが担当する。Node、テストランナー、形式仕様のbackendはコアの依存に含めない。

## TSから使う

次は、入力列に`bad`が含まれる既知の性質違反を探す例。`just package`で作ったarchiveを導入して実行する。

```ts
import { readFile } from "node:fs/promises";
import { createTestingWasm } from "@mizchi/idealis/testing";

const runtime = await createTestingWasm(
  await readFile(new URL(import.meta.resolve("@mizchi/idealis/testing.wasm"))),
);
const result = await runtime.property(
  {
    generator: {
      id: "example",
      version: "1",
      initial: ["s"],
      states: {
        s: [
          { action: "good", target: "s" },
          { action: "bad", target: "s" },
        ],
      },
      minLength: 0,
      maxLength: 8,
    },
    seed: 0x36a1,
    cases: 32,
    maxAttempts: 64,
    maxShrinkChecks: 64,
    maxWork: 10_000,
  },
  (testCase) =>
    testCase.actions.includes("bad")
      ? { status: "mismatch", signature: "contains-bad" }
      : { status: "matched" },
);
console.log(result.status, result.counterexample?.actions);
```

結果はfailed、縮小したactionsは`["bad"]`になる。Vitestやnode:testではこの結果をassertする。ブラウザでは同梱Wasmを静的ファイルとして配り、fetchしたUint8Arrayを`createTestingWasm`へ渡す。ホストimportとcross-origin isolationは不要。

実装との照合では、callbackから既存の`checkConformance(model, createDriver(), actions)`を呼ぶ。mismatchには原因を表すsignatureを付け、matched/invalidはそのまま返す。各caseで新しいDriverを作り、縮小候補でも初期化から実行する。[状態付きカウンタの接続例](../evals/experiments/testing-core/driver.ts)と[生成器](../evals/experiments/testing-core/fixture.ts)を参照。

## actorの操作順を制御する

`runtime.schedule(input)`はactorごとの有限操作列を、順序を保ったinterleavingの生成器へ変換する。生成するactionは`{ actor, step, operation }`で、stepは0始まり。たとえば次は2 actorのread/writeを組み合わせる。

```ts
const generator = await runtime.schedule({
  id: "concurrent-increments",
  version: "1",
  actors: [
    { id: "left", operations: ["read", "write"] },
    { id: "right", operations: ["read", "write"] },
  ],
});
console.log(generator.maxLength); // 4
```

このgeneratorを`property`や`shrink`へ渡す。評価callbackはcaseごとにSUTを初期化し、actionに対応するactorを1操作ずつ進める。TSのasync generatorとRustの実threadを使う[評価driver](../evals/experiments/controlled-schedules/driver.ts)と[Rustのgate](../evals/experiments/controlled-schedules/rust/src/lib.rs)を参照。コアがthreadやawaitを自動計装するわけではない。

生成器はactorごとのprogram counterの直積。各actorの順序を保つprefixを受理するため、readを消して同actorのwriteだけを残す縮小候補は実行しない。PBTではprefixの期待値も仕様側で定義する。今回のカウンタは完了したwrite数を期待値とし、read途中のprefixも検査できる。

上限は1-8 actor、合計128操作、1024状態で、actor IDは一意にする。空の操作列は許すが、並行処理を検査した意味にはならない。生成器の構築はSession.maxWorkに含めず、構造の上限で制限する。保存caseはactor/step/operationをそのまま再生する。操作列やdriverの意味を変える場合はversionとmanifestも更新する。

構築結果はimmutableなスナップショット。構築と比較は進行中のproperty sessionからも呼べる。PBTのpassedは生成したcaseに対する結果で、全interleavingの検査済みを意味しない。ブロッキング、依存関係、weak memoryを自動でモデル化する機能は含まない。[0038の評価](../evals/iterations/0038-controlled-schedules.md)を参照。

## 結果と縮小

| status       | 意味                                          |
| ------------ | --------------------------------------------- |
| passed       | 要求した数の有効な正常caseを実行した          |
| failed       | 同じsignatureの失敗を最後に再現した           |
| exhausted    | attemptsを使い切り、有効case数が足りない      |
| inconclusive | coreのworkやホストの評価予算が不足した        |
| error        | 実行error、または確認時に失敗が再現しなかった |
| not-failing  | 明示的に縮小した初期caseが失敗していなかった  |

passedは全入力や全実行の証明ではない。縮小のminimalは、有効な単一削除で同じsignatureを保持できない意味。unknownの候補や予算切れがある場合はboundedになる。scalar値の縮小と任意の高階Strategyは未対応。

workは生成と有限状態機械による候補検査を数える。SUTの実行時間やJSON処理を含めない。cases/attempts/shrink checksも明示してホスト呼出回数を制限する。I/Oの中断はホストで管理する。callbackがthrowすると同じ例外を伝播し、内部sessionをcancelする。

## ケースの保存と再生

Caseにはgenerator ID/version、乱数アルゴリズム、seed/index、具体的な全actionsを保存する。再生は保存した列を使い、seedで再生成しない。

```ts
if (result.counterexample && result.signature) {
  const replay = await runtime.replay(
    result.counterexample,
    evaluate,
    result.signature,
  );
}
```

ここでevaluateはpropertyに渡した評価callback。signatureを指定して再現しなければerrorになる。正常版への対照再生はsignatureを指定せず、passedを要求する。モデル・実装・生成器のsource versionはcaseと一緒にmanifestへ保存する。

`runtime.shrink(generator, testCase, evaluate, { maxChecks, maxWork })`で保存caseを単独で縮小できる。caseのgenerator ID/versionと、渡した生成器が一致する必要がある。

## 複数実装の比較

`runtime.compare(oracle, targets)`へreferenceの許可値集合と各実装の観測を渡す。対象を多数決でreferenceにしない。invalid/unknown/errorや空の許可集合をmatchedにしない。対象ごとのoutcomeも残すため、全体のmismatchだけを見て他の対象も評価完了したとは扱わない。

JSONでは配列順を保持し、objectのfield順を無視する。値はplain JSONとsafe integerに限定する。集合、map、任意精度整数等は既存のPortableValueへ正規化してから比較する。

## nativeの接続と並列実行

Rustは`Session::property/shrink/replay`を作り、Evaluateをホストで実行して`resume`へ返す。[単独Rust consumer](../crates/idealis-connect/examples/counter.rs)を参照。Rustの `idealis-cli` は同じJSONL protocolを提供する。TSの別transportは`createTestingRuntime(dispatch)`へ接続できる。[native接続例](../evals/experiments/testing-core/clients.ts)を参照。

1 runtimeは1つのproperty/shrink/replay sessionを所有する。並列workerには別runtimeを作る。比較は進行中のsessionと独立して呼べる。既存の`@mizchi/idealis/property`はfast-checkを使う経路として残す。Strategyやseed/pathは新APIと互換ではない。

```sh
just testing-core-test
just testing-test
just testing-browser
just testing-example
just package
```

基礎契約の採用評価は[0036](../evals/iterations/0036-testing-core.md)、明示した並行スケジュールは[0038](../evals/iterations/0038-controlled-schedules.md)。開発用の[protocol fuzzing](../evals/experiments/structured-fuzz/README.md)は、不正要求後のsession保持や保存caseのnative/Wasm再生を検査する。fuzzerが合成したfeedbackの整合性を扱い、実SUTの全動作を検査済みとはしない。
