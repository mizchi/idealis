# object入力の費用とWITを使う場合の設計

遅さの主因は、JS objectを再帰的に列挙してRustの`serde_json::Value`へコピーする処理と、その前に行うJSの入力検査だった。照合自体の費用は小さい。JSON文字列を渡す経路では、JS engineのstringifyとRustのparserがまとめて処理する。object経路では各fieldや配列要素にNode-APIを呼び、文字列とmapを構築する。

[計画](../plans/native-object-profile.md)に従い、`ce30a09b2`のJSとnative binaryを凍結した。Apple M5、Darwin arm64、Node v24.22.2。profileと通常時間は別に測った。検査を削る二つの候補も保存したが、事前の採用目安に届かなかったため元の検査を維持した。native objectの既定化も行わない。

## 区間を分けた測定

固定6入力、30 samples、100 warmupと前回のiterationsを使った。各呼出の全応答を計時外で比較する。下表は中央値、単位はµs。区間は独立した呼出なので、中央値の和を全requestの厳密な内訳にはしない。

| 入力          | JS入力検査 | object native呼出 | JSON native呼出 | object全request | JSON全request |
| ------------- | ---------: | ----------------: | --------------: | --------------: | ------------: |
| 単独          |      2.502 |             5.572 |           1.490 |           9.892 |         3.199 |
| 32 cases      |     84.525 |           163.803 |          38.443 |         283.515 |        78.156 |
| 入れ子256要素 |    306.604 |           469.446 |         129.213 |         817.642 |       197.825 |

native呼出の区間はRustの処理とJSON出力の受け取りまで含む。JSON区間だけは事前に文字列化した入力を使い、全requestではstringifyも計時する。object native呼出は検査を外してもJSON全requestより遅い。JS検査だけを取り除く方法では、この差を解消できない。

## call stackで確認した処理

入れ子入力を10000回実行した。Node inspectorは100 µs間隔、samplyは1000 Hz。Nodeの80217 samplesでは`dispatchObject`が41542、入力検査の`visit`が25866 self samplesだった。残りには共通decoderと、計時外の全report比較も含む。

nativeの全threadには待機中のsampleもあるため、全体を分母にしない。native callbackが現れる5284 samplesに絞ると、次の関数が現れた。inclusiveな件数には重なりがあり、合計しない。

| 処理                          | samples | callbackを含むsampleに対する比 |
| ----------------------------- | ------: | -----------------------------: |
| Node-APIからValueへの再帰変換 |    3065 |                          58.0% |
| Rustのdispatch_value          |    1646 |                          31.2% |
| reportを中間Valueへserialize  |     454 |                           8.6% |
| OracleGroupの照合             |      38 |                           0.7% |

