"""Plan 0114: save the portability matrix and raw external checker output."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
RCHECK = ROOT / "crates/rcheck/target/debug/rcheck"
EMIT = ROOT / "crates/rmodel/target/debug/examples/emit"
FIXTURES = ROOT / "fixtures/rcheck/linked"

EMPTY = """model EmptyInit
enum Flag = On | Off
var x: one Flag
init { x = On x = Off }
invariant Safe { x = On }
reachable Reach { x = On }
scope steps = 3
"""
ESCAPE = """model Escaping
var n: one Int
init { n = 0 }
behavior increment() { update { n = n + 1 } }
invariant NonNegative { n >= 0 }
reachable AtOne { n = 1 }
scope Int = 0..1, steps = 3
"""
MAX_INT = """model MaxInt
var n: one Int
init { n = 9223372036854775807 }
behavior increment() { update { n = n + 1 } }
invariant AtMax { n = 9223372036854775807 }
scope Int = 9223372036854775806..9223372036854775807, steps = 2
"""
PROGRESS = r"""---- MODULE Progress ----
VARIABLE delivered
Init == delivered = FALSE
Deliver == /\ ~delivered /\ delivered' = TRUE
Next == Deliver \/ UNCHANGED delivered
Spec == Init /\ [][Next]_delivered @FAIRNESS@
EventuallyDelivered == <>delivered
====
"""
PROBES = {
    "record-field": "record Amount { units: Entry } record Fee { units: Entry }",
    "sequence": "var log: Seq(Entry)",
    "ternary": "var grant: Entry -> Entry -> set Entry",
    "fairness": "fairness weak Deliver",
    "liveness": "property Delivery { eventually some seen }",
}
PROBE_MODEL = """model Probe
enum Entry = Message
var seen: set Entry
init { no seen }
behavior Deliver() { update { seen = seen + Message } }
{probe}
scope steps = 3
"""


def run(args, name, cwd, seconds=35):
    try:
        result = subprocess.run(
            args, cwd=cwd, capture_output=True, text=True, timeout=seconds, check=False
        )
        details = {"exit": result.returncode}
        stdout, stderr = result.stdout, result.stderr
    except subprocess.TimeoutExpired as error:
        details = {"exit": "timeout"}
        stdout = (error.stdout or b"").decode(errors="replace")
        stderr = (error.stderr or b"").decode(errors="replace")
    (cwd / (name + ".out")).write_text(stdout)
    (cwd / (name + ".err")).write_text(stderr)
    return details, stdout + stderr


def tlc(args, cwd, module):
    return run(
        [args.java, "-XX:+UseParallelGC", "-cp", args.tlc, "tlc2.TLC",
         "-workers", "1", "-deadlock", "-metadir", cwd / (module + "-meta"),
         "-config", module + ".cfg", module + ".tla"],
        "tlc-" + module, cwd,
    )


def row(args, name, source, alloy_supported):
    cwd = args.output / name
    cwd.mkdir()
    model = cwd / "source.model"
    model.write_text(source)
    result = {}
    info, ir = run([RCHECK, "generate", model, "ir"], "source-ir", cwd)
    result["ir"] = info
    if info["exit"] != 0:
        return result
    ir = json.loads(ir)
    wire = cwd / "ir.json"
    wire.write_text(json.dumps(ir))
    emit, _ = run([EMIT, model, cwd, "--builtins"], "emit", cwd)
    result["emit"] = emit
    result["checks"] = {}
    commands = []
    for check in [*ir["checks"], {"name": "Declarations"}]:
        question = check["name"]
        checks = result["checks"].setdefault(question, {})
        for label, input_path, backend in [
            ("reference", model, "reference"),
            ("ir-reference", wire, "reference"),
            ("explicit", model, "explicit"),
            ("sat", model, "sat"),
        ]:
            commands.append((question, checks, label, input_path, backend))
    for question, checks, label, input_path, backend in commands:
        out, raw = run(
            [RCHECK, input_path, "--checker", backend, "--question", question],
            f"{label}-{question}", cwd, 15,
        )
        if out["exit"] == 0:
            out.update(json.loads(raw)[0]["answers"][0])
        checks[label] = out
    if emit["exit"] != 0:
        return result

    alloy = {}
    if alloy_supported:
        alloy, _ = run(
            [args.java, "-jar", args.alloy, "exec", "--quiet", "--command", "*",
             "--solver", "sat4j", "--type", "json", "--output", cwd / "alloy",
             cwd / (ir["name"] + ".als")],
            "alloy", cwd,
        )
        if alloy["exit"] == 0:
            alloy["commands"] = {
                key: bool(value.get("solution"))
                for key, value in json.loads((cwd / "alloy/receipt.json").read_text())["commands"].items()
            }
    result["alloy"] = alloy if alloy_supported else {"unsupported": "Int or record"}
    for question, checks in result["checks"].items():
        stem = ir["name"] + "_" + question
        tla = cwd / (stem + ".tla")
        if not tla.exists():
            continue
        out, raw = tlc(args, cwd, stem)
        if "Invariant c_" + question + " is violated" in raw:
            out["verdict"] = "found"
        elif "No error has been found" in raw:
            out["verdict"] = "empty" if "0 distinct states found" in raw else "none"
        elif "No states satisfying Init" in raw:
            out["verdict"] = "empty"
        else:
            out["verdict"] = "error"
        checks["tlc"] = out
    if (cwd / (ir["name"] + "_Scope.tla")).exists():
        out, raw = tlc(args, cwd, ir["name"] + "_Scope")
        out["verdict"] = (
            "found" if "Invariant c_Scope is violated" in raw else
            "none" if "No error has been found" in raw else "error"
        )
        result["scope-tlc"] = out
    return result


def linked(directory):
    paths = json.loads((directory / "project.rmodel.json").read_text())["fragments"]
    return "\n".join((directory / name).read_text() for name in paths)


def mutant(source, before, after):
    if source.count(before) != 1:
        raise ValueError("a mutation must affect exactly one expression")
    return source.replace(before, after)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["java", "tlc", "alloy", "output"]:
        parser.add_argument(name, type=lambda value: Path(value).resolve())
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    withdrawal = linked(FIXTURES / "withdrawal")
    raft = linked(FIXTURES / "raft")
    cases = [
        ("withdrawal", withdrawal, False),
        ("withdrawal-fault", mutant(withdrawal, "beforeBalance = balance result = InsufficientFunds",
                                    "beforeBalance = balance balance = Amount { units: 0 } result = InsufficientFunds"), False),
        ("raft", raft, True),
        ("raft-fault", mutant(raft, "v != c no v.votes", "v != c"), True),
        ("acl", (ROOT / "fixtures/rmodel/permissions.model").read_text(), True),
        ("empty", EMPTY, True),
        ("escape", ESCAPE, False),
        ("integer-width", MAX_INT, False),
    ]
    with ThreadPoolExecutor(max_workers=4) as pool:
        jobs = [(name, pool.submit(row, args, name, source, alloy)) for name, source, alloy in cases]
        outputs = {name: job.result() for name, job in jobs}
    outputs["syntax"] = {}
    for name, probe in PROBES.items():
        source = args.output / (name + ".model")
        source.write_text(PROBE_MODEL.replace("{probe}", probe))
        outputs["syntax"][name], _ = run([RCHECK, "generate", source, "ir"], name, args.output)
    outputs["ir-probes"] = {}
    base = json.loads((args.output / "raft" / "ir.json").read_text())
    for name in ("sequence", "ternary", "fairness", "liveness"):
        candidate = json.loads(json.dumps(base))
        if name in ("sequence", "ternary"):
            candidate["relations"][0]["shape"]["kind"] = name
        elif name == "fairness":
            candidate["fairness"] = [{"weak": "grantVote"}]
        else:
            candidate["checks"][0]["kind"] = "liveness"
        path = args.output / (name + ".json")
        path.write_text(json.dumps(candidate))
        outputs["ir-probes"][name], _ = run(
            [RCHECK, path, "--checker", "reference"], "ir-" + name, args.output
        )

    outputs["progress"] = {}
    for name, fairness in [("without-fairness", ""), ("weak-fairness", r"/\ WF_delivered(Deliver)")]:
        cwd = args.output / name
        cwd.mkdir()
        (cwd / "Progress.tla").write_text(PROGRESS.replace("@FAIRNESS@", fairness))
        (cwd / "Progress.cfg").write_text("SPECIFICATION Spec\nPROPERTY EventuallyDelivered\n")
        out, raw = tlc(args, cwd, "Progress")
        out["verdict"] = (
            "violated" if out["exit"] == 13 and "was violated." in raw else
            "holds" if "No error has been found" in raw else "error"
        )
        outputs["progress"][name] = out
    (args.output / "matrix.json").write_text(json.dumps(outputs, indent=2) + "\n")


if __name__ == "__main__":
    main()
