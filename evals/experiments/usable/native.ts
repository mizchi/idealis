import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import type { PortableCommand } from "../../../packages/idealis/src/contracts.js";

export const startNative = (
  path = "crates/idealis-core/target/release/idealis-core",
) => {
  const child = spawn(path, [], {
    stdio: ["pipe", "pipe", "pipe"],
  });
  const pending: {
    resolve: (value: unknown) => void;
    reject: (error: Error) => void;
    text: boolean;
  }[] = [];
  let stderr = "";
  let stopped = false;
  const completion = new Promise<number | null>((resolve) =>
    child.once("close", (code) => {
      stopped = true;
      for (const request of pending.splice(0))
        request.reject(new Error(`Native process closed: ${code} ${stderr}`));
      resolve(code);
    }),
  );
  child.on("error", (error) => {
    stopped = true;
    for (const request of pending.splice(0)) request.reject(error);
  });
  child.stderr.on("data", (chunk) => {
    stderr += String(chunk);
  });
  const lines = createInterface({ input: child.stdout });
  lines.on("line", (line) => {
    const request = pending.shift();
    if (!request) return;
    try {
      request.resolve(request.text ? line : JSON.parse(line));
    } catch (error) {
      request.reject(error instanceof Error ? error : new Error(String(error)));
    }
  });
  const send = (command: PortableCommand, text: boolean): Promise<unknown> =>
    new Promise((resolve, reject) => {
      if (stopped) {
        reject(new Error("Native process stopped"));
        return;
      }
      pending.push({ resolve, reject, text });
      child.stdin.write(`${JSON.stringify(command)}\n`);
    });
  return {
    request: (command: PortableCommand): Promise<unknown> =>
      send(command, false),
    requestText: async (command: PortableCommand): Promise<string> =>
      String(await send(command, true)),
    close: async () => {
      child.stdin.end();
      const code = await completion;
      lines.close();
      if (code !== 0)
        throw new Error(`Native process exited ${code}: ${stderr}`);
    },
  };
};
