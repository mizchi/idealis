# 0063: Data定数のslot検索

[#65](https://github.com/mizchi/idealis/issues/65)で通常評価・Data slot・関数lookupを調べた。162fd73dを基準とし、内部address専用hashだけを採用する。値アクセスと関数lookupのinlineは、membershipの後退が再現したため撤去した。評価深さ・work課金・公開IR・APIは変更していない。

[事前plan](../plans/0063-evaluator-tuning.md)を3e804b5fで先にcommitした。条件追加は実行前に追記したが、個別のplan commitはない。rawは上書きせず、不採用と比較器の失敗も保持する。push・remote mergeは含めない。

## 採用した変更

PreparedProgramのData式addressを索引するHashMapを、SipHashからMurmurHash3 fmix64による非公開hashへ変えた。対象は内部のusize addressだけで、完全key比較は残る。ユーザー値、集合・関数、visited、membershipのhashと等価性は変えない。

定数はArenaごとに初回の成功後だけcacheする。未選択branch、失敗後の再評価、未登録のcloned expression、worker local Refのremapも従来の契約に従う。新しい命令VMや再利用handleは追加していない。

## 公開TCommitでの比較

原本TCommit、RM=8、縮約なし、同じ予算で比較した。各1 warmup + 3 samples、経路の順序を回転・反転する。Apple M5 arm64、32 GiB、Node 24.22.2、非専用ホスト。時間はmsの中央値で、以前の0062の時間から比率を計算していない。

| 経路         | IR check要求 前 |      後 | 新規Rust process 前 |      後 | 最大RSS MiB 前 |     後 |
| ------------ | --------------: | ------: | ------------------: | ------: | -------------: | -----: |
| native v1    |         143.770 | 113.244 |             138.697 | 118.614 |           5.97 |   5.91 |
| native v2    |         140.548 | 118.318 |             143.866 | 125.934 |           5.78 |   5.77 |
| Node Wasm v1 |         203.201 | 171.619 |              対象外 |  対象外 |         対象外 | 対象外 |
| Node Wasm v2 |         208.130 | 187.079 |              対象外 |  対象外 |         対象外 | 対象外 |

nativeのIR check要求はv1が約21%、v2が約16%短縮した。起動込みは約14.5% / 12.5%短縮した。check要求にはwire encode/decodeとprepared構築を含む。新規processはNode親のJSON encodeとmacOS time wrapperを含み、Node起動・TS frontendは含まない。RSSの小さい差を一般的なメモリ削減とは判断しない。

Quint6生成物・TCommit・共有chain 10/20/30/60の352試行と、112回の新規native processを保存した。同じIR versionのbefore/after native/Wasmについて、version自体、compiled program、workを含む完全result、unsupported packetの264比較が一致した。v1の深いchainの48拒否を合格へ読み替えない。TCommitは全経路で6,816状態・53,513生成、v1のwork 2,361,523 / v2のwork 2,857,051で、保存済みTLC oracleの計数とも一致する。

[全sample](../runs/0063-corpus-selected-fixed/result.json)、[範囲・中央値・完全応答の照合](../runs/0063-corpus-summary/result.json)を参照。TS frontendの既存の拒否と、既知のTLC temporal helperの1乖離は変更していない。

## 14本のkernelでは全般的な短縮にならない

同じbinary・同じ入力、20 samples、warmup 0.5秒、measurement 1秒のABBAで比較した。以下は2つのCriterion mean推定の算術平均から計算した時間の変化。poolした信頼区間ではない。

| search   | 1 worker | 8 worker要求 |
| -------- | -------: | -----------: |
| counter  |    +3.6% |        +7.2% |
| function |    -5.3% |        +6.1% |
| channel  |    +4.9% |        +1.8% |
| ring     |    -1.9% |        -2.2% |

残る6本は、新規functionのinternが-5.6%、既存internが+7.6%、EXCEPT functionが+1.6%、EXCEPT recordが約0%、membership functionが+5.6%、channelが+6.6%だった。10%以上の後退はなかったが、全モデルの高速化とは扱わない。TCommitでの短縮を採用根拠とし、小さいkernelの後退も残す。

[最終14本のraw](../runs/0063-kernel-selected/manifest.json)と[全推定・各反復の区間](../runs/0063-kernel-selected-summary/result.json)を参照。driverは実際にruntimeから読むinputs.jsonの原本・hashを保存し、実行前後の不変を検査する。bench自体のhashもbefore/afterで一致する。

## 撤去したinlineと再確認

Aはhashだけ。BはAへArena::cellのinline(always)、CはBへFunction::positionのinline(always)を追加した。4モデルの逐次searchでは一部が約4-8%短縮した。しかしCと基準版の14本ではmembership/channelが+11.7%、channel逐次searchが+20.5%、既存functionのinternが+15.0%だった。

該当3本を同じ固定binaryで再確認すると、順に+24.2%、-7.7%、+4.3%となった。membershipの後退が再現したためB/Cを両方撤去した。この組み合わせの結果だけで、どちらのinlineが原因かは断定しない。後退した全14本も保存する。

初回Aの4本には約0.7秒のTS driver buildが重なった。採用判断は後の独立した時間帯の14本と実モデルを使う。最初の実モデル比較も同じv1でcheck 121.52→103.28 ms、cold 124.65→106.74 msだったが、最終runの時間へ混ぜない。

[A](../runs/0063-kernel-a-summary/result.json)、[B](../runs/0063-kernel-b-summary/result.json)、[C](../runs/0063-kernel-c-summary/result.json)、[不採用Cの14本](../runs/0063-kernel-final-summary/result.json)、[3本の再確認](../runs/0063-kernel-confirmation-summary/result.json)、[初回実モデル](../runs/0063-corpus-a/result.json)を参照。

## profileと残る費用

通常の時間計測を終えてから、名前付きreleaseでTCommit v1/v2を各16回、1 ms間隔で採取した。各回の完全探索と計数を検査する。v1の1,972 sample weightのうちcellが194、positionが171、non-leaf評価が166だった。v2の1,967では順に191、156、175だった。sample weightをwall時間の割合や、変更の因果と同一視しない。

Data slotのhashを軽くした後も、値アクセス・lookup・共通評価が残る。inlineを一律に付ける方法は今回採用しない。domainの構造を利用するlookupなどは別の仮説として扱い、小さいPBT要求のSafetyProgram再利用も次の候補とする。[profile原本と集計](../runs/0063-tcommit-profile/result.json)を参照。

## 比較器の修正と検証

v2非対応だった古いbaseline用の比較器は、before経路でirVersionを省略していた。今回before-v2を追加した際も省略し、深いchainで検査が失敗した。before-v1だけ省略するよう修正し、[失敗run](../runs/0063-corpus-selected/error.json)を保持して新規runへ再実行した。集計時も全packetのprogram.versionを検査し、ラベルだけでv2とは数えない。

[21検査](../runs/0063-verification/result.json)はすべて通り、検証中のcrate sourceも不変だった。analysis 4,406件、safety 1,185件、Chromium、compiler/portable、fmt/clippy、runtime build/test、evaluate/replay-evaluation、cross-language、packageの単独consumerを含む。[156モデル・1,560要求](../runs/0063-parity/result.json)もbefore/after native/Wasmの完全応答が一致した。

[usableの実行・再生](../runs/0063-verification/usable/result.json)は600正常対照、1,200比較、1,200 native結果、故障20/20件の検出と反例20件の再生が通った。既存TS対象のmutation scoreは95.4%で109件をcacheから再利用した。今回のRust hashのmutation評価ではない。Jevはlive/offline 46比較で矛盾・未評価0だった。POR比較のcontextを適用判定へ絞り、別に行うIR契約の比較と抜粋上限を保った。

[source archiveと検証logの一覧](../runs/0063-audit/result.json)を保存する。v1既定、明示v2、拒否とboundedの区別を保ち、対応範囲を拡大したとは扱わない。

再実行は`just evaluator-kernel BEFORE AFTER RUN FILTER`、`just evaluator-summary RUN OUTPUT COUNT`、`just evaluator-corpus BEFORE AFTER JAVA JAR ARCHIVE RUN`、`just evaluator-corpus-summary RUN OUTPUT`を使う。buildとprofileを時間計測へ重ねない。
