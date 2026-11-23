# 0079: ralloyの関係をn項へ一般化する

[0078](0078-ralloy-alloy-oracle.md)の次の段階。今のralloyは、関係を単項（`u64`のbit集合）と二項（行ごとの`u64`）だけで表す。fieldの型は`A`だけを書ける。

## 変更

- 3項以上の関係を、atom番号の列（tuple）の集合で表す。単項と二項の表現と演算は今のまま残し、3項以上と混ざる演算だけをtupleの集合で計算する。結果が二項以下なら元の表現へ戻す。
- fieldの型に`A -> B`、`A -> B -> C`と、矢印の多重度`A m -> n B`（m, nはset/one/lone/some、既定はset）を書ける。fieldの全体の項数は4以下にする。
- 探索では、ownerのatomごとに、残りの列の積（64 tuple以下。超えればresource error）の部分集合をbit maskで選び、矢印の多重度を満たすものだけを残す。
- 和・差・交差・積・join・domain/range制限・等価・包含・`no/some/one/lone`をn項で扱う。転置と閉包は二項だけ（型検査で拒否する）。
- `Instance.fields`を`Vec<[String; 2]>`から`Vec<Vec<String>>`へ変える。二項のJSONの形は変わらない。Rust APIの型は変わる（alpha版）。

## 正しさ（結果を見る前に固定）

- group `nary`（5モデル・25 command、記録済み）をAlloyの答えへ再生して全一致すること。
- group `core`（0078）の再生が引き続き全一致すること。
- 既存のRust・TS・Wasmのテストが通ること。workの数え方が変わる場合は、変わったことと理由を記録する。
- 人工故障: n項のjoinで結合する列を間違える、tuple空間の積から1列落とす、矢印の右の多重度を検査しない、の3件を再生テストが検出すること。

## 速度（結果を見る前に固定）

`benches/oracle.rs`の`core`の13ケースを、0078の版（baseline）とこの版（candidate）で、Criterionで交互の順に測る（20 samples、warmup 1秒、measurement 2秒、median、95%区間）。baseline→candidate、candidate→baselineの4 runを`0079-*`へ保存する。

- 13ケースすべてで、両組のcandidate/baselineの比が1.1以下。二項以下の経路を変えないことの確認。

超えたケースは後退として記録し、閾値を変えない。再測定は追記planで1組だけ宣言できる。`nary`の速度は測るが、比べる旧版がないので報告だけにする。
