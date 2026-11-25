"""Compare just the shipment requirement, with paraphrase and changed-intent controls."""
import argparse
import json
import os
from pathlib import Path
import subprocess

MANIFEST = Path("spec/rmodel-design-documents.json")
CONTROLS = Path("fixtures/rcheck/shipment/prose-controls.md")
EXPECTED = {"baseline": "supports", "paraphrase": "supports", "allow-change": "contradicts"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    output = args.output
    output.mkdir(parents=True)

    manifest = json.loads(MANIFEST.read_text())
    baseline, = [
        comparison for comparison in manifest["comparisons"]
        if comparison["target"].get("heading") == "SHIPMENT-TRACKING"
    ]
    requirement, = [
        rule for rule in json.loads(Path("spec/rmodel-design.json").read_text())["spec"]["rules"]
        if rule["id"] == "SHIPMENT-TRACKING"
    ]
    files = {
        name: manifest["files"][name]
        for name in [baseline["reference"]["source"], baseline["target"]["source"]]
    }
    files["controls"] = {"path": str(CONTROLS), "role": "reference"}
    # Select only the prose. The generated claim and its expected verdict
    # must not be visible on the target side of the comparison.
    comparisons = [
        {**baseline, "id": name, "target": {**selector, "contains": text}}
        for name, selector, text in [
            ("baseline", baseline["target"], requirement["text"]["ja"]),
            ("paraphrase", {"heading": "SHIPMENT-TRACKING-PARAPHRASE", "source": "controls"}, "発送済み"),
            ("allow-change", {"heading": "SHIPMENT-TRACKING-ALLOW-CHANGE", "source": "controls"}, "発送済み"),
        ]
    ]
    focused = output / "manifest.json"
    focused.write_text(json.dumps({
        **manifest, "files": files, "roots": [file["path"] for file in files.values()],
        "comparisons": comparisons,
    }, indent=2, ensure_ascii=False) + "\n")

    environment = os.environ.copy()
    environment.pop("OPENROUTER_API_KEY", None)
    if not environment.get("TYPESAFE_API_KEY") and environment.get("TYPESAFE_API_KEy"):
        environment["TYPESAFE_API_KEY"] = environment["TYPESAFE_API_KEy"]
    has_key = bool(environment.get("TYPESAFE_API_KEY") or environment.get("TYPESAFEAI_API_KEY"))
    ledger = output / "ledger"
    result = subprocess.run([
        "pnpm", "exec", "jev-lint", "docs", "--model", "jev-1.14.0",
        "--document-manifest", str(focused), "--ledger", str(ledger), "--json",
        *([] if has_key else ["--dry-run"]),
    ], env=environment, capture_output=True, text=True, timeout=150)
    (output / "jev.json").write_text(result.stdout)
    (output / "jev.stderr").write_text(result.stderr)

    if not has_key:
        plan = json.loads(result.stdout)["input"]
        prepared = plan["comparisons"]
        assert not plan["issues"] and not plan["deferred"], plan
        assert {item["id"] for item in prepared} == EXPECTED.keys(), plan
        assert len({item["reference"]["contentHash"] for item in prepared}) == 1, plan
        assert len({item["target"]["contentHash"] for item in prepared}) == len(EXPECTED), plan
        assert prepared[0]["target"]["quote"] == requirement["text"]["ja"], plan

    answers = {}
    for file in sorted(ledger.glob("*.jsonl")):
        for line in file.read_text().splitlines()[1:]:
            value = json.loads(line)["value"]
            judgment = value.get("judgment")
            if judgment:
                answers[value["comparisonId"]] = judgment
    choices = {name: answer["choice"] for name, answer in answers.items()}
    missing = EXPECTED.keys() - choices.keys()
    status = "unmeasured" if missing else "matched" if choices == EXPECTED else "mismatch"
    summary = {
        "status": status,
        "reason": (
            "missing-typesafe-api-key" if not has_key else
            "missing-judgments" if missing else None
        ),
        "expected": EXPECTED, "answers": answers, "exitCode": result.returncode,
    }
    (output / "result.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary))
    return {"matched": 0, "mismatch": 1, "unmeasured": 3}[status]


if __name__ == "__main__":
    raise SystemExit(main())
