# 0061: 限定symmetryの次の改善

基準は62eac486。Rust nativeを主指標にし、Wasmは移植性と挙動を検査する。各段階を順に実装・評価し、後退する案は保存して採用を見送る。既存runは編集しない。pushとmergeは行わない。

## 順序と仮説

0. 0060の通常ringの変動を診断する。凍結したbefore/final binaryを同じfilterでbefore/after/after/before、さらに逆順で測る。Criterionは20 samples、warmup 0.5秒、measurement 1秒。buildやtestと同時に測らない。持続する差があればprofileと原因の修正を先に行う。
1. node参照のないrecord payloadとnode部分集合を状態として扱う。各nodeのrecord全体と各集合の所属bitを同じ行に含める。recordのschema・型とglobal値をkeyへ含め、相関を失わない。node参照、任意のnested構造、複数群は対象外。
2. 単一node bindingの操作で、現在状態の同じ行に属するnodeを代表だけ展開する試作を行う。初期状態生成へ適用せず、個別node・順序依存式を拒否する既存gateと利用者の対称性前提を維持する。複数bindingは通常展開へ戻す。具体trace、deadlock、予算を検査する。generated/workが変わる場合は明記する。
3. symmetryでも既存の並列frontier展開を試す。workerの値をrootへ移送してから逐次にcanonical keyを作る。狭いfrontierは逐次へ戻す。小さい縮約グラフと大きい縮約グラフを比較する。
4. 式ごとのHashMap検索を避ける命令IDの直接実行を試す。scalarのread/literal、算術・比較を最小対象とし、短絡・overflow・workの挙動を維持する。既存評価器へのfallbackを保ち、通常counter/ringで有用な短縮がなければ採用しない。
5. safetyに限定した部分順序削減を試す。独立性・propertyへの可視性・cycle条件を保守的に判定する。縮約できない場合は完全探索へ戻す。誤検出・見逃しがあれば採用せず、検査した境界と次の条件を記録する。

## 正しさの対照

Redを先に実行し、logを保存する。recordはN2/3とInt(0..2)・Bool field、集合はN2/3と2集合・関数との組み合わせを固定する。安全版と到達後に破れるinvariantを比較し、TLCで縮約あり/なしの判定を確認する。縮約なしのNextによる具体反例の再生、native/Wasmの照合、hash衝突・型・schema・相関・拒否条件、work/state/depth予算を検査する。

各段階の候補はsourceとbinary hashで固定する。counter N5/K3、N7/K5と2 Bool関数N5を既存対照にし、record N5/K2と集合+Bool関数N5を追加する。並列は同じ候補の1/8 worker要求を比較する。追加の大きい入力や条件は実行前にこのplanへ追記する。起動込みとpreparedを区別し、計数が異なる縮約のworkやgeneratedを同一の仕事量とは扱わない。

## 採否と検証

TLCへの判定・利用可能な計数の一致、反例再生、既存回帰を前提にする。速度の採用は同じホストで順序を入れ替えた測定から判断し、10%以上の後退を明記する。record/集合は対応範囲の追加価値も評価する。実際の改善がない試作は撤去し、試作source・失敗log・結果を保存する。

終了前にcheck/browser、portable-build/test、safety-test、runtime-build/test、fmt/clippy、evaluate/replay-evaluationを実行する。共有TSテストの変更があればmutation、配布変更があればpackageも実行する。SUPPORT/Jevのliveとofflineを確認し、キーなしのskipと検査済みを区別する。各段階の採否、未解決の制限、追加測定の条件をiterationへ記録する。

## 段階3の追加入力

段階1/2の小さいquotientに加え、recordのN12を使う。各nodeはInt(0..2)とBoolの6状態、2操作を持つ。縮約後は6,188状態、全node展開では148,513生成、代表展開では52,417生成を期待する。縮約なしの6^12状態はこの比較で探索しない。同じbinaryの1/8 worker、代表展開on/offを順序を入れ替えて測る。symmetry時の並列閾値はfrontier 64状態とし、閾値未満は逐次へ戻す。改善がない場合は段階3を撤去して、段階2の明示的な逐次経路を維持する。

## 段階4の実行計画

命令列はguard/assignのみのbranchを対象にする。literal/read、Intの加減乗算・比較、Boolのnot/and/or/impliesとif、等値比較を扱い、未対応式はbranch全体を既存評価器へ戻す。型確認の順序、短絡、overflow、next変数の読み取り、代入の巻き戻し、workを比較する。命令IDはVecの添字で、hot pathで式addressのHashMap検索をしない。既存bench-fixturesのcounter-product/linear-ringを同じbinaryのon/offで20 samples、warmup 0.5秒、measurement 1秒、順序を反転して比較する。1 workerを使う。準備費用を含め、速度が改善しない場合は試作sourceを保存して撤去する。

初回の段階4 driverはsymmetry用のmaxDepth=1000でringを実行し、boundedを検出して停止した。失敗runを残し、既存の通常benchと同じmaxDepth=100000へ変更して全4ケースを再測定する。途中までのcounter sampleだけで採否を決めない。

## 段階5の適用条件

PORは、他の操作・invariantから観測されないBool flagをFALSEからTRUEへ一度だけ変える独立な操作に限定する。候補は `~flag /\ flag' = TRUE /\ UNCHANGED otherVariables` の形。別branchがflagを読み書きする場合と、未解析の式がある場合は候補から外す。単なるUNCHANGEDは書き込み依存に含めない。選ぶたびに未設定flagが減り、他の操作がflagを戻せないことをcycle条件にする。有限scalarのinvariant/deadlockだけを対象にし、SYMMETRYとの併用は拒否する。temporal/fairness・constraint・initial/action propertyは通常探索へ戻す。

N2/3の独立flagとvisible counter(0..3)について安全版、visible counter<3という故障版、deadlockを比較する。依存するguard・可視のflag・flagを戻す操作を拒否対照にし、通常探索へのfallbackを確認する。反例は元のNextで再生する。POR時は元のグラフでの最短反例を保証しない。generated/states/workは削減後の計数であり、TLCとの計数一致を求めない。

性能はN10のflagと4状態のvisible counterを使う。通常は4,096状態・24,577生成、PORは14状態・15生成を期待する。同じbinaryのon/offを20 samples、warmup 0.5秒、measurement 1秒、順序反転で測る。これは限定した条件での実験で、一般のPORへ保証を広げない。
