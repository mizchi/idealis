"""Measure the pre-built reference and bitset checker on fixed workloads."""
import platform
import shlex
import subprocess
import sys
from pathlib import Path

BENCHMARK = Path("evals/experiments/rprob/target/release/rprob-benchmark")
PERMISSIONS = Path("fixtures/rmodel/permissions.model")
EDGES = Path("fixtures/rprob/edges.model")


def command_args(backend, mode, model, loops):
    return [str(BENCHMARK), backend, mode, str(model), str(loops)]


def measure(output, label, model, loops, runs, modes, *, warmup=1):
    commands = []
    for mode in modes:
        expected = None
        for backend in ("reference", "rprob"):
            argv = command_args(backend, mode, model, loops)
            tested = subprocess.check_output(argv, text=True)
            if expected is None:
                expected = tested
            elif expected != tested:
                raise SystemExit(f"{label}: {mode} disagrees with the reference")
            commands += ["--command-name", f"{backend}-{mode}", shlex.join(argv)]
        (output / f"{label}-{mode}.txt").write_text(expected)
    subprocess.run([
        "hyperfine", "--warmup", str(warmup), "--runs", str(runs),
        "--metrics", "time_wall_clock:ms,time_cpu:ms,memory_peak_resident:MiB",
        "--export-json", str(output / f"{label}.json"), *commands,
    ], check=True)


def main():
    output = Path(sys.argv[1])
    output.mkdir(parents=True)
    version = subprocess.check_output(["hyperfine", "--version"], text=True)
    (output / "host.txt").write_text(f"{platform.platform()}\n{version}")
    large = output / "edges-4x4.model"
    source = EDGES.read_text()
    assert source.count("Left = 3") == 1
    large.write_text(source.replace("Left = 3", "Left = 4"))
    measure(output, "permissions", PERMISSIONS, 3, 7, ["cold", "prepared"])
    measure(output, "edges-3x4", EDGES, 4, 7, ["cold", "prepared"])
    measure(output, "edges-4x4", large, 1, 5, ["prepared"])


if __name__ == "__main__":
    main()
