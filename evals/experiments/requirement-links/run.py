"""Exercise three requirements link designs against the same rmodel model."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

MODEL = Path("fixtures/rcheck/structured/shipped-transition.model")
CASES = Path("fixtures/rcheck/requirement-links/cases.json")
RCHECK = Path("crates/rcheck/target/debug/rcheck")
PARTS = ("status", "tracking")


def run(*args):
    return json.loads(subprocess.check_output([RCHECK, *args], text=True))


def fingerprint(value):
    canonical = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(canonical.encode()).hexdigest()


def replace(source, before, after):
    assert source.count(before) == 1, before
    return source.replace(before, after)


def find_claim(ir, name):
    return next((entry["formula"] for entry in ir["transitions"] if entry["name"] == name), None)


def split_claim(formula):
    # Project the existing shipment formula. The two clause checks
    # together are equivalent to the original implication.
    if formula is None:
        return {}
    [implication] = formula["args"]
    assert implication["op"] == "implies"
    [status, *comparisons] = implication["right"]["args"]
    assert status["op"] == "has_tag" and len(comparisons) <= 1
    results = {"status": {**formula, "args": [{**implication, "right": status}]}}
    if comparisons:
        [comparison] = comparisons
        assert comparison["op"] == "eq"
        # Read the payload only when both states are shipped.
        tracking = {
            **implication,
            "left": {"op": "and", "args": [implication["left"], status]},
            "right": comparison,
        }
        results["tracking"] = {**formula, "args": [tracking]}
    return results


def describe(source, prose, name, directory):
    model_file = directory / "shipment.model"
    model_file.write_text(source)
    ir = run("generate", model_file, "ir")
    formula = find_claim(ir, name)
    parts = split_claim(formula)

    probe_ir = copy.deepcopy(ir)
    probe_ir["transitions"].extend(
        {"name": f"Clause_{part}", "formula": part_formula}
        for part, part_formula in parts.items()
    )
    probe_file = directory / "shipment.json"
    probe_file.write_text(json.dumps(probe_ir))
    reports = run(probe_file, "--checker", "reference")
    verdicts = {}
    for report in reports:
        answer, = report["answers"]
        verdicts[report["question"]] = answer["verdict"], answer.get("depth")
    assert verdicts["WasShipped"][0] == "reachable", verdicts
    return {
        "model": fingerprint(ir),
        "claim": None if formula is None else fingerprint([prose, formula]),
        "parts": {
            part: fingerprint([prose["applicability"], prose[part], part_formula])
            for part, part_formula in parts.items()
        },
        "verdicts": verdicts,
    }


def review(current, reviewed):
    if current is None:
        return "unresolved"
    return "reviewed" if current == reviewed else "needs-review"


def compare(current, approved, check):
    previous_evidence = "current" if current["model"] == approved["model"] else "outdated"
    verdicts = current["verdicts"]
    return {
        "link": "resolved" if current["claim"] is not None else "unresolved",
        "wholeReview": review(current["claim"], approved["claim"]),
        "clauses": {
            part: review(current["parts"].get(part), approved["parts"][part]) for part in PARTS
        },
        "previousModelEvidence": previous_evidence,
        "checked": verdicts.get(check),
        "checkedClauses": {part: verdicts.get(f"Clause_{part}") for part in PARTS},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    output = parser.parse_args().output

    source = MODEL.read_text()
    fixture = json.loads(CASES.read_text())
    prose = {name: fixture[name] for name in ("applicability", *PARTS)}
    name = fixture["check"]
    no_tracking = replace(
        source, "    payload(before(status), Shipped) = payload(after(status), Shipped)\n", ""
    )
    overwrites = replace(
        source, "acknowledged = true }", "acknowledged = true status = Shipped(Post) }"
    )
    cases = [
        ("original", source, prose),
        ("paraphrase", source, {**prose, "tracking": fixture["paraphrase"]}),
        ("permission", source, {**prose, "tracking": fixture["allowChange"]}),
        ("weaker-property", no_tracking, prose),
        ("wrong-update", overwrites, prose),
        ("broken-link", replace(source, name, "RenamedTransition"), prose),
        ("comment-only", source + "\n// Documentation-only change.\n", prose),
    ]
    with tempfile.TemporaryDirectory() as directory:
        directory = Path(directory)
        approved = describe(source, prose, name, directory)
        results = {
            case: compare(describe(version, text, name, directory), approved, name)
            for case, version, text in cases
        }
    assert results["original"] == results["comment-only"]
    assert results["paraphrase"] == results["permission"]
    for case in ["original", "paraphrase", "permission", "wrong-update"]:
        assert results[case]["link"] == "resolved"
    assert results["original"]["wholeReview"] == "reviewed"
    assert results["permission"]["wholeReview"] == "needs-review"
    assert results["permission"]["clauses"] == {"status": "reviewed", "tracking": "needs-review"}
    assert results["permission"]["previousModelEvidence"] == "current"
    assert results["permission"]["checked"] == ("holds", None)
    assert results["weaker-property"]["wholeReview"] == "needs-review"
    assert results["weaker-property"]["clauses"] == {"status": "reviewed", "tracking": "unresolved"}
    assert results["weaker-property"]["checked"] == ("holds", None)
    assert results["wrong-update"]["previousModelEvidence"] == "outdated"
    assert results["wrong-update"]["wholeReview"] == "reviewed"
    assert results["wrong-update"]["checked"] == ("violated", 2)
    assert results["wrong-update"]["checkedClauses"] == {"status": ("holds", None), "tracking": ("violated", 2)}
    assert results["broken-link"]["link"] == "unresolved"
    assert results["broken-link"]["checked"] is None
    with output.open("x") as stream:
        json.dump(results, stream, indent=2, ensure_ascii=False)
        stream.write("\n")


if __name__ == "__main__":
    main()
