# 0112 条件と更新の読み取り

[事前の計画](../plans/0112-rmodel-readability.md)どおり、同じ入出金、ロック、セッションを `action` と `behavior` で読み比べた。対照の3モデルはコミット `2e5c2c37b` のものと同一。各対で rmodel IR と参照評価器の結果が一致した。探索が完了した判定は入出金3件、ロック5件、セッション6件だった。[記録した結果](../runs/0112-rmodel-readability-development/summary.json)

| 題材       | `action` | `behavior` |
| ---------- | -------: | ---------: |
| 入出金     |      8/9 |        9/9 |
| ロック     |      9/9 |        9/9 |
| セッション |      9/9 |        9/9 |
| 合計       |    26/27 |      27/27 |

TypeSafe の `jev-1.14.0` に、一回につき片方の構文で三問を出した。誤りは `action` で、一方の口座に入金した後「どちらの残高も変わらない」を選んだ一回答。リクエスト時間の中央値は `action` 133 ms、`behavior` 132 ms。これは API リクエストの観測値で、読み手個人の所要時間や性能差の指標には使わない。[全回答](../runs/0112-rmodel-readability-development/responses.json)

全体で正答が増え、題材ごとの後退がなかったため、事前の基準では代理読み手の改善と判定した。差は一回答で、同じモデルによる選択肢を巡回させた比較。人の読みやすさは未測定。文法の選定では `behavior` を引き続き使い、この結果を次の判断の証拠に加える。

`just idealis-model-readability-eval <新しいrunディレクトリ>` は TypeSafe API を使って比較を実行する。`just idealis-model-readability-replay evals/runs/0112-rmodel-readability-development` は保存した回答から同じ採点結果を再現する。
