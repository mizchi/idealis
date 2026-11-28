"""Derive and verify the two fixed shipment traces, using rmodel as the oracle."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

MODEL = Path("fixtures/rcheck/structured/shipped-transition.model")
R_CHECK = Path("crates/idealis-check/target/debug/idealis-check")
FIXTURES = Path("fixtures/rcheck/shipment")
BINDINGS = {
    "rust": Path("crates/idealis-check/tests/generated/shipment.rs"),
    "typescript": Path("tests/idealis-check/generated/shipment.ts"),
    "go": Path("tests/idealis-check/go/shipment/bindings.go"),
}


def invoke(*args, request=None, check=True):
    return subprocess.run(
        [R_CHECK, *args],
        input=json.dumps(request) if request is not None else None,
        capture_output=True,
        check=check,
        text=True,
    )


def validation_request(states, command):
    return {
        "config": states[0]["config"],
        "initial": states[0]["observation"],
        "prefix": [frame["command"] for frame in states[1:]],
        "command": command,
    }


def trace(method):
    initial = {"status": {"tag": "Ready"}, "acknowledged": False}
    states = [frame(0, {"kind": "init"}, initial)]
    for command in [{"kind": "ship", "method": method}, {"kind": "acknowledge"}]:
        reply = invoke("validate", MODEL, "-", request=validation_request(states, command))
        states.append(frame(len(states), command, json.loads(reply.stdout)["state"]))
    return {
        "#meta": {"format": "ITF", "source": str(MODEL)},
        "vars": ["config", "command", "observation"],
        "states": states,
    }


def frame(index, command, observation):
    return {"#meta": {"index": index}, "config": {}, "command": command, "observation": observation}


def verify_model_fault(courier_trace):
    source = MODEL.read_text()
    old = "update { acknowledged = true }"
    assert source.count(old) == 1
    with tempfile.TemporaryDirectory() as directory:
        fault = Path(directory) / "shipment.model"
        fault.write_text(source.replace(old, "update { acknowledged = true status = Shipped(Post) }"))
        report = invoke(fault, "--checker", "reference", "--question", "ShippedStays")
        answer = json.loads(report.stdout)[0]["answers"][0]
        assert (answer["verdict"], answer["depth"]) == ("violated", 2), answer
        states = courier_trace["states"]
        reply = invoke(
            "validate", fault, "-", check=False,
            request=validation_request(states[:-1], states[-1]["command"]),
        )
        assert reply.returncode == 2 and "outside the model: transition ShippedStays" in reply.stderr, reply


def persist(path, contents, write):
    if write:
        path.write_text(contents)
    elif path.read_text() != contents:
        raise SystemExit(f"{path} is stale; run just shipment-write")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    write = parser.parse_args().write
    for target, path in BINDINGS.items():
        source = invoke("generate", MODEL, target).stdout
        if target == "go":
            source = subprocess.run(["gofmt"], input=source, capture_output=True, check=True, text=True).stdout
        persist(path, source, write)
    for method in ["Courier", "Post"]:
        result = trace(method)
        persist(FIXTURES / f"{method.lower()}.itf.json", json.dumps(result, indent=2) + "\n", write)
        if method == "Courier":
            verify_model_fault(result)
    print("Two model-derived ITF traces verified; the faulty model violates ShippedStays at acknowledge.")


if __name__ == "__main__":
    main()
