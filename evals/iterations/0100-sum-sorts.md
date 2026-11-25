# 0100: 和のsort（多態のentity）

[plan](../plans/0100-sum-sorts.md)のとおり、sig・enumをまとめる和のsortを入れた。0099の型検査で書けなくなった`Cat + Dog`（`Animal`の子）や`Account.owner in Customer + Staff`が、部分型の関係で型を持つ。意味はAlloyの`abstract sig`と`extends`と同じで、Alloyへの写し方も1対1。

## 計画からの差（結果の前）

- 構文を`sort Party = Person | Org`から`type Party = Person + Org`に変えた。`X = A | B`はQuintの直和型（構成子のタグ付きの和）と同じ見た目だが、意味は部分集合と部分型で違う。mizchiと相談し、集合の和の`+`で書く形にした（普通のプログラミング言語の利用者に素直に見える）。意味と写し方は変えていない。読みやすさは測っていない。
- 差分テストの実行器を、seedごとの子プロセスと600秒の上限に変えた。1つのプロセスでseedをまたいでメモリが伸び、系が強制終了した（seed 5141の前で2回）。子プロセスの異常終了と時間切れは`crash`の行として記録し、一致にも食い違いにも数えない。

## 回帰テスト

- `a not in Cat + Dog`（`a: Animal`）、`Account.owner in Customer + Staff`、和のsortの上の量化、和のsortの引数への子のatomの代入、子の集合の比較。参照評価器・rtlc・ralloyのSATで、結果の前に書いた期待と一致した。
- 拒否: 和に属さないsigの混在、2つの和への所属、和の子への和、子のsortの変数への和の値の代入。

## 差分テスト（check数、`Declarations`を含む）

| 分類                                                                     |  dev（5000〜5299） | 確認（6000〜6299） |
| ------------------------------------------------------------------------ | -----------------: | -----------------: |
| 比べたseed（全300）                                                      |                299 |                299 |
| 3つのengineが一致                                                        |                554 |                502 |
| 空で一致                                                                 |                252 |                306 |
| そのうち`Declarations`の違反                                             |                 52 |                 53 |
| rtlcのエラー（[#112](https://github.com/mizchi/idealis/issues/112)ほか） |                 60 |                 81 |
| rtlcの予算切れ                                                           |                 67 |                 53 |
| 参照評価器の予算切れ                                                     |                 26 |                 17 |
| crash                                                                    | 1（5141、SIGKILL） |   1（6130、600秒） |
| 判定の食い違い                                                           |                  0 |                  0 |

[dev](../runs/0100-sum-sorts-development/results.jsonl)・[確認](../runs/0100-sum-sorts-confirmation/results.jsonl)。生成器は和のsortを確率30%で加え、parseの失敗は0件。和を含むモデルの数は記録していない。

crashした2つのseedは、どちらも参照評価器が原因だった（結果を見た後に単独で調べた）。各関係は列挙できる大きさ（tuple 20以下）だが、設定の組み合わせが2億（5141）・2900万（6130）通りあり、参照評価器は（1）組み合わせをすべてメモリに作ってから回し、（2）予算に状態だけを数えて設定と初期状態の候補を数えていなかった。どちらも[0101](0101-checker-registry.md)で直し、5141は60秒・32MBで、明示状態と同じ判定（violated・reachable・unknown）を返した。

## 書けるようになった表現と、まだ書けないもの

- 書ける: 子のsortを合わせた集合、和のsortで宣言した関係・引数・量化。
- まだ書けない: 入れ子（ralloyのAlloyのcorpusの`Triangle extends Polygon`、`Polygon extends Shape`）、自分のatomも持つ親（`sig Student extends Person`で、`Person`が`abstract`でないもの）、1つのsigの複数の和への所属。
