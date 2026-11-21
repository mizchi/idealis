import json, sys, glob

def reads(x, out):
    if isinstance(x, dict):
        if x.get("op") == "bound":
            out.add(x["slot"])
        for v in x.values():
            reads(v, out)
    elif isinstance(x, list):
        for v in x:
            reads(v, out)

def has_enabled(x):
    if isinstance(x, dict):
        if x.get("op") == "enabled":
            return True
        return any(has_enabled(v) for v in x.values())
    if isinstance(x, list):
        return any(has_enabled(v) for v in x)
    return False

def scan(branch_steps, found):
    i = 0
    while i < len(branch_steps):
        if branch_steps[i].get("kind") == "bind":
            j = i
            while j < len(branch_steps) and branch_steps[j].get("kind") == "bind":
                j += 1
            if j < len(branch_steps):
                later = branch_steps[j:]
                if not has_enabled(later):
                    r = set()
                    reads(later, r)
                    slots = [s["slot"] for s in branch_steps[i:j]]
                    dead = [s for s in slots if s not in r]
                    if dead:
                        found.append({"binds": len(slots), "dead": len(dead)})
            i = j
        else:
            i += 1
    for s in branch_steps:
        for inner in s.get("branches", []) if isinstance(s, dict) else []:
            scan(inner.get("steps", []), found)

def program_runs(program):
    found = []
    for a in program["next"]:
        scan(a["steps"], found)
    for d in program.get("definitions", []):
        if isinstance(d, dict):
            for key in ("branches",):
                for b in d.get(key, []):
                    scan(b.get("steps", []), found)
    return found

if __name__ == "__main__":
    for path in sys.argv[1:]:
        p = json.load(open(path))["program"] if "program" in json.load(open(path)) else json.load(open(path))
        f = program_runs(p)
        print(path.split("/")[-1], len(f), f[:3])
