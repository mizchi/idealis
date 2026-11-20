# 0048: 量化fairnessと純粋な標準演算

## 仮説と範囲

定数の有限domainに対するSPECIFICATION内の量化fairnessを、既存の時間論理IRで検査できるようにする。forall/exists、tuple、入れ子、空domain、直接WF/SFの対照を固定する。stateに依存するdomain、無制限・時間変数量化、量化したInit/Nextの一般的な分解は対応範囲に含めない。

SequencesのSelectSeq、TLCのSortSeqとAssertを追加する。SelectSeqのpredicateは非再帰の1引数operator名、SortSeqの比較は非再帰の2引数operator名に限定する。既存foldと同様に定義元のscopeとLETの捕捉を保持する。一般の高階引数やLAMBDAへ対応を広げない。Assertは引数の評価順とmessageの評価をTLCに照合する。

RandomElement、TLCGet、TLCSetは未対応として残す。乱数やworker固有の状態を完全探索の意味へ暗黙に持ち込まない。

## 意味論と対照

比較元はTLC revision 5a47802。Sequences.SelectSeq、TLC.SortSeq/Assert、SpecProcessor.processConfigSpecのソースを読む。固定した入力をTLC/native/Wasmへ渡し、判定・計数・値と反例を比較する。

- fairnessあり/なし、forall/exists、WF/SFの対照で活性違反の有無が変わることを確認する。反例は元の仕様とfairnessから再生する。
- SelectSeqの順序、重複、空列、Bool以外のpredicate、関数表現のsequence、誤った引数種を確認する。
- SortSeqの挿入順、同順位の扱い、常に真/偽の比較、不正な比較結果、空列・singletonを確認する。TLCの比較順序を保持し、独自の標準sortへ置換しない。
- Assertの真/偽、Bool以外の条件、messageの値種、message内の評価エラー、未評価の分岐を確認する。
- callbackのslot、scope、prime、ネストとwork予算を検査する。予算切れ・型エラーをcompleteへ変えない。

## 費用と採用判断

既存の17本のCriterionを残し、追加演算と量化fairnessのcompile/checkを分けて測る。20 sample、warmup 0.5 s、measurement 1 sを使い、計測中は他のbuild/test/TLC実行を並走させない。parser・IPC・起動は測定外。runは新しいディレクトリへ保存し、以前の証拠を変更しない。

対応項目はSUPPORT.mdへ更新する。fmt/clippy、crate・native/Wasmの契約、Chromium、just check/browser、配布consumer、Jevのstaged/オフライン検査を通す。速度改善や全TLA+互換ではなく、固定した範囲の対応拡張として判断する。
