# 0055: oracleの二重deserializeとsnapshot配列を減らす

最終版は前回のe5b88f9fに対し、固定rtla応答の往復照合を449.16→377.36 µs、1.19倍に改善した。coreは33.29→29.74 µs、1.12倍。Rustの観測読み取りとJSのsnapshot生成を変え、同じ親runnerで変更を分けて比較した。これは収集済み応答の通信・照合費用で、モデル検査の探索速度を示さない。

## profileで選んだ箇所

[事前計画](../plans/0055-oracle-hotspots.md)をコミットし、前回のJS/native/Wasmを`.artifacts/0055-before`へ保存した。[初期Node CPU profile](../runs/0055-profile-before/summary.json)ではsnapshotのvisitがself約22.6%。serdeのContentDeserializerが約16.4%のinclusive sampleを持つ関数もあった。これは関数ごとの割合で、複数関数を足して占有率にはしない。

serde_derive 1.0.229の`de/enum_internally.rs`を読むと、internally tagged enumはTaggedContentVisitorで中間Contentへ受けてからvariantをdeserializeしていた。公開Observation型は保ち、JSON wire専用visitorでValueを直接読むようにした。kindが後ろにある入力、重複field、nullと余分なfieldも検査する。

JS側はobjectのentries→map→fromEntriesをkeyごとの構築へ変えた。配列は一括descriptor objectを作らず、own property名と各indexのdescriptorを検査する。毎回入力を読み、全reportを返す。

## setterを呼ばない構築の維持

初期候補は配列をpush、objectを通常の代入で構築していた。レビューで、prototypeにsetterがあると旧版が呼ばなかったsetterを呼ぶ差を見つけた。Nodeの独立したVM realmで再現するテストを追加し、[Red](../runs/0055-validation-final/setters-red.log)を確認してから修正した。

最終版では配列をArray.fromで構築し、継承propertyを持つobject keyはdefinePropertyを使う。`__proto__`を含めown data propertyとして構築し、snapshotの形状とprototypeを保つ。[Green](../runs/0055-validation-final/setters-green.log)ではobject/arrayの両方でsetterを呼ばない。

[初期候補の往復](../runs/0055-roundtrip/result.json)は1.23倍だったが、その値を採用結果には含めない。修正後に別runで測り直した。初期の[区間before](../runs/0055-phases-before/result.json)・[wire](../runs/0055-phases-wire/result.json)・[combined](../runs/0055-phases-combined/result.json)、[追加対照after](../runs/0055-phases-controls-after/result.json)、[候補profile](../runs/0055-profile-after/summary.json)も補助記録として残す。

## 最終版を同じ親runnerで比較する

[最終往復raw](../runs/0055-roundtrip-final/result.json)はbefore/wire/combined/combined/wire/before。wireはRustだけ、combinedは修正後のJSも含む。全stageで同じbeforeの親runnerを使い、各runにlegacy Node equalityを対照として置いた。Apple M5、Node v24.22.2、20 samples・75 warmup呼出・25 iterations、予定呼出上限10000。全stageで2306/2306呼出が一致した。

各runのmedianを取り、同じstageの2 runのmedianを表へ示す。速度比は記述値であり、有意差や他の環境への保証は主張しない。source/binary hashと各runの値は[最終集計](../runs/0055-summary-final/result.json)へ保存した。frozen executableはGitへ含めず、`.artifacts/0055-before`、`0055-wire`、`0055-final`に置く。

| 応答         |    前回版 |  Rustのみ | Rust＋JS最終版 | Rustの倍率 | JSの追加倍率 | 全体の倍率 |
| ------------ | --------: | --------: | -------------: | ---------: | -----------: | ---------: |
| rtla         | 449.16 µs | 409.07 µs |      377.36 µs |     1.10倍 |       1.08倍 |     1.19倍 |
| idealis-core |  33.29 µs |  31.20 µs |       29.74 µs |     1.07倍 |       1.05倍 |     1.12倍 |

legacy対照はrtlaが54.47/54.54/54.61 µs、coreが11.64/11.66/11.06 µs。rtlaの共通費用はstage間でほぼ一定だった。coreは対照も約5%変動したので、1.12倍をoracleだけの改善率として一般化しない。最終版もlegacy比較に対しrtla約6.91倍、core約2.69倍の費用がある。

## 最終版に残る費用

[最終区間raw](../runs/0055-phases-controls-final/result.json)はsnapshot、JSON文字列化、UTF-8 encode、Wasm dispatch、decode/parse、全compareOracleを正順/逆順で測る。各20 samples・75 warmup呼出・25 iterations。全出力は時計を止めてから固定期待値へ照合し、入力準備と出力破棄を時計外へ置く。

