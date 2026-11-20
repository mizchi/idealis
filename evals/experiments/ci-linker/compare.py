"""Run on one Linux CI runner per workload, with separate empty Cargo targets."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

task = sys.argv[1]
projects = {
    "rtlc-tests": ("rtlc", ["test", "--all-targets"]),
    "native-core": ("idealis-core", ["build", "--release"]),
    "wasm-core": ("idealis-core", ["build", "--release", "--lib", "--target", "wasm32-unknown-unknown"]),
    "wasi-cli": ("idealis-cli", ["build", "--release", "--bin", "idealis", "--target", "wasm32-wasip1"]),
}
crate, args = projects[task]
common = ["--locked", "--manifest-path", f"crates/{crate}/Cargo.toml"]
subprocess.run(["cargo", "fetch", *common], check=True)
wild = Path(os.environ["RUNNER_TEMP"]) / "wild"
command = '{"version":1,"request":{"kind":"shrink-candidates","length":0}}'
expected = {"kind": "candidates", "ranges": []}

def verify(target, mode):
    if task == "native-core":
        binary = target / "release/idealis-core"
        result = subprocess.check_output([binary], input=command+"\n", text=True)
        assert json.loads(result) == expected
    elif task == "wasm-core":
        module = target / "wasm32-unknown-unknown/release/idealis_core.wasm"
        code = """import {readFileSync} from 'node:fs';
import {createPortableWasm} from './dist/packages/idealis/src/portable-wasm.js';
const core = await createPortableWasm(readFileSync(process.argv[1]));
console.log(JSON.stringify(core.request(JSON.parse(process.argv[2]))));"""
        result = subprocess.check_output(["node", "--input-type=module", "-e", code, module, command], text=True)
        assert json.loads(result) == expected
    elif task == "wasi-cli":
        binary = target / "wasm32-wasip1/release/idealis.wasm"
        code = """import {WASI} from 'node:wasi'; import {readFileSync} from 'node:fs';
const wasi = new WASI({version:'preview1', args:['idealis','tlc','-help'], returnOnExit:true});
const {instance} = await WebAssembly.instantiate(readFileSync(process.argv[1]), {wasi_snapshot_preview1:wasi.wasiImport});
process.exitCode = wasi.start(instance);"""
        result = subprocess.check_output(["node", "--input-type=module", "-e", code, binary], text=True, stderr=subprocess.STDOUT)
        assert "TLC" in result or "tlc" in result, result
    if task in ("rtlc-tests", "native-core"):
        binary = (target / "release/idealis-core") if task == "native-core" else next(p for p in (target / "debug/deps").glob("rtlc-*") if os.access(p,os.X_OK))
        comment = subprocess.check_output(["readelf", "-p", ".comment", binary], text=True)
        print(comment)
        if mode == "wild":
            assert "wild" in comment.lower(), comment

for mode in ["default", "wild", "wild", "default"]:
    with tempfile.TemporaryDirectory(dir=os.environ["RUNNER_TEMP"]) as tmp:
        target = Path(tmp)
        env = dict(os.environ, CARGO_TARGET_DIR=tmp)
        if mode == "wild":
            if task in ("wasm-core", "wasi-cli"):
                triple = args[-1].upper().replace("-", "_")
                env[f"CARGO_TARGET_{triple}_RUSTFLAGS"] = f"-Clinker={wild} -Clinker-flavor=wasm-ld"
            else:
                env["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"] = "clang"
                env["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS"] = f"-Clink-arg=--ld-path={wild}"
        start = time.monotonic()
        subprocess.run(["cargo", *args, "--offline", *common], check=True, env=env)
        elapsed = time.monotonic() - start
        verify(target, mode)
        row = f"{task} {mode} {elapsed:.3f}s"
        print("MEASURE", row, flush=True)
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as out:
            print("-", row, file=out)
