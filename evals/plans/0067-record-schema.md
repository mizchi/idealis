# 0067: 同じrecord値のschema再検査

[#69](https://github.com/mizchi/idealis/issues/69)。基準は4587a3ea。SYMのnode行を作る際、先頭recordのschemaを検査してから各nodeのschemaを組み立て直している。完全に同じScalarのrecordだけは再構築を省く。shape.lenの論理workは従来と同じ順で課金し、異なるrecordのschema・型検査とエラーを維持する。Arenaをまたぐcache・新しい共有metadata・縮約条件の拡張は加えない。

旧版で同一recordのnode関数について、key・先頭代表とkey/代表の論理workを先に検査する。TLCStateMutの具体値と代表の分離、FcnRcdValueのimmutableな値の扱いを照合する。既存の異なるschema・node参照・typed値の拒否も保つ。

同じsymmetry benchとfixtureを使い、before/afterのbinary・全sourceを固定する。最初はrecord N12の全展開/代表展開と1/8 workerの4本をABBA、20 samples、warmup 0.5秒、measurement 1秒で測る。build/test/profileを重ねない。約5%以上のnative短縮を採用根拠とし、小さい差なら固定binaryの4本を一度だけ再確認する。追加で5%へ届かなければproductionの変更を撤去する。10%以上の後退も固定binaryで確認する。

有用なら21本のSYM/通常14本とsource境界・TLCの対照、必要な全体検査へ広げる。撤去する場合はsafety_symmetry.rsのproduction部分が4587a3eaと同じことを確認し、追加した挙動testとcheck/browser、fmt/clippy、portable、runtime build/test、evaluate/replay、SUPPORT/Jevを確認する。rawは追記のみ。不採用のsource・全sample・論理workの対照を保存する。

これを今回の最後の小さい仮説とする。shape cache・borrowed entries・小整数圧縮・worker制御の拡張は、実モデルで支配費用やRSS制約を示せるまで保留する。残る費用を推測だけで高速化と呼ばず、採否と再開条件をiterationと#69/#63へ記録する。

初回は代表展開で約4〜5%、全展開で約6%短縮した。採否の境界に近いため、同じ固定binary・同じ4本をもう一度ABBAで確認する。全4本を報告し、初回とは混ぜない。全展開の1/8 workerで5%以上の短縮が再現し、代表展開にも後退がなければ、recordのschema検査に限定した改善として次の対照へ進む。

追加4本では約8〜16%短縮したが、初回との幅があるため、21本のSYMと14本の通常kernelへ広げる。source境界は既存driverの8モデル・計12設定、record N12は1 query、他は100 query/sample、1 warmup + 3 samplesで固定する。同じworker設定の旧版完全応答へworkも含めて比較する。小さい単発APIの雑音を避けるため、初回から100回反復を用い、0065の単発値とは混ぜない。
