# 0075-abba-complete-scope

[0075-abba-complete](../0075-abba-complete/) の summary は、full の 10% 以上の後退 17 件を「再現せず、load の高い測定の揺れ」と判断した。17 件には `frontend-check_fold-recursive`（+11.84%）が含まれるが、repeat と null-control は `nesting_chains` と `recursive_actions` の 2 本だけで、`frontend` の bench は流し直していない。この記録は、その判断の範囲を限定する。0075-abba-complete の記録は変えていない。

## 範囲

- 17 件の内訳は、`nesting_chains` 8 件、`recursive_actions` 8 件、`frontend` 1 件。揺れと判断した根拠があるのは前の 16 件。repeat（33 関数、10% 以上 0 件、最大 +8.7%）と null-control（33 関数、10% 以上 0 件、最大 +5.3%）が、この 2 本の再現しなさを示す。
- `frontend-check_fold-recursive` は repeat の対象外。full の 1 回の測定（before 17417、after 19479、+11.84%）が残るだけで、再現するかどうかは確かめていない。同じ bench の `frontend-compile_fold-recursive`（-0.62%）と `frontend-parse_fold-recursive`（-0.21%）は後退していない。
- full の表を最終の性能の数値に使わない点は変わらない。この関数について、揺れとも変更による後退とも結論しない。未確認として残す。
- 0075-abba-3 の結論（修正後に 10% 以上の再現する後退なし）は、abba-3 が測った範囲のものとして変わらない。

## 次の確認

`frontend` の bench を、同じ before / after の binary で repeat と null-control の条件のとおりに流し直す。この記録の時点では流していない。
