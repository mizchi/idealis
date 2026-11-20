# 0066: Ref domainの添字候補

[#68](https://github.com/mizchi/idealis/issues/68)で関数lookupへ小さいprobeを加えた。Ref keyと先頭Refの差を添字候補にし、その位置のkeyが完全一致する場合だけ使う。既存Int fast path・線形fallback・公開IR/API・論理workを保つ。hash/cacheやinline指定を足していない。

[事前plan](../plans/0066-domain-offset.md)は7d48352aでcommitした。基準は54f39529。TLCのFcnRcdValue.apply/selectの実ソースを読み、domain外selectと関数適用のエラーを区別する。2つのArena経由の挙動testを旧実装・新実装で通し、疎なdomain、Refの割り当てと意味の順序が逆の場合、欠落key、型違い、更新と元の不変性を検査した。

## 実モデルの初回と追加確認

原本TCommit RM=8、Quint6生成物、sharing chainの11入力をv1/v2で比較した。各1 warmup + 3 samples、順序を回転・反転する。before/after native/Wasmのcompiled program・version・result全体・拒否の264比較が一致した。TCommitの6,816状態・53,513生成も保存済みTLC oracleと一致する。

| TCommit経路 | before ms | after ms |
| --- | ---: | ---: |
| native v1 IR check | 113.057 | 108.230 |
| native v2 IR check | 117.990 | 114.636 |
| native v1 新規process | 116.023 | 111.542 |
| native v2 新規process | 122.041 | 118.068 |
| Node Wasm v1 IR check | 170.425 | 165.581 |
| Node Wasm v2 IR check | 175.094 | 170.535 |

中央値。check要求はIR decodeと準備を含み、新規processはsource frontend・Node親のJSON encode・macOS time wrapperも含む。Node自体の起動は含まない。ピークRSSの中央値はnative v1が6,242,304→6,307,840 bytes、v2が6,225,920→6,242,304 bytesで、メモリ削減の根拠にはならない。[初回raw](../runs/0066-corpus/result.json)と[全sampleの照合・集計](../runs/0066-corpus-summary/result.json)を参照。

初回のnative短縮は約3〜4%で5%へ届かなかった。実行前にplanへ追加し、同じ固定binaryのnative IR checkだけを1 warmup + 12 samplesで確認した。各回は同じcompiled IRへ問い合わせ、完全応答を照合した。

| 追加確認 | before min / median / max ms | after min / median / max ms | 中央値の変化 |
| --- | ---: | ---: | ---: |
| native v1 | 103.561 / 105.014 / 106.865 | 96.375 / 97.611 / 98.689 | -7.0% |
| native v2 | 108.479 / 109.608 / 110.291 | 101.252 / 102.555 / 103.980 | -6.4% |

コンパイル・起動を除く常駐JSONL API区間。経路順を回転・反転し、[52試行の原応答](../runs/0066-tcommit-confirmation/result.json)を保存した。初回は他モデル・Wasmとも混在する常駐process、追加確認はTCommit IR checkだけのprocessである。条件が異なるため数値をpoolせず、起動込みの7%短縮とは主張しない。Apple M5 arm64、32 GiB、Node 24.22.2、非専用ホスト。

## 通常14本の後退確認

同じCriterionをABBA、20 samples、warmup 0.5秒、measurement 1秒で測った。search8本は約-3〜+1%、membership2本は約-2〜+1%、EXCEPT2本は約+1%、新規internは約0%、既存internは約+5.6%だった。10%以上の後退はなかったが、汎用の高速化とは扱わない。[14本のraw](../runs/0066-safety-kernel/manifest.json)と[各推定・反復の区間](../runs/0066-safety-summary/result.json)を参照。build/test/profileと時間測定は重ねていない。
