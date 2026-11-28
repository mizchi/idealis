"""Independent, single-Boolean state explorer for testing the file protocol."""

import json
import sys

request = json.load(sys.stdin)
model = request["ir"]
reply = {"version": 1, "requestId": request["requestId"]}
coverage = {"kind": "complete"}


def value(expr, state):
    if expr["op"] != "name":
        raise ValueError("only named Boolean values are supported")
    return {"true": True, "false": False, **state}[expr["name"]]


def condition(expr, state):
    if expr["op"] == "not":
        return not condition(expr["arg"], state)
    if expr["op"] == "and":
        return all(condition(part, state) for part in expr["args"])
    if expr["op"] == "eq":
        return value(expr["left"], state) == value(expr["right"], state)
    raise ValueError("unsupported Boolean formula")


def explore():
    relations = model["relations"]
    shape = {"kind": "unary", "sort": "Bool", "mult": "one"}
    if len(relations) != 1 or not relations[0]["var"] or relations[0]["shape"] != shape:
        raise ValueError("expected one Boolean state variable")
    if model["facts"] or any(action["params"] for action in model["actions"]):
        raise ValueError("configuration and operation arguments are unsupported")
    name = relations[0]["name"]
    predicate = next(check["formula"] for check in model["checks"]
                     if check["name"] == request["question"]["name"] and check["kind"] == "reachable")
    candidates = [{name: bit} for bit in [False, True]]
    queue = [(state, state, []) for state in candidates
             if all(condition(formula, state) for formula in model["init"])]
    if not queue:
        return {"verdict": "empty", "coverage": coverage}
    seen = {state[name] for _, state, _ in queue}
    for count, (initial, state, commands) in enumerate(queue):
        if count == request["budget"]["states"]:
            return {"verdict": "unknown", "coverage": coverage, "reason": "state budget"}
        if condition(predicate, state):
            return {"verdict": "reachable", "coverage": coverage, "witness": {
                "config": {}, "initial": initial, "commands": commands, "finalState": state,
            }}
        for action in model["actions"]:
            if not all(condition(guard, state) for guard in action["guards"]):
                continue
            next_state = state | {assignment["var"]: value(assignment["value"], state)
                                  for assignment in action["assignments"]}
            if next_state[name] not in seen:
                seen.add(next_state[name])
                queue.append((initial, next_state, commands + [{"kind": action["name"]}]))
    return {"verdict": "unreachable", "coverage": coverage}


try:
    reply.update(explore())
except (KeyError, StopIteration, ValueError) as error:
    reply.update(verdict="unsupported", reason=str(error))
print(json.dumps(reply))
