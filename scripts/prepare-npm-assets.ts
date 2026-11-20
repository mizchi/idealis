import { createHash } from "node:crypto";
import { copyFile, cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const packageDir = "packages/idealis";
const assets = join(packageDir, "assets");
const { version } = JSON.parse(
  await readFile(join(packageDir, "package.json"), "utf8"),
);
await mkdir(assets, { recursive: true });
for (const [crate, manifest] of [
  ["idealis-core", "manifest.json"],
  ["rtla", "rtla-manifest.json"],
  ["ralloy", "ralloy-manifest.json"],
  ["idealis-connect", "idealis-connect-manifest.json"],
] as const) {
  const binary = await readFile(
    `crates/${crate}/target/wasm32-unknown-unknown/release/${crate.replaceAll("-", "_")}.wasm`,
  );
  const sha256 = createHash("sha256").update(binary).digest("hex");
  await writeFile(join(assets, `${crate}.wasm`), binary);
  await writeFile(
    join(assets, manifest),
    `${JSON.stringify({ version, abi: 1, sha256, bytes: binary.length }, null, 2)}\n`,
  );
}
await copyFile(
  "crates/idealis-cli/target/wasm32-wasip1/release/idealis.wasm",
  join(assets, "idealis-cli.wasm"),
);
await copyFile("README.md", join(packageDir, "README.md"));
await copyFile("LICENSE", join(packageDir, "LICENSE"));
await cp("docs", join(packageDir, "docs"), { recursive: true });
