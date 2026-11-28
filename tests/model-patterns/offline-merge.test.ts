import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Editor = "Alice" | "Bob";
type Setting = "Original" | "Remote" | "Local";
type Document = { readonly title: Setting; readonly label: Setting };
type Result = "Merged" | "Conflict";
type Command =
  | { readonly kind: "checkout"; readonly editor: Editor }
  | {
      readonly kind: "submit";
      readonly editor: Editor;
      readonly edit: Document;
    };
type Fault = "none" | "replace-document" | "ignore-conflict" | "partial-commit";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/offline-merge/merge.model";

test("offline edits merge against the saved base and reject conflicting updates atomically", async (t) => {
  const base = await reachableScenario<Command>(
    checker,
    model,
    "BobHasStaleBase",
  );
  assert.equal(base.commands.length, 3);
  const cases = [
    {
      name: "disjoint",
      edit: { title: "Original", label: "Local" },
      expected: { doc: { title: "Remote", label: "Local" }, result: "Merged" },
      faults: ["replace-document"],
    },
    {
      name: "same value",
      edit: { title: "Remote", label: "Local" },
      expected: { doc: { title: "Remote", label: "Local" }, result: "Merged" },
      faults: [],
    },
    {
      name: "conflict",
      edit: { title: "Local", label: "Local" },
      expected: {
        doc: { title: "Remote", label: "Original" },
        result: "Conflict",
      },
      faults: ["ignore-conflict", "partial-commit"],
    },
  ] as const;
  for (const scenario of cases) {
    const faults: readonly Fault[] = ["none", ...scenario.faults];
    for (const fault of faults) {
      await t.test(`${scenario.name} / ${fault}`, async () => {
        using db = new DatabaseSync(":memory:");
        db.exec(`
          CREATE TABLE live (title TEXT NOT NULL, label TEXT NOT NULL);
          INSERT INTO live VALUES ('Original', 'Original');
          CREATE TABLE bases (editor TEXT PRIMARY KEY, title TEXT NOT NULL, label TEXT NOT NULL);
        `);
        const responses = new Map<Editor, Result>();
        const load = () => {
          const row = db.prepare("SELECT title, label FROM live").get();
          assert.ok(row);
          return { title: row.title, label: row.label };
        };
        const write = (doc: Document) =>
          db
            .prepare("UPDATE live SET title = ?, label = ?")
            .run(doc.title, doc.label);
        const command: Command = {
          kind: "submit",
          editor: "Bob",
          edit: scenario.edit,
        };
        const result = await recordReplay<Command>(
          checker,
          { ...base, commands: [...base.commands, command] },
          {
            async apply(input) {
              if (input.kind === "checkout") {
                db.prepare(
                  "INSERT INTO bases SELECT ?, title, label FROM live",
                ).run(input.editor);
                return;
              }
              const original = db
                .prepare("SELECT title, label FROM bases WHERE editor = ?")
                .get(input.editor);
              assert.ok(original);
              const live = load();
              const patch: Partial<Record<keyof Document, Setting>> = {};
              let conflict = false;
              for (const key of ["title", "label"] as const) {
                const edited = input.edit[key];
                if (edited === original[key]) continue;
                if (
                  live[key] !== original[key] &&
                  live[key] !== edited &&
                  fault !== "ignore-conflict"
                ) {
                  conflict = true;
                } else {
                  patch[key] = edited;
                }
              }
              const merged = { ...live, ...patch } as Document;
              if (conflict) {
                if (fault === "partial-commit") write(merged);
                responses.set(input.editor, "Conflict");
                return;
              }
              write(fault === "replace-document" ? input.edit : merged);
              responses.set(input.editor, "Merged");
            },
            async observe() {
              return { doc: load(), result: responses.get("Bob") ?? null };
            },
            project: ({ live, results }) => ({
              doc: live,
              result: (results as Record<Editor, unknown>).Bob,
            }),
          },
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(
            result.recording.observations.at(-1),
            scenario.expected,
          );
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(result.difference, {
            step: 4,
            command,
            expected: scenario.expected,
            observed:
              fault === "replace-document"
                ? {
                    doc: { title: "Original", label: "Local" },
                    result: "Merged",
                  }
                : fault === "ignore-conflict"
                  ? {
                      doc: { title: "Local", label: "Local" },
                      result: "Merged",
                    }
                  : {
                      doc: { title: "Remote", label: "Local" },
                      result: "Conflict",
                    },
          });
        }
      });
    }
  }
});
