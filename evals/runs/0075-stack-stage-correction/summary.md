# 0075-stack-stage-correction

[0075-stack-stage](../0075-stack-stage/) の summary の表は、「上記以外の 33 セル」を 4 つの build すべてで「終了コード 0、`chain complete 6 7 20`」と書いた。保存した `chains-*.json` の 72 セルを数え直すと、この記述は before の build では誤りだった。0075-stack-stage の記録は変えていない。この記録が訂正である。

## 数え直した結果

`chains-<profile>-<side>.json` の 18 セルずつ（3 family x 6 長さ）を、終了コードと stdout で分けた。

| build | SIGABRT | exit 0 かつ `chain complete 6 7 20` | exit 0 かつ `chain error Safety expression nesting exceeds 48` |
| --- | --- | --- | --- |
| debug before | 3 | 7 | 8 |
| release before | 3 | 7 | 8 |
| debug after | 3 | 15 | 0 |
| release after | 3 | 15 | 0 |
| 合計 | 12 | 44 | 16 |

- 72 セルのうち 12 は parse か compile の失敗で SIGABRT。元の summary の記述どおり。
- 残りの 60 セルは終了コード 0 で、44 が `chain complete 6 7 20`、16 が before の build の `chain error Safety expression nesting exceeds 48` だった。後者は、before の checker が 48 段を超える連鎖を `check` の中で拒否した結果で、完了した検査ではない。
- before の build の 8 セル（debug と release で同じ）は、`if` 48・61、`flat` 61・127・128、`case` 48・61・127。after の build では同じセルがすべて `chain complete 6 7 20` になる。これは 0075 が連鎖を 1 段と数える変更の効果で、0075-stack の表の before の列と整合する。

## 訂正

- 0075-stack-stage の「上記以外の 33 セル ... 終了コード 0、`chain complete 6 7 20`」は、after の build では正しい。before の build では 33 セルのうち 25 が完了で、8 が checker の入れ子 48 の拒否だった（上の 8 セル）。
- 3 つの SIGABRT のセル（`if 127`、`if 128`、`case 128`）の記述は変わらない。
