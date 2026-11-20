# roracle の応答生成とデコード

基準は typed-oracle の 54f8a703e と、作業ツリーにある frozen worker の修正。既存の oracle-native-eval / oracle-native-replay を使い、変更前、TS デコーダのみ、TS デコーダと Rust の直接出力を順に測る。各段階を新しいディレクトリに保存する。

- 既存6入力、Wasm / native JSON / native object、100 warmup、各30 samples。入力別の反復数と経路の開始順回転は既存測定器を使う。ビルドや検査は測定と並走させない。
- 主比較は batch-32 と nested-256 の全 request の中央値。入力、全 report、失敗の扱いを維持する。3段階の fixture と期待 report の一致、各段階の再生を確認する。
- TS は検証後の report をそのまま返す。Rust は native object の出力から中間 Value を除く。構造上のコピー削減と速度の実測を区別し、順次測定の小差を高速化と判定しない。既定 backend はこの比較の対象外。
- ハンドル化は既存の呼び出しで同じ期待値を何度使うかを調べる。新しい寿命管理や可変参照のキャッシュは今回導入しない。
