#!/usr/bin/env python3
"""Apply one temporary fault to a scratch copy of the after tree, run the rtlc lib tests, restore the file.

usage: S=<scratch> W=<worktree> faults.py <fault id|baseline>
The fault edits are exact string replacements; an edit that does not match exactly once aborts.
"""
import os, subprocess, sys

S = os.environ["S"]
tree = f"{S}/trees/fault"
E = "crates/rtlc/src/safety_expressions.rs"
L = "crates/rtlc/src/safety_lazy.rs"

LOOP_HEAD = "    loop {\n        let argument = ctx\n"
FAULTS = {
    "F1": [(E, "            forwarded: inner.forwarded.saturating_add(1),\n", "            forwarded: inner.forwarded,\n")],
    "F2a": [
        (E, LOOP_HEAD, "    let mut charged = false;\n" + LOOP_HEAD),
        (E, "        arena.consume(1)?;\n        let inner = EvalContext {\n", "        if !charged {\n            arena.consume(1)?;\n            charged = true;\n        }\n        let inner = EvalContext {\n"),
    ],
    "F2b": [
        (E, LOOP_HEAD, "    let mut hops = 0usize;\n" + LOOP_HEAD),
        (E, "        arena.consume(1)?;\n        let inner = EvalContext {\n", "        hops += 1;\n        let inner = EvalContext {\n"),
        (E, "        let Some(next) = argument.expression.reference() else {\n            return evaluate(&argument.expression, inner, arena);\n", "        let Some(next) = argument.expression.reference() else {\n            arena.consume(hops)?;\n            return evaluate(&argument.expression, inner, arena);\n"),
    ],
    "F3a": [(E, "            old: argument.old,\n            ..ctx\n        };\n        let Some(next)", "            old: argument.old,\n            prime: false,\n            ..ctx\n        };\n        let Some(next)")],
    "F3b": [(E, "            return evaluate(&argument.expression, inner, arena);\n", "            let terminal = match &*argument.expression {\n                Expression::Prime { argument } => &**argument,\n                other => other,\n            };\n            return evaluate(terminal, inner, arena);\n")],
    "F4": [(E, "            locals: &argument.locals,\n            frame: argument.frame.as_deref(),\n            old: argument.old,\n            ..ctx\n", "            locals: ctx.locals,\n            frame: argument.frame.as_deref(),\n            old: ctx.old,\n            ..ctx\n")],
    "F5a": [(E, "                                    forwarded: ctx.forwarded,\n                                    old,\n", "                                    forwarded: 0,\n                                    old,\n")],
    "F5b": [(E, "                            forwarded: ctx.forwarded,\n                            recursive: ctx.recursive,\n", "                            forwarded: 0,\n                            recursive: ctx.recursive,\n")],
    "F5c": [(L, "            forwarded: context.forwarded,\n", "            forwarded: 0,\n")],
    "F6": [(E, "        inner.check_nesting(arena)?;\n", "")],
    "F7": [(E, "        if self.nesting_is_unbounded(arena) {\n            return Ok(());\n        }\n        self.refuse_excess_nesting()?;\n        self.evaluation_depth += 1;", "        if !self.recursive && self.evaluation_depth >= DEPTH_LIMIT {\n            return Err(CoreError::protocol(\"Safety evaluation nesting exceeds 256\"));\n        }\n        if self.nesting_is_unbounded(arena) {\n            return Ok(());\n        }\n        self.refuse_excess_nesting()?;\n        self.evaluation_depth += 1;")],
}

name = sys.argv[1]
originals = {E: open(f"{S}/trees/fault-safety_expressions.rs.orig").read(), L: open(f"{S}/trees/fault-safety_lazy.rs.orig").read()}
for path, text in originals.items():
    open(f"{tree}/{path}", "w").write(text)
if name != "baseline":
    texts = dict(originals)
    for path, old, new in FAULTS[name]:
        assert texts[path].count(old) == 1, (name, path, old[:60], texts[path].count(old))
        texts[path] = texts[path].replace(old, new)
    for path, text in texts.items():
        open(f"{tree}/{path}", "w").write(text)
    diff = subprocess.run(["diff", "-u", f"{S}/trees/fault-safety_expressions.rs.orig", f"{tree}/{E}"], capture_output=True, text=True).stdout
    diff += subprocess.run(["diff", "-u", f"{S}/trees/fault-safety_lazy.rs.orig", f"{tree}/{L}"], capture_output=True, text=True).stdout
    open(f"{S}/runs/faults/{name}.patch", "w").write(diff)
print(f"{name} applied")
