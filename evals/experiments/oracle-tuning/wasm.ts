import { readFile } from "node:fs/promises";
import { join } from "node:path";

export const readOracleWasm = async (
  root: string,
): Promise<Buffer<ArrayBuffer>> => {
  const release = "target/wasm32-unknown-unknown/release";
  for (const crate of ["roracle", "oracle"]) {
    try {
      return await readFile(
        join(root, "crates", crate, release, "roracle.wasm"),
      );
    } catch (error) {
      if (
        !(error instanceof Error && "code" in error && error.code === "ENOENT")
      )
        throw error;
    }
  }
  return readFile(join(root, "crates/oracle", release, "idealis_oracle.wasm"));
};
