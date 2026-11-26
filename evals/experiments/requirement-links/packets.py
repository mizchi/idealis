"""Render lookup tasks with a different case/design order per reader."""
import json
from pathlib import Path
import sys

SCHEDULE = [
    [("B", "permission"), ("A", "weaker-property"), ("C", "wrong-update")],
    [("C", "weaker-property"), ("B", "wrong-update"), ("A", "permission")],
    [("A", "wrong-update"), ("C", "permission"), ("B", "weaker-property")],
]
LABEL = {
    "resolved": "解決済み",
    "unresolved": "参照先なし",
    "reviewed": "確認済み",
    "needs-review": "要再確認",
    "current": "現行版",
    "outdated": "旧版",
    "holds": "成立",
    "violated": "違反",
}
QUESTIONS = """1. 要件から性質全体へのリンクは有効か。
2. 現在のモデル検査は何を示したか。各節の結果があれば、その範囲と最初の違反操作も答える。
3. 保存済みの自然文と式の対応レビューは利用できるか。再確認の対象範囲を答える。画面に根拠がなければ「この資料から不明」と答える。
4. 最初に見た行を一つ、読みづらかった箇所を一つ挙げる。後者はなければ「なし」。
"""


def result(value):
    if value is None:
        return "判定なし"
    verdict, depth = value
    return LABEL[verdict] + (f"（最短 {depth} 操作）" if depth is not None else "")


def card(fixture, rows, design, case, index):
    report = rows[case]
    prose = [
        fixture["applicability"],
        fixture["status"],
        fixture["allowChange"] if case == "permission" else fixture["tracking"],
    ]
    formula = ["発送済み → 各操作の直後も発送済み"]
    if case != "weaker-property":
        formula.append("発送済み前後で追跡方法が等しい（後状態も発送済みの場合）")
    change = {
        "permission": "自然文の末尾を編集した。",
        "weaker-property": "性質の本体を編集した。",
        "wrong-update": "確認操作を編集した。",
    }[case]
    lines = [
        f"## 画面 {index}",
        f"変更履歴　{change}",
        f"{fixture['rule']} → {fixture['check']}（{LABEL[report['link']]}）",
        f"適用範囲　{prose[0]}",
    ]
    if design != "C":
        lines.extend([f"要件　{' '.join(prose[1:])}", f"式　{' ／ '.join(formula)}"])
    lines.append(f"現在のモデル検査　{result(report['checked'])}")
    if report["checked"][0] == "violated":
        lines.append("違反を再現する列　発送（Courier）→ 確認（Post に変化）")
    if design != "A":
        lines.append(f"保存済みのモデル検査　{LABEL[report['previousModelEvidence']]}")
    if design == "B":
        lines.append(f"要件全体の対応レビュー　{LABEL[report['wholeReview']]}")
    if design == "C":
        lines.extend([
            "",
            "| 要件の節 | 式 | 現在の検査 | 対応レビュー |",
            "| --- | --- | --- | --- |",
        ])
        for offset, part in enumerate(["status", "tracking"], 1):
            target = formula[offset - 1] if len(formula) >= offset else "参照先なし"
            lines.append(
                f"| {prose[offset]} | {target} | {result(report['checkedClauses'][part])} | "
                f"{LABEL[report['clauses'][part]]} |"
            )
    return "\n\n" + "\n".join(lines) + "\n\n" + QUESTIONS


def main():
    rows = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    output = Path(sys.argv[2])
    output.mkdir()
    fixture = json.loads(Path("fixtures/rcheck/requirement-links/cases.json").read_text(encoding="utf-8"))
    for reader, cases in enumerate(SCHEDULE, 1):
        packet = "# 発送の仕様レビュー\n\n各画面は独立した変更例です。"
        for index, (design, case) in enumerate(cases, 1):
            packet += card(fixture, rows, design, case, index)
        (output / f"{reader}.md").write_text(packet + "\n最後に、最も調査しやすかった画面番号とその理由を一つ答える。\n", encoding="utf-8")


if __name__ == "__main__":
    main()
