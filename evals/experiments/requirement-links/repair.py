"""Probe candidate repairs without weakening the requirement-side witnesses."""
import argparse
import json
from pathlib import Path
import tempfile

from run import MODEL, replace, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    output = parser.parse_args().output

    source = MODEL.read_text()
    witnesses = "".join(
        f"""
reachable {carrier}Acknowledged {{
  is(status, Shipped)
  payload(status, Shipped) = {carrier}
  acknowledged = true
}}
"""
        for carrier in ("Courier", "Post")
    )
    source = replace(source, "scope steps = 4", witnesses + "scope steps = 4")
    faulty = replace(
        source, "acknowledged = true }", "acknowledged = true status = Shipped(Post) }"
    )
    original_guard = "guard { is(status, Shipped) }"
    examples = [
        ("normal", source, ["holds", "reachable", "reachable"]),
        ("overwrite", faulty, ["violated", "unreachable", "reachable"]),
        ("disable-all", replace(faulty, original_guard, "guard { false }"),
         ["holds", "unreachable", "unreachable"]),
        ("disable-courier", replace(
            faulty, original_guard,
            "guard { is(status, Shipped) payload(status, Shipped) = Post }",
        ), ["holds", "unreachable", "reachable"]),
        ("remove-overwrite", replace(faulty, "true status = Shipped(Post)", "true"),
         ["holds", "reachable", "reachable"]),
    ]
    questions = ("ShippedStays", "CourierAcknowledged", "PostAcknowledged")
    observations = {}
    with tempfile.TemporaryDirectory() as directory:
        model = Path(directory) / "shipment.model"
        for name, version, expected in examples:
            model.write_text(version)
            reports = run(model, "--checker", "reference")
            answers = {row["question"]: row["answers"][0] for row in reports}
            actual = {question: answers[question]["verdict"] for question in questions}
            assert list(actual.values()) == expected, (name, actual)
            observations[name] = actual
    with output.open("x") as stream:
        json.dump(observations, stream, indent=2)
        stream.write("\n")


if __name__ == "__main__":
    main()
