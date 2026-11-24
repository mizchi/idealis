# 0086への追記: 1組の再測定

[0086の比較](../runs/0086-ralloy-comparison/result.json)は`hold`になった。開発組で`core-iff-implies` 1.197、`core-sig-multiplicity` 1.132、`core-set-algebra` 1.123、`core-multiplicity` 1.116が閾値を超えた。確認組では、13ケースすべてが0.853〜0.989だった。`core`はすべて総当たりのままで、0086は総当たりの経路を変えていない。測定中の負荷平均は5.5前後で、直後には8.5まで上がった（Chrome、IME、WindowServerなど）。

測定の途中で、外部の`rustup update`によりrustcが1.100.0へ更新された。比較にコンパイラの違いが混ざらないよう、0086では結果を見る前に、baselineを0085の版（f1b2c20f7）からrustc 1.100.0でビルドし直したbinaryに変えた。0086のplanにはこの変更と再測定の規定がなかったので、ここに記録する。

再測定は1組だけ行う。binaryは0086と同じもの（rebuildしたbaselineと0086のcandidate）を使い、閾値も0086と同じ（13ケースすべてで両組1.1以下）。結果は`0086a-*`へ保存し、0086のrunとは組み合わせない。超えたケースは後退として記録し、さらに再測定しない。
