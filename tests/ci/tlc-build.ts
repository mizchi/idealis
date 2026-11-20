import { createHash } from "node:crypto";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const nativePath = "crates/idealis-core/target/release/idealis-core";
export const wasmPath =
  "crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm";

export function tlcBuild() {
  const manifest = readFileSync("crates/rtlc/Cargo.toml", "utf8");
  const rtlc = /^version = "([^"]+)"$/m.exec(manifest)?.[1];
  if (!rtlc) throw new Error("Missing rtlc version");
  const tla = JSON.parse(
    readFileSync(new URL(import.meta.resolve("tla-ts/package.json")), "utf8"),
  ).version;
  const reference = dirname(fileURLToPath(import.meta.resolve("tla-ts")));
  const hash = createHash("sha256");
  const add = (name: string, path: string) => {
    hash
      .update(name)
      .update("\0")
      .update(createHash("sha256").update(readFileSync(path)).digest());
  };
  for (const path of [
    nativePath,
    wasmPath,
    "pnpm-lock.yaml",
    "dist/evals/experiments/usable/native.js",
  ]) {
    add(path, path);
  }
  for (const [label, directory] of Object.entries({
    reference,
    host: "dist/packages/idealis/src",
    cache: "dist/tests/ci",
  })) {
    for (const file of readdirSync(directory, {
      recursive: true,
      encoding: "utf8",
    }).sort()) {
      if (/\.c?js$/.test(file) && !file.endsWith(".test.js"))
        add(label + "/" + file, join(directory, file));
    }
  }
  return `rtlc-${rtlc}-tla-ts-${tla}-${process.platform}-${process.arch}-${process.version}-${hash.digest("hex")}`;
}