profileでは`Object::keys`、property取得、UTF-8文字列化、BTreeMap構築も確認できる。固定したnapi 3.16.2のserde変換実装でも、objectのkey列挙と各値の再帰変換を行っている。[napi-rsの型変換](https://napi.rs/docs/concepts/type-conversions)は、JS値とRust所有の値の変換を区別している。生成されたTS型は、この変換を省く仕組みではない。

Rust側も、深さ確認、sizeを数えるserialize、ValueからRequestへのdeserialize、report用Valueの構築、JSON出力を行う。入力と出力の中間Valueを減らす余地はあるが、sizeや深さの検査を削除する変更とは分ける必要がある。

## 検査処理の改善候補

getter拒否、symbol・非列挙field、疎なarray、循環、深さ・展開後ノード数、次の呼出での再検査を維持した。元の6入力に、同じJSON値で共有参照のない6入力を加えた。native binaryは同じhashを要求し、JSの変更だけを前後/後前の順で測る。JSON全requestも対照にする。

| run           | 候補                                                                               | object全requestの速度比の幾何平均 | 判定             |
| ------------- | ---------------------------------------------------------------------------------- | --------------------------------: | ---------------- |
| identity      | 変更前同士                                                                         |                           0.99896 | 改善と判定しない |
| comparison-01 | descriptorの中間object・entries配列を削減し、prototypeの列挙を呼出ごとに一度へ集約 |                           1.12998 | 1.15未満         |
| comparison-02 | 上記にkey文字列の検査のインライン化を追加                                          |                           1.14672 | 1.15未満         |

両候補に10%以上の対照の後退はなかった。共有参照がない入力でも同じ傾向だった。目安を下げたり同じ候補を繰り返し測って閾値を越える結果を選んだりせず、候補はarchiveへ保存した。現在の`canonicalObject`は元の検査を別moduleへ移しただけで、候補を採用したものではない。

二つの比較runは、それぞれ60個の全report、1440 samples、凍結source/binaryのhashと統計を再生した。空の比較がNaNを返すRedを追加し、空suiteや不足sampleを拒否するよう測定器を修正した。最初の区間測定には測定器のTS型エラーが残っていたため、そのrunも保存し、型修正と契約検査が成功した`phases-before-02`を本文の値に使った。

保存reportの改変と0時間sampleの対照は、どちらもreplayで拒否した。基本のNode/Vitest各79件、Wasm各23件、native契約各13件とworkspace各8件が成功した。既存TS mutation対象は104/109の95.4%で、今回のnative変換の変異検出率ではない。既存評価の保存反例も再生した。crateのsourceは変更しておらず、Jevの68比較は既存の有効な記録をoffline/staged gateで再確認する。live同期はAPIキー未設定として警告し、スキップした。

## WITで除去するものと残るもの

WITはJSON形式を別名で渡すためではなく、型付きの引数とABIを定める用途に向く。Canonical ABIはrecordなどをflatな値かlinear memory上の表現へ変換する。文字列やlistにも変換が必要で、JS objectをそのまま共有できるわけではない。[Canonical ABI](https://component-model.bytecodealliance.org/advanced/canonical-abi.html)

このrepositoryで試すなら、次の境界を一緒に変える。

| 対象               | 次の試作で比較する表現                                                                 |
| ------------------ | -------------------------------------------------------------------------------------- |
| requestの外枠      | version、case、実装ID、観測kindを型付きrecord・variant・listへする                     |
| 任意の観測値       | keyと参照IDを持つnode表、または型付きbufferへ一度だけ符号化する                        |
| Rustコア           | Valueを受けてRequestを作り直す入口に加え、検査済みの型付きbatchを受ける入口を設ける    |
| 大きなモデル・履歴 | resource/handleで保持し、操作と必要な差分を渡す。保持・解放と予算を契約に含める        |
| 応答               | 同じ全reportの構築・受け取りまで比較する。statusだけの経路を全reportの高速化へ数えない |

WITがあっても現在の`dispatch_value`へ再度Valueを構築して渡すなら、その費用は残る。JSからrecordを構築する費用、文字列の符号化、payloadのpacking、出力のlifting、size・Unicode・未知field等の拒否条件も計測対象にする。Nodeのnative経路は、WITの契約に対応する型付きadapterやbufferの入口が別途必要になる。WITを指定するだけでnapi-rsの現在の関数が高速になるわけではない。

node表を使う場合も、objectのkey順に依存しない照合、循環の拒否、深さと展開後ノード数の予算を保つ。既存のValueへ変換する費用をpackingの計時外へ移したり、成功時のreportを省略したりして改善とは数えない。

ブラウザにはjcoでcomponentをJS wrapperとcore Wasmへ変換する経路がある。[jcoのtranspile](https://bytecodealliance.github.io/jco/transpiling.html)を利用できる。まず小さい型付きbatchと大きいpayloadを分け、JSON、現在のobject、WIT、型付きbufferを同じ意味・全report・拒否入力で比較したい。今回はWIT bindingを実装しておらず、速度改善もまだ実測していない。

[実行手順](../roracle-native/README.md)と[保存証拠](../runs/native-object-profile/evidence.json)を参照。
