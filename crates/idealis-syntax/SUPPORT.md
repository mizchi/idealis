# idealis-syntax の対応範囲

- [x] Idealis のモデルソースを読み、名前・型・式を確認して検査済みモデルにする。[構文と型検査](src/syntax.rs) · [単独利用の試験](tests/source.rs)
- [x] ソース位置、用語 ID の束縛、更新分岐のソース情報を保持する。外部で構築したモデルも同じ型検査に通せる。[公開 API](src/lib.rs)
- [x] 用語辞書の構文と用語間の参照を検査する。[辞書構文](src/syntax/vocabulary.rs)

- [ ] 検査器への変換とホスト言語の型生成。[アダプター](../idealis-model/SUPPORT.md)
