import assert from "node:assert/strict";
import test from "node:test";
import { chromium, type Page } from "@playwright/test";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

type Row = "First" | "Second";
type Notice = "FirstSucceeded" | "Duplicate" | "SecondSucceeded";
type Result = "Idle" | "Committed" | "Aborted";
type Command =
  | { kind: "writeBatch"; policy: "Recover" | "Rollback" }
  | { kind: "reopen" };
type Batch = {
  result: Result;
  requests: { $sequence: Notice[] };
  committed: Row[];
  published: Row[];
  visibleAtConflict: boolean;
};
type Observation =
  | { kind: "reopened"; rows: Row[] }
  | ({ kind: "batch" } & Batch);

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/browser/indexeddb-request-error.model";

// An IndexedDB transaction can auto-commit between browser tasks. The entire
// write stays in one browser evaluation, with new requests queued in callbacks.
async function transactInPage({
  command,
  publishFirstSuccess,
}: {
  command: Command;
  publishFirstSuccess: boolean;
}): Promise<Observation> {
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open("request-error", 1);
    request.onupgradeneeded = () => {
      request.result.createObjectStore("rows").add("anchor", "Anchor");
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  const readRows = () =>
    new Promise<Row[]>((resolve, reject) => {
      const request = db.transaction("rows").objectStore("rows").getAllKeys();
      request.onsuccess = () => {
        if (!request.result.includes("Anchor")) {
          reject(new Error("existing row was lost"));
          return;
        }
        resolve(
          request.result.filter(
            (key): key is Row => key === "First" || key === "Second",
          ),
        );
      };
      request.onerror = () => reject(request.error);
    });
  try {
    if (command.kind === "reopen")
      return { kind: "reopened", rows: await readRows() };

    const notices: Notice[] = [];
    const applied = new Set<Row>();
    const published = new Set<Row>();
    let visibleAtConflict = false;
    const result = await new Promise<"Committed" | "Aborted">(
      (resolve, reject) => {
        const tx = db.transaction("rows", "readwrite");
        const store = tx.objectStore("rows");
        tx.oncomplete = () => {
          for (const row of applied) published.add(row);
          resolve("Committed");
        };
        tx.onabort = () => resolve("Aborted");
        const first = store.add("first", "First");
        first.onerror = () => reject(first.error);
        first.onsuccess = () => {
          notices.push("FirstSucceeded");
          applied.add("First");
          if (publishFirstSuccess) published.add("First");
          const duplicate = store.add("duplicate", "Anchor");
          duplicate.onsuccess = () =>
            reject(new Error("duplicate key was accepted"));
          duplicate.onerror = (event) => {
            if (duplicate.error?.name !== "ConstraintError") {
              reject(duplicate.error);
              return;
            }
            notices.push("Duplicate");
            visibleAtConflict = published.has("First");
            if (command.policy === "Recover") {
              event.preventDefault();
              const second = store.add("second", "Second");
              second.onsuccess = () => {
                notices.push("SecondSucceeded");
                applied.add("Second");
              };
              second.onerror = () => reject(second.error);
            }
          };
        };
      },
    );
    return {
      kind: "batch",
      result,
      requests: { $sequence: notices },
      committed: await readRows(),
      published: [...published],
      visibleAtConflict,
    };
  } finally {
    db.close();
  }
}

function openDriver(page: Page, publishFirstSuccess: boolean) {
  let batch: Batch = {
    result: "Idle",
    requests: { $sequence: [] },
    committed: [],
    published: [],
    visibleAtConflict: false,
  };
  let readBack: Row[] | null = null;
  return {
    async apply(command: Command) {
      const observation = await page.evaluate(transactInPage, {
        command,
        publishFirstSuccess,
      });
      if (observation.kind === "reopened") {
        readBack = observation.rows;
      } else {
        const { kind: _, ...snapshot } = observation;
        batch = snapshot;
      }
    },
    async observe() {
      return {
        ...batch,
        reopened: readBack !== null,
        readBack: readBack ?? [],
      };
    },
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test(
  "IndexedDB request success and handled errors do not publish before transaction completion",
  { timeout: 20_000 },
  async (t) => {
    const browser = await chromium.launch();
    t.after(() => browser.close());
    const cases = [
      {
        question: "ReadyToReopenAfterAbort",
        result: "Aborted",
        rows: [],
        notices: ["FirstSucceeded", "Duplicate"],
      },
      {
        question: "ReadyToReopenAfterCommit",
        result: "Committed",
        rows: ["First", "Second"],
        notices: ["FirstSucceeded", "Duplicate", "SecondSucceeded"],
      },
    ] as const;
    for (const expected of cases) {
      const prefix = await reachableScenario<Command>(
        checker,
        model,
        expected.question,
        { checker: "rprob" },
      );
      for (const publishFirstSuccess of [false, true]) {
        await t.test(
          `${expected.result} / ${publishFirstSuccess ? "publishes on request success" : "waits for transaction outcome"}`,
          async (t) => {
            const page = await browser.newPage();
            t.after(() => page.close());
            await page.route("https://indexeddb.test/", (route) =>
              route.fulfill({
                contentType: "text/html",
                body: "<title>IndexedDB test</title>",
              }),
            );
            await page.goto("https://indexeddb.test/");
            const driver = openDriver(page, publishFirstSuccess);
            const result = await recordReplay(
              checker,
              {
                ...prefix,
                commands: [...prefix.commands, { kind: "reopen" }],
              },
              driver,
            );
            if (publishFirstSuccess) {
              assert.equal(result.verdict, "mismatch");
              assert.equal(result.difference.step, 1);
              assert.partialDeepStrictEqual(result.difference.observed, {
                result: expected.result,
                committed: expected.rows,
                requests: { $sequence: expected.notices },
                published:
                  expected.result === "Aborted" ? ["First"] : expected.rows,
                visibleAtConflict: true,
              });
              const itf = JSON.parse(
                JSON.stringify(toReplayItf(result.recording)),
              );
              assert.deepEqual(
                await replayRecording(
                  checker,
                  fromReplayItf(itf),
                  driver.project,
                ),
                {
                  verdict: "mismatch",
                  difference: result.difference,
                },
              );
              return;
            }
            assert.equal(result.verdict, "matched");
            assert.deepEqual(await driver.observe(), {
              result: expected.result,
              requests: { $sequence: expected.notices },
              committed: expected.rows,
              published: expected.rows,
              visibleAtConflict: false,
              reopened: true,
              readBack: expected.rows,
            });
          },
        );
      }
    }
  },
);
