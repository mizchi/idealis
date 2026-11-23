# ralloy

[対応範囲・非対応のチェックリスト](SUPPORT.md)を参照。

静的なAlloy関係モデルを、有限スコープ内で検査する実験用Rustライブラリ。Node、JVM、Z3、idealisの他crateへ依存せず、同じ実装をnativeとホストimport不要のWasmへビルドする。Alloy AnalyzerのSAT変換やAlloy全体を移植したものではない。

## Rustから使う

```rust
use ralloy::{Model, Status};

fn main() -> Result<(), ralloy::Error> {
    let model = Model::parse(
        "sig Node { edge: set Node } fact { no iden & edge } \
         assert Acyclic { no iden & ^edge } \
         check Acyclic for 2 but exactly 2 Node, 0 Int",
    )?;
    let result = model.analyze(0, 2_000_000)?;
    assert_eq!(result.status, Status::Counterexample);
    println!("{:?}", result.instance);
    Ok(())
}
```

`Model::parse`は全宣言とcommandを検査する。`commands()`はkindと解決済みscopeを返し、`analyze(index, max_work)`で1つを実行する。`Analysis`はcommand、status、instanceとwork/candidates/factInstancesを持ち、serdeでJSONへ変換できる。

## 実装のスナップショットを検査する

`Model::check_instance(command, &instance, max_work)`は、渡したsig/fieldを固定して評価する。atom名には実装のIDを使い、全sigと全fieldを明示する。別の充足する構造を探索し直さない。

結果は`InstanceCheck`で、matched、mismatch、invalid、unknownを返す。宣言とfactを満たしcommandの式が真ならmatched、式が偽ならmismatch。check commandもassertの真偽を返すため、反例はmismatchになる。scope・宣言multiplicity・factの違反はinvalidで、reasonはscope/declaration/facts。性質違反はproperty、予算切れはwork。matchedのreasonはnull。未知の宣言・atom、重複atom/tuple、fieldの欠落、builtin Intとのatom名の衝突はprotocol error。

これは1つの観測構造への適合性検査。未観測の入力や時間的な実装の振る舞いを保証しない。workは構造の検査と式の評価を数え、parserとJSON復号の費用を含めない。

## 対応するサブセット

- sigとone/lone/some、`abstract`、`extends`、親が1つの`in`、一括宣言、signature fact。名前が一意なfield。fieldの既定multiplicityはone。`A -> B`・`A -> B -> C`と`A m -> n B`で4項までのfieldを書ける。
- fact、引数付きpred/fun（引数は既定の`one`）、名前付きassert、run/check、全体scopeと型ごとの上限・exactly。呼び出しは`P[a, b]`・`a.P[b]`・`p.f`、それ以外の`r[x]`はbox join。
- 関係の和、差、交差、積、join、転置、推移閉包、反射推移閉包、domain/range制限。
- 等価・包含、論理結合、単一atomを束縛するall/no/some/one/loneとdisj。
- none、univ、iden、Int、String。noneは単項の空集合で、二項の空関係はnone -> noneと書く。既定4-bitのInt atomはuniv/idenへ含める。0 Intも指定できる。
- 整数リテラル、`#e`、`plus`などの算術と比較。Int型の関係は値の和として整数になり、値はbit幅の2の補数で桁あふれする（Alloy 6.2.0の既定）。

signatureは最大16個、fieldは最大8個、signatureのscopeは最大8、Intは0-5 bit、全体は64 atomまで。2列以上のfieldは、ownerのatomごとのtuple空間が64まで。sourceは1 MiB、10万token、tokenは256 bytes、式の再帰は48段、同一式の連続演算は512 tokenまで。超える入力はresource errorにする。

sigの継承・subset、複数sigの一括宣言、signature fact、module読込、引数付きpred/function、setを束縛する高階量化、整数演算・cardinality、3項以上の関係、時間・varは未対応として拒否する。Alloyの型検査全体や全構文の互換性は示さない。

## 結果と探索

| status            | 意味                                                            |
| ----------------- | --------------------------------------------------------------- |
| instance          | runの式とfactを満たす構造が見つかった                           |
| counterexample    | factを満たしassertを破る構造が見つかった                        |
| no-counterexample | factを満たす構造があり、指定scope内を列挙して反例がなかった     |
| no-instance       | runの条件、またはcheckのfactを満たす構造が指定scope内になかった |
| unknown           | work予算で探索を打ち切った。instanceは返さない                  |

no-counterexampleはスコープ外の証明ではない。no-instanceもスコープ外の不整合を意味しない。未対応、構文・型・scope・protocol違反は`Error`として返し、検査成功へ変換しない。

関係をbit matrixとして評価し、signatureの個数とfieldを列挙する。atom名で区別できないモデルでは、signatureごとに標準のatom名を使う。入れ替えられるatomの重複した枝は、lex-leaderで探索の途中で刈る。`div`・`rem`を含まないモデルで、fieldのslotの幅が24 bitを超えるcommandは、関係・sigの階層・整数（bitベクトル）をAIGとTseitin変換でCNFにし、ralloyの中のCDCLで解く。workは列挙と式の評価を数え、parserの費用やwall clockの上限ではない。

## nativeとWasm

`ralloy` binaryはJSONLをstdin/stdoutで扱う。

```json
{
  "version": 1,
  "request": {
    "kind": "analyze",
    "source": "sig A {} run { some A } for 1 but 0 Int",
    "command": 0,
    "maxWork": 100000
  }
}
```

固定instanceのJSONL requestはkindを`check-instance`とし、同じsource/command/maxWorkに`instance: {signatures, fields}`を加える。応答は`kind: instance-check`とInstanceCheck。

成功は`kind: analysis`とAnalysis、拒否は`kind: error`とError。Wasm ABI v1は`ralloy_abi_version`、`ralloy_input_resize`、`ralloy_dispatch`、`ralloy_output_ptr`、`ralloy_output_len`で、同じJSONを扱う。

## Alloyとの差分テスト

`tests/alloy/<group>/*.als`のモデルを公式Alloy 6.2.0（SAT4J）で1回実行し、commandごとに解があるか（runならinstance、checkならcounterexample）を[roracle](../roracle)の`CheckReport`として`tests/alloy/<group>.json`へ保存する。`tests/alloy_oracle.rs`は現在のモデルを登録して保存した答えを`replay`し、ralloyだけを実行する。予算切れ、解析エラー、自分の`check_instance`で再現できないinstanceは一致にしない。モデルを編集して記録し直していなければ、入力の不一致で失敗する。

記録にはJavaとAlloyのjarが必要で、再生には不要。instanceがAlloyでも成り立つかは検査していない。

```sh
just ralloy-oracle-record core   # Java/Alloyで記録し直す
just ralloy-oracle-bench         # 各モデルの全commandの解析時間
```

```sh
just ralloy-build
just ralloy-test
just ralloy-example
```

[Alloy言語仕様](https://alloytools.org/spec.html)とAlloy 6.2.0のparser/ScopeComputerを参照した。出典はNOTICEへ記録する。registryへは未公開。
