"""Replay the 0116 checks using generated TLA+ and an independent TLC."""

import argparse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
MODELS = ROOT / "fixtures/rcheck/structured"
EMIT = ROOT / "crates/idealis-model/target/debug/examples/emit"

CASES = [
    ("journal", "ordered-log", None, {
        "Bounded": "holds", "ValidResult": "holds", "Ordered": "reachable",
        "Repeated": "reachable", "Missing": "reachable", "Scope": "holds",
    }),
    ("overflow", "ordered-log", ("guard { len(log) < 2 }", ""), {"Scope": "violated"}),
    ("payment", "payment", None, {
        "NoZeroPayment": "holds", "PaidExactly": "reachable", "CanReject": "reachable",
    }),
    ("fair", "delivery", None, {"Delivers": "holds"}),
    ("unfair", "delivery", ("fairness weak deliver", ""), {"Delivers": "temporal-violation"}),
    ("strong", "retry-delivery", None, {"Delivers": "holds"}),
    ("weak", "retry-delivery", ("fairness strong deliver", "fairness weak deliver"),
     {"Delivers": "temporal-violation"}),
    ("service", "service", None, {
        "Disjoint": "holds", "EventuallyServed": "holds",
        "EventuallyServedUnfair": "temporal-violation",
    }),
]

TRANSITION_CASES = [
    ("shipment", "shipped-transition", None, {"ShippedStays": "holds"}),
    ("faulty-shipment", "shipped-transition",
     ("acknowledged = true }", "acknowledged = true status = Shipped(Post) }"),
     {"ShippedStays": "action-violation"}),
    ("retry", "retry-transition", None, {"AttemptsNeverFallWhileSent": "holds"}),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("java", "jar", "output"):
        parser.add_argument(name, type=lambda v: Path(v).resolve())
    parser.add_argument("--transition", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    jobs = []

    for case, model, edit, checks in (TRANSITION_CASES if args.transition else CASES):
        folder = args.output / case
        folder.mkdir()
        text = (MODELS / (model + ".model")).read_text()
        if edit:
            assert text.count(edit[0]) == 1, edit
            text = text.replace(*edit)
        source = folder / "input.model"
        source.write_text(text)
        subprocess.run([EMIT, source, folder, "--builtins"], check=True)
        for name, expected in checks.items():
            config, = folder.glob(f"*_{name}.cfg")
            jobs.append((config, name, expected))

    def check(job):
        config, name, expected = job
        module = config.stem
        result = subprocess.run(
            [args.java, "-cp", args.jar, "tlc2.TLC", "-workers", "1", "-deadlock",
             "-metadir", module + "-meta", "-config", config.name, module + ".tla"],
            cwd=config.parent, capture_output=True, text=True, timeout=35, check=False,
        )
        log = config.with_suffix(".log")
        log.write_text(result.stdout + result.stderr)
        if result.returncode == 0 and "No error has been found" in result.stdout:
            actual = "holds"
        elif result.returncode == 12 and f"Invariant c_{name} is violated" in result.stdout:
            actual = "reachable" if expected == "reachable" else "violated"
        elif result.returncode == 13 and f"Temporal property c_{name} was violated" in result.stdout:
            actual = "temporal-violation"
        elif result.returncode != 0 and f"Action property c_{name} is violated" in result.stdout:
            actual = "action-violation"
        else:
            actual = f"error ({result.returncode})"
        return log, expected, actual

    with ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(check, jobs))
    summary = "\n".join(
        f"{log.relative_to(args.output)} {actual} expected {expected}"
        for log, expected, actual in results
    )
    (args.output / "results.txt").write_text(summary + "\n")
    print(summary)
    if any(expected != actual for _, expected, actual in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
