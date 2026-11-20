import { readFile } from "node:fs/promises";
import { isAbsolute, parse, posix, relative, sep } from "node:path";
import { WASI } from "node:wasi";

// Guest paths use POSIX separators, including for a native Windows drive or UNC share.
const mounts = new Map<string, string>();
function guestPath(path: string): string {
  const root = parse(path).root;
  let mount = mounts.get(root);
  if (!mount) {
    mount = root === "/" ? "/" : "/host" + mounts.size;
    mounts.set(root, mount);
  }
  return posix.join(mount, relative(root, path).split(sep).join("/"));
}

const cwd = guestPath(process.cwd());
const args = process.argv
  .slice(2)
  .map((arg) => (isAbsolute(arg) ? guestPath(arg) : arg.split(sep).join("/")));
const wasi = new WASI({
  version: "preview1",
  args: ["idealis", ...args],
  env: { IDEALIS_CWD: cwd },
  preopens: Object.fromEntries(
    Array.from(mounts, ([host, guest]) => [guest, host]),
  ),
  returnOnExit: true,
});
const program = await WebAssembly.compile(
  new Uint8Array(
    await readFile(new URL("../../assets/idealis-cli.wasm", import.meta.url)),
  ),
);
const instance = await WebAssembly.instantiate(program, {
  wasi_snapshot_preview1: wasi.wasiImport,
});
process.exitCode = wasi.start(instance);
