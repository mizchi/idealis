#!/usr/bin/env python3
"""Link rustc's live objects in alternating order, retaining the default output."""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

args = sys.argv[1:]
if "-o" not in args:
    os.execv(os.environ["BASE_LINKER"], [os.environ["BASE_LINKER"], *args])
if os.environ["FORMAT"] == "native":
    default = [os.environ["BASE_LINKER"], *args]
    wild = ["clang", *args, "--ld-path=" + os.environ["WILD_BINARY"]]
else:
    if args[:2] == ["-flavor", "wasm"]:
        args = args[2:]
    default = [os.environ["BASE_LINKER"], "-flavor", "wasm", *args]
    wild = [os.environ["WILD_BINARY"], "-flavor", "wasm", *args]
samples = []
for cmd in [default, wild, wild, default]:
    start = time.monotonic()
    subprocess.run(cmd, check=True)
    samples.append(time.monotonic() - start)
with open(os.environ["LINK_TIMINGS"], "a") as out:
    print(json.dumps(samples), file=out)
