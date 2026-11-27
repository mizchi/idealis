"""Measure the dense workload with alternating AB and BA pairs."""

import argparse
import hashlib
import json
import platform
import shlex
import statistics
import subprocess
from pathlib import Path

from bench import BENCHMARK, EDGES

METRICS = ("time_wall_clock", "instructions", "memory_peak_resident")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    output = args.output
    output.mkdir(parents=True)
    baseline = args.baseline.resolve()
    candidate = BENCHMARK.resolve()
    if baseline == candidate:
        parser.error("baseline must be a separate build")
    model = output / "edges-4x4.model"
    model.write_text(EDGES.read_text(encoding="utf-8").replace("Left = 3", "Left = 4", 1), encoding="utf-8")
    argv = {key: [str(binary), "rprob", "prepared", str(model), "3"]
            for key, binary in (("A", baseline), ("B", candidate))}
    report = subprocess.check_output(argv["A"])
    if report != subprocess.check_output(argv["B"]):
        raise SystemExit("before and after reports differ")
    (output / "report.txt").write_bytes(report)

    raw = output / "hyperfine.json"
    order = ["AB", "BA"] * 4
    command = ["hyperfine", "--runs", "1", "--metrics", ",".join(METRICS), "--export-json", str(raw)]
    names = []
    for pair, sequence in enumerate(order, start=1):
        for side in sequence:
            name = f"{pair}-{side}"
            names.append(name)
            command += ["--command-name", name, shlex.join(argv[side])]
    subprocess.run(command, check=True)
    results = json.loads(raw.read_text(encoding="utf-8"))["results"]
    if [result["name"] for result in results] != names:
        raise SystemExit("Hyperfine returned an unexpected command order")
    values = {result["name"]: result["measurements"][0] for result in results}
    summary = {}
    for metric in METRICS:
        by_side = {side: [sample[metric]["value"] for name, sample in values.items() if name.endswith(side)]
                   for side in argv}
        summary[metric] = {side: {"min": min(samples), "median": statistics.median(samples), "max": max(samples)}
                           for side, samples in by_side.items()}
        summary[metric]["before_over_after_by_order"] = {
            sequence: statistics.median(
                values[f"{pair}-A"][metric]["value"] / values[f"{pair}-B"][metric]["value"]
                for pair, executed in enumerate(order, start=1) if executed == sequence
            )
            for sequence in ("AB", "BA")
        }
    record = {
        "host": platform.platform(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "samples_per_command": len(order),
        "order": order,
        "iterations_per_command": 3,
        "sha256": {key: sha256(path) for key, path in (
            ("before", baseline), ("after", candidate), ("model", model), ("raw", raw)
        )},
        "summary": summary,
    }
    (output / "summary.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
