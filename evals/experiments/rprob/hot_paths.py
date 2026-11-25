"""Measure successive rprob kernels with identical models and budgets."""

import argparse
import platform
import subprocess
from pathlib import Path

from bench import BENCHMARK, EDGES, PERMISSIONS
from candidates import measure

FIXTURES = Path("fixtures/rprob")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("--stress", action="store_true")
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
    workloads = [
        ("dense-edges", dense, 3, None),
        ("permissions", PERMISSIONS, 20, None),
        ("sparse-sets", FIXTURES / "sparse-sets.model", 3, None),
        ("split-sets", FIXTURES / "split-sets.model", 3, None),
        ("wide-function", FIXTURES / "wide-function.model", 40, 46658),
    ]
    options = {"warmup": 2, "runs": 9} if args.stress else {}
    if args.stress:
        workloads.append(("bounded-initial", FIXTURES / "candidates-large.model", 3, 1_000_000))
    for label, model, iterations, budget in workloads:
        measure(output, label, before, "prepared", model, iterations, budget, **options)
    measure(output, "large-sets", before, "cold", FIXTURES / "candidates-large.model", 100 if args.stress else 1, 1, **options)


if __name__ == "__main__":
    main()
