"""Pin an rprob benchmark run and emit versioned adoption goal evidence."""
import argparse
import hashlib
import json
import platform
import shlex
import statistics
import subprocess
from pathlib import Path

from bench import BENCHMARK, EDGES, command_args, measure

POLICY = {
    "workload": "edges-3x4",
    "mode": "prepared",
    "loops": 4,
    "runs": 5,
    "warmup": 1,
    "metric": "time_wall_clock",
    "max_sample_range_fraction": 0.20,
    "minimum_difference_fraction": 0.10,
}
NAME = "goal"
RECEIPT = "measurement.json"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write(path, data):
    with path.open("x", encoding="utf-8") as file:
        json.dump(data, file, indent=2, ensure_ascii=False)
        file.write("\n")


def classify(reference, candidate):
    for group in (reference, candidate):
        if len(group) != POLICY["runs"] or not all(
            type(value) in (int, float) and 0 < value < float('inf')
            for value in group
        ):
            return "unmeasured"
        if max(group) - min(group) > POLICY["max_sample_range_fraction"] * statistics.median(group):
            return "unmeasured"
    difference = POLICY["minimum_difference_fraction"]
    if max(candidate) <= min(reference) * (1 - difference):
        return "improved"
    if min(candidate) >= max(reference) * (1 + difference):
        return "regressed"
    return "no-gain"


def commands():
    mode = POLICY["mode"]
    return {
        f"{engine}-{mode}": shlex.join(command_args(engine, mode, EDGES, POLICY["loops"]))
        for engine in ("reference", "rprob")
    }


def score(plan, receipt, measurements, checked_output):
    evidence = {"revision": plan["revision"], "outcome": "unmeasured"}

    if (
        plan.get("schema") != "idealis-rprob-goal-v1"
        or plan.get("policy") != POLICY
        or plan["revision"].get("candidate") != f"sha256:{plan.get('benchmark_sha256')}"
        or receipt.get("plan") != plan
        or receipt.get("measurements_sha256") != digest(measurements)
        or receipt.get("checked_output_sha256") != digest(checked_output)
    ):
        return {**evidence, "reason": "plan-or-artifact-mismatch"}
    # These are the named workload's expected verdicts, fixed before
    # measuring either implementation. bench.measure also compares outputs.
    if not checked_output.startswith(b"scope Holds, declarations Holds\n") or (
        b'check: "Full", outcome: Reachable { depth: 12 }' not in checked_output
    ):
        return {**evidence, "reason": "invalid-workload-verdict"}

    rows = []
    try:
        document = json.loads(measurements)
        metadata = document["metadata"]
        if (
            document["schema_version"] != 2
            or document["primary_metric"] != POLICY["metric"]
            or metadata["platform"] != plan["platform"]
            or metadata["hyperfine_version"] != plan["hyperfine_version"]
        ):
            return {**evidence, "reason": "measurement-format-or-platform-mismatch"}
        expected = commands()
        results = document["results"]
        if not isinstance(results, list) or len(results) != len(expected):
            return {**evidence, "reason": "missing-or-extra-results"}
        for (name, command), result in zip(expected.items(), results):
            if result["name"] != name or result["command"] != command:
                return {**evidence, "reason": "command-mismatch"}
            samples = []
            for sample in result["measurements"]:
                metric = sample[POLICY["metric"]]
                if type(sample["exit_code"]) is not int or sample["exit_code"] != 0 or metric["unit"] != "second":
                    return {**evidence, "reason": "failed-or-invalid-sample"}
                samples.append(metric["value"])
            rows.append(samples)
    except (KeyError, ValueError, TypeError):
        return {**evidence, "reason": "invalid-measurements"}
    outcome = classify(*rows)
    return {**evidence, "outcome": outcome, **({"reason": "missing-or-noisy-samples"} if outcome == "unmeasured" else {})}


def inputs():
    return {
        "benchmark_sha256": digest(BENCHMARK.read_bytes()),
        "model_sha256": digest(EDGES.read_bytes()),
        "platform": {"os": platform.system(), "architecture": platform.machine()},
        "hyperfine_version": subprocess.check_output(["hyperfine", "--version"], text=True).strip().split()[-1],
    }


def record(output, revision):
    if set(revision) != {"candidate", "rule", "evaluation_revision"} or not all(
        isinstance(value, str) and value for value in revision.values()
    ):
        raise ValueError("expected an evaluation revision from adoption-evidence --pin")
    initial_inputs = inputs()
    if revision["candidate"] != f"sha256:{initial_inputs['benchmark_sha256']}":
        raise ValueError("the candidate must identify the current benchmark executable by SHA-256")
    plan = {"schema": "idealis-rprob-goal-v1", "revision": revision, "policy": POLICY, **initial_inputs}
    output.mkdir(parents=True)
    write(output / "plan.json", plan)
    if initial_inputs != inputs():
        raise RuntimeError("benchmark inputs changed after planning")
    measure(output, NAME, EDGES, POLICY["loops"], POLICY["runs"], [POLICY["mode"]], warmup=POLICY["warmup"])
    if initial_inputs != inputs():
        raise RuntimeError("benchmark inputs changed during measurement")
    measurements, checked_output = read_artifacts(output)
    receipt = {
        "plan": plan,
        "measurements_sha256": digest(measurements),
        "checked_output_sha256": digest(checked_output),
    }
    write(output / RECEIPT, receipt)
    result = score(plan, receipt, measurements, checked_output)
    write(output / "goal-evidence.json", result)
    return result


def read_artifacts(output):
    return (output / f"{NAME}.json").read_bytes(), (output / f"{NAME}-{POLICY['mode']}.txt").read_bytes()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["identify", "record", "replay"])
    parser.add_argument("output", nargs="?", type=Path)
    parser.add_argument("revision", nargs="?", type=Path)
    args = parser.parse_args()
    if args.mode == "identify" and args.output is None and args.revision is None:
        print(f"sha256:{digest(BENCHMARK.read_bytes())}")
        return
    if args.mode == "record" and args.output is not None and args.revision is not None:
        result = record(args.output, json.loads(args.revision.read_text()))
    elif args.mode == "replay" and args.output is not None and args.revision is None:
        plan = json.loads((args.output / "plan.json").read_text())
        receipt = json.loads((args.output / RECEIPT).read_text())
        result = score(plan, receipt, *read_artifacts(args.output))
        if result != json.loads((args.output / "goal-evidence.json").read_text()):
            raise SystemExit("goal evidence disagrees with replay")
    else:
        parser.error("record requires a revision file; replay requires only the run directory")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
