// CI test support. Each entry contains the checker's reply, so assertions still run on hits.
import { createHash, randomUUID } from "node:crypto";
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { deserialize, serialize } from "node:v8";

export function createResultCache<T>(
  directory: string | undefined,
  checker: string,
  complete: (reply: T) => boolean,
  logStats = false,
) {
  let hits = 0;
  let writes = 0;
  if (directory) {
    mkdirSync(directory, { recursive: true });
    if (logStats)
      process.once("exit", () => {
        if (hits || writes)
          console.log(
            `TLC results (${checker}): ${hits} hits, ${writes} writes`,
          );
      });
  }
  const keyFor = (input: unknown) =>
    createHash("sha256")
      .update(serialize([checker, input]))
      .digest("hex");
  const checksum = (key: string, bytes: Buffer) =>
    createHash("sha256").update(key).update(bytes).digest();
  const read = (key: string): T | undefined => {
    let bytes: Buffer;
    try {
      bytes = readFileSync(join(directory!, key));
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
      throw error;
    }
    const payload = bytes.subarray(32);
    if (!bytes.subarray(0, 32).equals(checksum(key, payload))) return;
    const reply: T = deserialize(payload);
    if (!complete(reply)) return;
    hits++;
    return reply;
  };
  const save = (key: string, reply: T) => {
    if (complete(reply)) {
      const payload = serialize(reply);
      const path = join(directory!, key);
      const temporary = path + "." + randomUUID();
      writeFileSync(
        temporary,
        Buffer.concat([checksum(key, payload), payload]),
      );
      renameSync(temporary, path);
      writes++;
    }
    return reply;
  };
  return {
    runSync(input: unknown, execute: () => T): T {
      if (!directory) return execute();
      const key = keyFor(input);
      return read(key) ?? save(key, execute());
    },
    async run(input: unknown, execute: () => Promise<T>): Promise<T> {
      if (!directory) return execute();
      const key = keyFor(input);
      return read(key) ?? save(key, await execute());
    },
  };
}
