# 0062: 共有IRの統合と最新版の比較

P0の比較・profile更新 [#64](https://github.com/mizchi/idealis/issues/64) と共有IR [#60](https://github.com/mizchi/idealis/issues/60) に対応した。draft PR #62のa89e1d9cを、0061の3f249cc7へローカル統合した。v1を既定に保ち、v2は明示選択する。共有IRを一般的な探索高速化とは判断しない。inlineの膨張とslot混線を解消する機能として採用し、残るv1の費用増も記録する。

[事前planと条件変更](../plans/0062-p0-checker.md)を参照。初期planは3146b783で実行前にcommitした。以後の条件追加は実行前に追記したが、merge作業中のため個別のplan commitはない。固定benchの反復は、新しい独立モデルや新seedでの確認とは数えない。remoteのpush・PR mergeはこの作業に含めない。

## 統合した契約と制限

両frontendは`irVersion: 2`でexpression定義表、call-by-nameの引数、callerの字句環境を生成する。calleeのbinder slotをcallerと分離し、要求時のcurrent/next/primeを使う。call/frame入口と引数要求はworkへ課金する。v1から自動再試行しない。

v2のmodule連結・設定置換・SYMMETRY、再帰とaction template共有は未対応。action/temporal helperはcall siteごとのanonymous frameを使う。v1のSelectSeq/SortSeq/Assertのsource loweringもv2では未対応だが、対応するv2 IRの検証・実行は別に検査する。詳細は[契約](../../docs/safety-ir-v2.md)と各crateのSUPPORTを参照。

既存SYMMETRYのwire decodeとprepared定数cacheを維持し、共有定義・frameも走査対象にした。深いaction frameの準備は明示stackで行う。v2のSYMMETRYは理由を示して拒否し、独立性を確認できないPORは通常探索へ戻す。deep dropには既存の追加演算の子も含めた。

固定TLCのtemporal helperには[既知の乖離1入力](../fixtures/tlc-operator-sharing/divergences.json)がある。TLC一致と数えず、元の違反ログと意味上の結果を別に検査する。THEOREMの証明、全TLA+互換、非公開モデルの受理を主張しない。

## 通常探索の同条件比較

Apple M5 arm64、32 GiB、Node 24.22.2、tla-ts 0.0.10、固定したaarch64 Temurin 21.0.13+9とtla2tools v1.7.4を使った。VM propertiesと実行ファイルのarm64を確認した。ホストは専用ではない。

全4モデルを同じsource/configで完全探索した。各1 warmup + 3 samples、経路順を回転・反転した。4つの小モデルの正常/故障72対照と、largeの144実行を保存した。native/Wasmは共通TS frontendとNode起動・IR転送を含むv1の時間で、以下は秒の中央値。

| モデル        |     状態数 / generated | tla-ts 1 |  TLC 1 | TLC 8 | Node Wasm 1 | native 1 | native 8要求 |
| ------------- | ---------------------: | -------: | -----: | ----: | ----------: | -------: | -----------: |
| counter N48   | 5,308,416 / 21,233,665 |    6.490 |  6.913 | 7.922 |       4.195 |    3.218 |        2.968 |
| function N20  | 1,048,576 / 20,971,521 |    5.424 | 12.781 | 4.325 |       4.613 |    3.485 |        2.440 |
| channel N7    | 2,097,152 / 22,020,097 |   15.728 | 23.247 | 6.285 |      19.977 |   13.809 |        5.497 |
| ring N1000000 |  1,000,000 / 1,000,001 |    0.407 |  0.926 | 1.474 |       0.255 |    0.182 |        0.181 |

nativeの8-worker要求はTLC 8比で2.67 / 1.77 / 1.14 / 8.15倍だった。worker数は上限で、幅1のringは逐次へ戻る。tla-tsの並列性能は測っていない。NodeとTLCのheap指定は8 GiBで、共通の厳密なRSS制限ではない。

同じrunの統合前/後では、functionのnative 1/8は3.215→3.485秒、2.296→2.440秒と約8%/6%遅くなった。channelのnative 1も12.980→13.809秒と約6%遅い。native 8は6.446→5.497秒だったが、変更前の反復の幅が大きく、因果を確定しない。Wasm channelも18.790→19.977秒と約6%遅く、同じrunのtla-tsより約27%遅い。Wasmのtla-ts超えを採用条件にはしない。

[raw比較](../runs/0062-comparison/result.json)と[全sample・範囲・14本の集計](../runs/0062-summary/result.json)を参照。古い0059/0061の時間を、この表の比率へ混ぜていない。

## 共有によるサイズとcompile費用

同じ最終binaryのv1/v2で比べた。以下のcompileは、常駐native JSONLへのsource要求からIR応答のdecodeまでを含む。探索だけの速度ではない。各1 warmup + 3 samples。

| sharing chain |      v1 IR bytes | v2 IR bytes | v2定義数 | v1 compile ms | v2 compile ms |
| ------------- | ---------------: | ----------: | -------: | ------------: | ------------: |
| 10            |          330,878 |       4,707 |       12 |        35.534 |         0.359 |
| 20            | nesting 40で拒否 |       8,677 |       22 |          拒否 |         0.407 |
| 30            | nesting 40で拒否 |      12,647 |       32 |          拒否 |         0.551 |
| 60            | nesting 40で拒否 |      24,557 |       62 |          拒否 |         0.936 |

chain 10は約70分の1のIRと約99倍のcompile短縮になった。未選択branchの木を作らない効果で、両枝を実際に評価する指数仕事を除いた結果ではない。eager chainはmaxWork 256でbounded/workになることを検査する。TS/Rustの構造・native/Wasm応答を別のcohortで照合する。

## 公開モデルと起動費用

Quint 0.33.1の既存6生成物と、TLC repository d8fb231dの原本[TCommit](../fixtures/tcommit/provenance.json)を使った。原本は改変していない。TCommitはRM=8、TCInit/TCNext、TCTypeOK/TCConsistent、deadlock無効、縮約なし。新しいTLC実行は6,816状態・53,513生成で、Rust v1/v2と一致した。THEOREM自体を証明したとは扱わない。

compile/check/sourceの常駐要求をまとめた264試行（拒否対照を含む）と、84回の新規native processを保存した。以下はTCommitの中央値。IR checkにもdecode・prepared構築・応答を含み、再利用するSafetyProgram handleではない。

| 経路    | compile要求 ms | IR check要求 ms | source check要求 ms | 新規process ms | 最大RSS MiB | IR bytes |
| ------- | -------------: | --------------: | ------------------: | -------------: | ----------: | -------: |
| 0061 v1 |          0.694 |         111.573 |             109.464 |        114.847 |        5.52 |    8,037 |
| 最終 v1 |          0.697 |         124.674 |             124.038 |        124.050 |        5.95 |    8,037 |
| 最終 v2 |          0.596 |         126.123 |             132.089 |        129.320 |        5.81 |    8,367 |

起動込みはNode親のJSON encodeとmacOS time wrapperを含む。Node起動・TS frontendは含まない。小さいQuintの新規processは約2.6-3.0 msで、native APIの代表値0.04-0.37 ms程度より境界費用が大きい。小さい仕様ではv2の定義表がIRを増やす場合もあった。TCommitのv1も統合前より遅くなっており、v2の一般的な探索高速化とは判断しない。

TS frontendはQuintのTLC/Variants importとTCommitのmodel valueを従来から拒否する。Rust frontendの受理と区別し、生成物を削ってTSへ合わせない。[実モデル・chainの全記録](../runs/0062-corpus-fixed/result.json)、[fresh TLC](../runs/0062-tcommit-oracle/result.json)を参照。

stripped nativeは2,107,312→2,636,528 bytes、core Wasmは2,420,013→3,087,226 bytesになった。gzipではnativeが919,283→1,185,691 bytes、Wasmが595,894→798,380 bytes。共有対応と深いIRの破棄を含むbundle全体の差で、runtime stateのRSSとは分ける。

## profileで選んだ調整

統合直後はcounter 1が+62%、function 1が+39%、channel 1が+29%の時間になった。採用を保留して、名前付きreleaseを通常計測と別にprofileした。eval自身のself sampleがcounterで約38%を占めた。

1. A: 小さいleaf dispatchを戻し、深さ検査はnon-leafの境界へ移した。深いscalar/callを大きい集合演算用stackへ通す途中版はstack overflowした。既存境界テストを弱めず、小さい別関数へ分岐して修正した。
2. B: 深さの非公開fieldをu8へ縮めた。v1の長いENABLED branchへ128上限を適用する回帰を追加テストで再現し、v2だけの制限へ戻した。branch深さの加算は飽和させる。4モデルではA比約2-4%の短縮だった。
3. C: branch visitorを静的dispatchにする案はchannelで約24%短縮したが、counter/functionが約12%遅くなった。撤去し、source/binaryと全測定を残した。
4. D: Bへ戻し、小さいleaf evalをinline(always)にした。B比でcounter/function/ringは約8-9%、channelは約4%短縮した。最終版はA/B/Dを採用する。

最終14本ではcounter/ringのsearchが約3-5%短縮し、function 1が約4%、channel 1が約5%遅かった。channel 8の小さいN5は+11.5%だったが、固定binaryの追加ABBAは+2.0%だった。large N7の結果と併記し、良い反復だけで判断しない。

[初回](../runs/0062-kernel/result.json)、[A](../runs/0062-kernel-a/result.json)、[B](../runs/0062-kernel-b/result.json)、[不採用C](../runs/0062-kernel-c/result.json)、[D](../runs/0062-kernel-d/result.json)、[最終14本](../runs/0062-kernel-final/result.json)、[N5再確認](../runs/0062-channel-confirmation/result.json)を参照。Criterionは各20 samples、warmup 0.5秒、measurement 1秒、ABBA。2つのmean推定の算術平均で、poolした信頼区間ではない。

最終profileではcounterのscalar評価・branch訪問、function/channelの値アクセスとintern、TCommitのnon-leaf評価・cell/positionが残る。TCommit v2のframe生成はinclusive 34/2267 sample weight程度で、ここでは支配費用ではなかった。sample weightをwall比と同一視しない。[通常4モデル](../runs/0062-profile-final-summary/result.json)と[TCommit v1/v2](../runs/0062-tcommit-profile/result.json)を参照。

## 評価基盤と検証

analysis parityとframe decoderを`just analysis-test`・CIへ組み込んだ。Chromiumでは60定義chainの実行とeager chainのwork境界を確認する。単独TS/Rust package consumerにも明示v2を追加した。

Jevは巨大なimpl blockのexcerpt上限を超えたため、method境界で読める形にし、v2のlowering/検証/dropへ比較を分けた。上限は引き上げない。SUPPORTが参照するcrate外資料もmanifestへ含め、資料の変更で保存記録が古くなることを自己テストする。READMEの最短反例の主張も通常BFSへ限定した。live/offlineの46比較で矛盾・未評価0を確認する。

初回Redのrequest envelope不足、途中のstack overflow、集計器の2モデル固定、Wasm例外とnative error packetの扱い差、checklistのfixture不足、APIキーのないskipと検査済みは混同しない。[途中log](../runs/0062-integration/)と失敗した[corpus](../runs/0062-corpus/error.json)も残す。

最終のcheck、analysis 4,406件、safety 1,185件、compiler/portable、実Chromium、fmt/clippy、runtime、cross-language、package consumerが通った。旧版との[156モデル・1,560要求](../runs/0062-parity/result.json)はnative/Wasmの完全応答が一致した。既存TS対象のmutation scoreは95.4%で、109件はcache再利用、今回のRust評価器のmutation評価ではない。

[21検査の初回](../runs/0062-verification/result.json)はusable-evalへのCLI引数指定を誤り、生成前のreplayも失敗した。runnerを修正し、[該当2検査](../runs/0062-verification-repair/result.json)と[usable実行・再生](../runs/0062-usable/result.json)を追加して通した。初回のfailedをcompleteへ書き換えない。検証中のcrate sourceは不変だった。

## 再実行

Rust buildとfreezeを時間計測の前に行う。`just p0-comparison BEFORE AFTER JAVA JAR ARCHIVE RUN`、`just p0-corpus BEFORE AFTER JAVA JAR ARCHIVE RUN`、`just p0-profile NAMED_BINARY RUN`を使う。runは既存ディレクトリへ上書きしない。通常の時間・RSS・profile・検証は別に実行する。

次はv1に残る共通評価費用、TCommitのcell/position、Data slot lookup、PBTの小さい要求の再利用を優先する。frame生成だけの調整や静的visitorの一律適用を先に進めない。採否は[#63](https://github.com/mizchi/idealis/issues/63)へ戻す。