| rtla応答の区間    | 最終版のmedian |
| ----------------- | -------------: |
| snapshot pair     |       72.71 µs |
| JSON文字列化      |       36.23 µs |
| UTF-8 encode      |        3.96 µs |
| Wasm dispatch     |      147.57 µs |
| decode/JSON parse |       71.43 µs |
| 全compareOracle   |      331.44 µs |

区間にはloopと時計の費用も入り、独立して用意した入力を使うため、区間値の合計を全照合時間とはみなさない。[初期before区間](../runs/0055-phases-controls-before/result.json)とは測定時刻も離れたので、主な速度比較には直近のbeforeを含む往復runを使う。mismatch区間の全照合は6.01→6.20 µsと約3%増、Unicodeは6.70→6.69 µsとほぼ同じだった。10%以上の後退はなかった。

[最終profile](../runs/0055-profile-final/summary.json)も別runで全照合を5000回記録した。ContentDeserializerの読み直しは解消したが、JSONのparse/serializeとsnapshot自体は残る。次の候補は全reportと拒否条件を保ったJSON境界の費用。

## CriterionによるRust kernelの比較

[Criterion raw](../runs/0055-kernel/manifest.json)はbefore/after/after/beforeで同じ8入力を測る。20 samples、warmup 0.5秒、measurement 1秒。payloadのparse・照合・全reportの構築を含み、入力準備と出力破棄は含まない。実行ファイルとbench sourceのhashを検査し、build/test/profileを並行しなかった。candidateは`.artifacts/0055-kernel-after`へ保存した。最終修正はJSだけなので、このRustの実行ファイルは変わらない。

| 入力          |  Value 前回→今回 |   倍率 |  bytes 前回→今回 |   倍率 |
| ------------- | ---------------: | -----: | ---------------: | -----: |
| 単独          |     3.33→2.91 µs | 1.14倍 |     2.58→2.22 µs | 1.16倍 |
| 32 cases      | 298.35→236.96 µs | 1.26倍 | 248.53→208.00 µs | 1.19倍 |
| 入れ子256要素 | 247.61→190.90 µs | 1.30倍 | 214.76→167.53 µs | 1.28倍 |
| unknown       |     5.35→4.73 µs | 1.13倍 |     4.26→3.78 µs | 1.13倍 |

最終版rtlaの往復は目標1.15倍を満たした。core・unknown・mismatchの対照に10%以上の後退はない。Rustと、setter互換を保ったJSの変更を採用する。

## 互換性と探索中の失敗

[旧新版native/Wasm](../runs/0055-compat-combined/result.json)の56ケースは全応答が一致した。最終修正でRust/Wasmのbinaryは変えていない。[最終snapshot比較](../runs/0055-snapshot-compat-final/result.json)では118入力のsnapshotと236の全reportが一致し、13種の拒否入力と2種の単独surrogateのbridge拒否を維持した。Unicode、bigint、NaN、負のzero、特殊key、共有参照、深さ境界、非列挙index、accessorと疎配列を含む。

Rustの挙動テストは、公開Observationのserdeを独立した読み取り基準に使う。564形状をreference/actualへ入れた1128通りで、受理/拒否と保持した観測を比較する。float、負のzero、整数の上下限も含む。拒否variantのmessageは直接visitorの診断へ変わる場合がある。error envelopeと拒否条件を維持するが、診断文の完全一致は契約にしない。

最初のRustテスト案は`[kind, payload]`が拒否されるという誤った仮定で失敗した。公開serdeと凍結nativeを実行し、既存のsequence表現として受理されることを確認した。厳格化せず、この形式も維持する契約テストへ修正した。

[最初のsnapshot互換検査](../runs/0055-snapshot-compat/failure.json)も、単独UTF-16 surrogateをbridgeで受理できるというfixtureの仮定で中断した。旧版も拒否するため、snapshot内容の一致とbridgeの拒否を別々に検査した。別runで完了し、この制約を新規対応とは数えない。

[最終検証](../runs/0055-validation-final/result.json)にcheck、実ブラウザ、Rust oracle、既存eval契約147件、runtime、evaluate/replay、mutation、cross/async、配布consumer、SUPPORT/Jevを残す。mutationは109件のcacheを使い、104 killed・5 survived、score 95.4%。新たな変異実行とは扱わない。[測定手順](../workspace-oracle/README.md)で再比較できる。
