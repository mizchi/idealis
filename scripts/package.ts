import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFile,
  cp,
  mkdir,
  mkdtemp,
  readFile,
  writeFile,
} from "node:fs/promises";
import { resolve } from "node:path";

const target = resolve(process.argv[2] ?? ".artifacts/packages");
await mkdir(target, { recursive: true });
const packageDir = resolve("packages/idealis");
const pkg = JSON.parse(
  await readFile(resolve(packageDir, "package.json"), "utf8"),
);
execFileSync("pnpm", ["pack", "--pack-destination", target], {
  cwd: packageDir,
  stdio: "inherit",
});
const workspace = await mkdtemp(resolve(target, "cargo-workspace-"));
const crates = [
  "rtla",
  "rtlc",
  "rtla-compiler",
  "idealis-core",
  "rusmt",
  "ralloy",
  "idealis-connect",
  "idealis-cli",
  "roracle",
];
for (const name of crates) {
  await mkdir(resolve(workspace, name));
  for (const file of [
    "src",
    ...(name === "idealis-cli" ? [] : ["examples"]),
    "tests",
    "Cargo.toml",
    "Cargo.lock",
    "README.md",
    "SUPPORT.md",
    "LICENSE",
    ...(["rtla", "ralloy"].includes(name) ? ["NOTICE"] : []),
    ...(["rusmt", "roracle"].includes(name) ? ["benches"] : []),
  ])
    await cp(`crates/${name}/${file}`, resolve(workspace, name, file), {
      recursive: true,
    });
}
await writeFile(
  resolve(workspace, "Cargo.toml"),
  `[workspace]\nmembers=${JSON.stringify(crates)}\nresolver="3"\n`,
);
await copyFile("crates/rtlc/Cargo.lock", resolve(workspace, "Cargo.lock"));
execFileSync(
  "cargo",
  [
    "metadata",
    "--offline",
    "--format-version",
    "1",
    "--manifest-path",
    resolve(workspace, "Cargo.toml"),
  ],
  { stdio: "pipe" },
);
execFileSync(
  "cargo",
  [
    "package",
    "--workspace",
    "--allow-dirty",
    "--offline",
    "--locked",
    "--manifest-path",
    resolve(workspace, "Cargo.toml"),
  ],
  { stdio: "pipe" },
);
for (const name of crates) {
  const crate = `${name}-${pkg.version}.crate`;
  await copyFile(
    resolve(workspace, "target/package", crate),
    resolve(target, crate),
  );
}
for (const file of [
  "idealis-core.wasm",
  "manifest.json",
  "rtla.wasm",
  "rtla-manifest.json",
  "ralloy.wasm",
  "ralloy-manifest.json",
  "idealis-connect.wasm",
  "idealis-connect-manifest.json",
]) {
  await copyFile(
    resolve(packageDir, "assets", file),
    resolve(target, file === "manifest.json" ? "wasm-manifest.json" : file),
  );
}
const smt = await readFile(
  "crates/rusmt/target/wasm32-unknown-unknown/release/rusmt.wasm",
);
await writeFile(resolve(target, "rusmt.wasm"), smt);
await writeFile(
  resolve(target, "rusmt-manifest.json"),
  `${JSON.stringify({ version: pkg.version, abi: 1, sha256: createHash("sha256").update(smt).digest("hex"), bytes: smt.length }, null, 2)}\n`,
);
console.log(
  `Verified npm, rtla/rtla-compiler/rtlc/idealis-core/rusmt/ralloy/idealis-connect/idealis-cli/roracle Cargo and Wasm artifacts: ${target}`,
);
