#!/usr/bin/env python3
"""Run one measured command under an RSS watchdog, a wall-clock limit and a disk floor.

usage: E=<scratch> run.py [--timeout SECONDS] [--rss-gib 4] [--floor-gib 30] <name> <cwd> -- <command...>

Appends one row to $E/commands.tsv, writes $E/logs/<name>.log and $E/logs/<name>.exit and refuses to
overwrite a log. The row has: name, start, end, command, cwd, load at start, exit, outcome
(ok, failed, rss-cap, timeout or disk-floor), peak RSS of the process tree in KiB, wall seconds.
Host paths in the row are replaced by the placeholders listed in $E/replace.tsv (path<TAB>placeholder).
"""
import os
import signal
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone


def tree_rss(root):
    out = subprocess.run(["ps", "-axo", "pid=,ppid=,rss="], capture_output=True, text=True).stdout
    children, rss = {}, {}
    for line in out.splitlines():
        parts = line.split()
        if len(parts) != 3:
            continue
        pid, ppid, kib = map(int, parts)
        children.setdefault(ppid, []).append(pid)
        rss[pid] = kib
    total, stack, seen = 0, [root], set()
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        total += rss.get(pid, 0)
        stack.extend(children.get(pid, []))
    return total


def free_gib(path):
    return shutil.disk_usage(path).free / 2**30


def placeholders(env_dir):
    rows = []
    path = os.path.join(env_dir, "replace.tsv")
    if os.path.exists(path):
        for line in open(path):
            line = line.rstrip("\n")
            if "\t" in line:
                a, b = line.split("\t", 1)
                rows.append((a, b))
    rows.sort(key=lambda row: -len(row[0]))
    return rows


def main():
    args = sys.argv[1:]
    timeout, rss_gib, floor = 1800.0, 4.0, 30.0
    while args and args[0].startswith("--"):
        flag = args.pop(0)
        value = args.pop(0)
        if flag == "--timeout":
            timeout = float(value)
        elif flag == "--rss-gib":
            rss_gib = float(value)
        elif flag == "--floor-gib":
            floor = float(value)
        else:
            sys.exit(f"unknown flag {flag}")
    name, cwd = args[0], args[1]
    assert args[2] == "--", "expected --"
    command = args[3:]
    env_dir = os.environ["E"]
    os.makedirs(os.path.join(env_dir, "logs"), exist_ok=True)
    log = os.path.join(env_dir, "logs", name + ".log")
    if os.path.exists(log):
        print(f"refusing to overwrite {log}", file=sys.stderr)
        sys.exit(99)
    start = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    load = os.popen("uptime").read().split("load averages:")[-1].strip().replace(",", "")
    started = time.time()
    def interrupt(signum, frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, interrupt)
    outcome, code, peak = "ok", None, 0
    if free_gib(env_dir) < floor:
        outcome, code = "disk-floor", 98
        open(log, "w").write(f"free disk {free_gib(env_dir):.1f} GiB is below the floor {floor} GiB before start\n")
    else:
        env = {k: v for k, v in os.environ.items() if k not in ("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR")}
        with open(log, "w") as sink:
            child = subprocess.Popen(command, cwd=cwd, stdout=sink, stderr=subprocess.STDOUT, env=env, start_new_session=True)
            while True:
                try:
                    code = child.wait(timeout=0.25)
                    break
                except subprocess.TimeoutExpired:
                    pass
                except KeyboardInterrupt:
                    outcome = "interrupted"
                    try:
                        os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    code = child.wait()
                    sink.write("\n[run.py] killed: interrupted\n")
                    break
                rss = tree_rss(child.pid)
                peak = max(peak, rss)
                reason = None
                if rss > rss_gib * 2**20:
                    reason = "rss-cap"
                elif time.time() - started > timeout:
                    reason = "timeout"
                elif free_gib(env_dir) < floor:
                    reason = "disk-floor"
                if reason:
                    outcome = reason
                    try:
                        os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    code = child.wait()
                    sink.write(f"\n[run.py] killed: {reason}\n")
                    break
            peak = max(peak, 0)
        if outcome == "ok" and code != 0:
            outcome = "failed"
    end = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    shown = [" ".join(command), cwd]
    for a, b in placeholders(env_dir):
        shown = [text.replace(a, b) for text in shown]
    row = "\t".join([name, start, end, shown[0], f"cwd={shown[1]}", f"load_at_start={load}", f"exit={code}", f"outcome={outcome}", f"peak_rss_kib={peak}", f"wall_s={time.time() - started:.1f}"])
    with open(os.path.join(env_dir, "commands.tsv"), "a") as table:
        table.write(row + "\n")
    open(os.path.join(env_dir, "logs", name + ".exit"), "w").write(f"{name} exit={code} outcome={outcome}\n")
    print(f"{name} exit={code} outcome={outcome} peak_rss_kib={peak} wall_s={time.time() - started:.1f}")
    sys.exit(0 if code == 0 else (code if code and code > 0 else 1))


main()
