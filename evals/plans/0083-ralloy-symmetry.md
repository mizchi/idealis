# 0083: ralloyの探索の高速化（対称性の除去）

[0078](0078-ralloy-alloy-oracle.md)の6段目。0079〜0082で機能を広げる間、二項以下のcaseに小さな固定費が加わり（0079・0080は`hold`）、最も重い`core-set-algebra`は約120 msのままだった。`nary`の`CompositionHolds`は、候補が約1.3億個で予算切れになる。

実装前の探索profile（0082のcandidate、`core-set-algebra`）では、時間の98%が式の評価で、列挙（`assign`）は約2%だった。この数値は探索用で、採否には使わない。

## 変更

1. 対称性の除去（lex-leader）: すべてのsig（subset sigを含む）について所属が同じで、Int atomでないatomの組は、入れ替えても式の真偽を変えない。式はatomをsigとfieldを通してしか参照しないため。同じ所属の隣り合うatomの互換ごとに、fieldの全行を入れ替えた構造（ownerの行と、各行の中のbitの両方を入れ替える）と比べる。元の構造が辞書順で大きい候補は評価しない。各軌道の辞書順で最小の代表は、すべての互換の条件を満たすので残る。解のある・なしは変わらない。factを満たす構造があれば代表もfactを満たすので、`no-instance`と`no-counterexample`の区別も変わらない。2列以上のfieldがあるcommandでは除去しない（tuple空間の置換を扱わない）。
2. 階層の順序（`check::hierarchy`）を`Model::parse`で1回だけ計算し、解析ごとに作り直さない。0080で残った小さいcaseの固定費への対処。

返すinstanceと、`stats`の`candidates`・`factInstances`・`work`は変わる。statusは変えない。

## 正しさ（結果を見る前に固定）

- 5 groupの再生が0082と同じ状態であること（`nary`の1件だけが予算切れ）。`CompositionHolds`が解けた場合は、pendingの一覧を更新し、その答えがAlloyと一致すること。
- 人工故障: 互換の判定でsubset sigの所属を無視する、Int atomも入れ替えの対象にする、行の中のbitを入れ替えずにownerの行だけを入れ替える、の3件を再生テストが検出すること。

## 速度（結果を見る前に固定）

0082のcandidateの保存binaryをbaselineとして、`core`の13ケースを交互の順に測る。

- 主指標: `core-set-algebra`は両組でcandidate/baselineの比0.5以下。
- 後退: 残る12ケースは両組で1.1以下。

超えたケースは後退として記録し、閾値を変えない。再測定は追記planで1組だけ宣言できる。
