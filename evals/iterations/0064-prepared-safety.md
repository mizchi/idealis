# 0064: 準備済みSafetyProgramとbatch

[#66](https://github.com/mizchi/idealis/issues/66)でRustのPreparedSafetyProgramとnative/Wasmのprepare/check/batch/releaseを実装した。同じIRへ異なる予算・optionsで問い合わせる用途に採用する。実装traceの照合は既存のfinite handleと区別し、反復しただけで新しい故障を検出したとは数えない。

[事前plan](../plans/0064-prepared-safety.md)は3e8d8a50でcommitした。基準はbef6255f。追加条件は実行前に追記し、各raw runを保持する。

## 所有と契約

準備済みRust objectがprogramをBoxで所有し、Data式のaddress索引とv2の静的解析metadataを保持する。Data索引へ値の参照やcloneを保存せず、評価時に渡された値を使う。v2 metadataは定義のarityと解析summaryを所有するため、unsafeや自己参照は要らない。

Arena・constant Scalar cache・visited・graph・work・coverage・symmetryの代表はcheckごとに新規となる。prepareでASSUME・定数invariant・未選択Dataを評価しない。既存one-shotの意味とwork、既定v1・明示v2を保つ。

batchは1〜256件のjobを指定順に実行し、各bounds/optionsへresultまたはerrorを返す。空・過大・未知handle・不正なJSON構造はprotocol error。jobの実行エラーで後続を省略しない。[APIとreleaseの契約](../../docs/prepared-safety.md)を参照。

## 反復と予算を変えるquery

Quint6、原本TCommit RM=8、small4モデル、sharing chain 10/60の13入力を使った。各IR v1/v2、1 warmup + 3 samples、経路順を回転・反転。TCommitは1/10回、他は1/10/100回を測った。v1のchain60拒否を保存し、速度の比較へ入れない。

以下はCounterを100回検査したmsの中央値。one-shotとbatchは同じ変更後binaryで、batchには準備・破棄の費用を含む。常駐native JSONLとNode WasmのAPI時間で、結果の照合・disk I/Oは含まない。

| query             | native v1 one-shot | batch | native v2 one-shot | batch | Node Wasm v1 one-shot | batch |
| ----------------- | -----------------: | ----: | -----------------: | ----: | --------------------: | ----: |
| 固定予算100回     |              3.883 | 0.643 |              5.319 | 0.497 |                 2.207 | 0.890 |
| 予算を変える100件 |              2.162 | 0.256 |              2.546 | 0.287 |                 1.868 | 0.374 |

予算を変えるrunはmaxWork=2^(index mod 20)、maxDepth=[0,1,2,4,16][index mod 5]で、boundedとcompleteを混ぜた。各jobは従来one-shotの完全応答へ照合し、前のresultや残りbudgetを使い回す実装では通らない。native v1のAPI合計は固定queryで約6.0倍、予算を変えるqueryで約8.4倍になった。

新規native processで100件を実行する時間も別に測った。Counter v1の固定queryはone-shot 7.216→batch 5.165 ms、予算を変えるqueryは3.954→2.709 ms。Node親のJSON encodeとmacOS time wrapperを含み、Node自体の起動は含まない。常駐APIの倍率をcoldへ当てはめない。

TCommitの通常探索10回は変更後one-shot 1,109.896→batch 1,053.669 ms（v1）、1,259.875→1,402.049 ms（v2）で、探索主体の用途で一律の短縮はない。少ない予算の10 queryではv1が1.064→0.347 msだった。予算、モデル、反復数ごとに使うかを判断する。

各runは2,336計測sampleと1,168 cold processを保存した。gzipの原応答まで再読し、各123,600件の返却jobをbefore one-shotへ照合した。固定入力・warmup・反復を含む件数で、独立したモデルや新しい故障の数ではない。

[固定queryのraw](../runs/0064-reuse/result.json)と[全区間・RSS・応答照合](../runs/0064-reuse-summary/result.json)、[予算を変えるraw](../runs/0064-budgets/result.json)と[その集計](../runs/0064-budget-summary/result.json)を参照。Apple M5、Node 24.22.2、非専用ホストで、他runの時間を比率へ混ぜない。

## 既存kernelの後退確認

同じ14本を20 samples、warmup 0.5秒、measurement 1秒のABBAで測った。8本のsearchは約-3%〜+9%、値操作は約-2%〜+0%、membershipは約+9%/+15%だった。membership/channelとfunctionの8-worker要求を固定binaryで再確認すると、約+3.1%/-0.5%となり、10%以上の後退は再現しなかった。

[14本のraw](../runs/0064-kernel/manifest.json)、[正しいplan IDでの集計](../runs/0064-kernel-analysis/result.json)、[再確認](../runs/0064-kernel-confirmation-summary/result.json)を保存した。初回のsummaryは旧helperの既定0063を付けたため保持し、helperへplan引数を追加して新規summaryに再集計した。時間のrawは編集していない。

## Redと検証

Rustの未実装method、nativeの未知request、TSの未exportを先にRedとして記録した。途中のfixture必須field不足、serdeのderive不足、TSの推論境界もlogへ残す。Rustでは反復・予算変更・実行エラー後・coverage・複数caller・未選択の不正Dataを検査した。

Wasmではモデル・時間論理の既存cohortをv1/v2で反復し、batch途中のerrorと後続結果、snapshot所有、release、finite handleとの種別違いを確認した。実Chromiumの4検査と単独TS/Rust consumerへ準備済みAPIを追加した。[21検査](../runs/0064-verification/result.json)はすべて通り、検証中のcrate sourceは不変だった。analysisは4,408件、safetyは1,185件で、fmt/clippy、runtime、評価と反例再生、3言語比較、package consumerを含む。

[旧版との156モデル・1,560要求](../runs/0064-parity/result.json)もnative/Wasmの完全応答で照合する。Jevはwire契約と準備済みAPIの根拠を分け、47比較で矛盾・未評価0だった。既存TS対象のmutation検査は109件をcacheから再利用したもので、今回のRust準備処理のmutation評価とは区別する。

小さいqueryの準備・wire費用を削る機能として採用し、TLA+の対応範囲や実装conformanceの保証を拡大したとは扱わない。次はSYMMETRYのrow/key構築の割り当てを独立して評価する。
