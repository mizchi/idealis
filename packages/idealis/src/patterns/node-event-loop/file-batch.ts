import { readFile } from "node:fs";
import { nextTick } from "node:process";
import type { IoBatch, IoCallback, NextTickCallback } from "./adapter.js";

/** Schedule all three callbacks from the same file I/O callback. */
export function nodeFileIoBatch(file: string | URL): IoBatch {
  return fileIoBatch<IoCallback>(file, 3, (record) => {
    queueMicrotask(() => record("Microtask"));
    setImmediate(() => record("Immediate"));
    setTimeout(() => record("Timer"), 0);
  });
}

/** Register one next tick from I/O and another from inside the microtask queue. */
export function nodeFileNextTickBatch(
  file: string | URL,
): IoBatch<NextTickCallback> {
  return fileIoBatch<NextTickCallback>(file, 4, (record) => {
    queueMicrotask(() => {
      record("FirstMicrotask");
      nextTick(() => record("NestedNextTick"));
    });
    queueMicrotask(() => record("SecondMicrotask"));
    nextTick(() => record("IoNextTick"));
  });
}

function fileIoBatch<Callback extends string>(
  file: string | URL,
  callbackCount: number,
  schedule: (record: (name: Callback) => void) => void,
): IoBatch<Callback> {
  let observed: readonly Callback[] = [];
  return {
    async drain() {
      observed = await new Promise<Callback[]>((resolve, reject) => {
        readFile(file, (error) => {
          if (error) {
            reject(error);
            return;
          }
          const calls: Callback[] = [];
          schedule((name) => {
            calls.push(name);
            if (calls.length === callbackCount) resolve(calls);
          });
        });
      });
    },
    observe: () => observed,
  };
}
