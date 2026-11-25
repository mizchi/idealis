"""Compare two builds of the same rprob harness on fixed candidate workloads."""

import argparse
import platform
import shlex
import subprocess
from pathlib import Path

from bench import BENCHMARK, EDGES, PERMISSIONS


def measure(output, label, before, mode, model, iterations, budget=None, *, warmup=1, runs=5):
    commands = []
    expected = None
    for name, binary in (("before", before), ("after", BENCHMARK.resolve())):
        argv = [str(binary), "rprob", mode, str(model), str(iterations)]
        if budget is not None:
            argv.append(str(budget))
        report = subprocess.check_output(argv, text=True)
        if expected is None:
            expected = report
        elif report != expected:
            raise SystemExit(f"{label}: reports differ\nBefore:\n{expected}\nAfter:\n{report}")
        commands += ["--command-name", name, shlex.join(argv)]
    (output / f"{label}.txt").write_text(expected)
    subprocess.run([
        "hyperfine", "--warmup", str(warmup), "--runs", str(runs),
        "--metrics", "time_wall_clock:ms,instructions,memory_peak_resident:MiB",
        "--export-json", str(output / f"{label}.json"), *commands,
    ], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("baseline", type=Path)
    args = parser.parse_args()
    output = args.output
    before = args.baseline.resolve()
    if before == BENCHMARK.resolve():
        parser.error("baseline must be a separate build")
    output.mkdir(parents=True)
    version = subprocess.check_output(["hyperfine", "--version"], text=True)
    (output / "host.txt").write_text(f"{platform.platform()}\n{version}")
    dense = output / "edges-4x4.model"
    source = EDGES.read_text()
    assert source.count("Left = 3") == 1
    dense.write_text(source.replace("Left = 3", "Left = 4"))

    measure(output, "large-sets", before, "cold", Path("fixtures/rprob/candidates-large.model"), 1, 1)
    measure(output, "total-function", before, "cold", Path("fixtures/rprob/function-choice.model"), 20, 1000)
    measure(output, "dense-edges", before, "prepared", dense, 3)
    measure(output, "permissions", before, "prepared", PERMISSIONS, 20)


if __name__ == "__main__":
    main()
