import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test, { type TestContext } from "node:test";
import {
  recordReplay,
  transitionScenario,
} from "../../examples/model-patterns/replay.js";

type Writer = "Original" | "Replacement";
type Request = "Primary" | "Fresh";
type Body = "A" | "B";
type Receipt = { body: Body; author: Writer };
type Reply =
  | { tag: "Silent" | "Conflict" | "Fenced" }
  | { tag: "Stored"; value: Receipt };
type Command =
  | { kind: "promote" | "loseReply" }
  | { kind: "submit"; writer: Writer; request: Request; body: Body };
type Fault = "owner-first" | "writer-scoped" | "key-only";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/crash-recovery/recovery-receipt.model";

async function openRecovery(t: TestContext, fault?: Fault) {
  const dir = await mkdtemp(join(tmpdir(), "idealis-recovery-receipt-"));
  const path = join(dir, "publication.sqlite");
  const original = new DatabaseSync(path);
  const replacement = new DatabaseSync(path);
  t.after(async () => {
    original.close();
    replacement.close();
    await rm(dir, { recursive: true, force: true });
  });
  original.exec(`
    CREATE TABLE authority (owner TEXT NOT NULL);
    INSERT INTO authority VALUES ('Original');
    CREATE TABLE published (seq INTEGER PRIMARY KEY, body TEXT NOT NULL);
    CREATE TABLE receipts (
      request TEXT NOT NULL, body TEXT NOT NULL, author TEXT NOT NULL,
      PRIMARY KEY (${fault === "writer-scoped" ? "request, author" : "request"})
    );
  `);

  let reply: Reply = { tag: "Silent" };
  function owner(db: DatabaseSync) {
    return db.prepare("SELECT owner FROM authority").get()?.owner as Writer;
  }

  function submit({
    writer,
    request,
    body,
  }: Extract<Command, { kind: "submit" }>): Reply {
    const db = writer === "Original" ? original : replacement;
    db.exec("BEGIN IMMEDIATE");
    try {
      function resolve(): Reply {
        if (fault === "owner-first" && owner(db) !== writer)
          return { tag: "Fenced" };
        const receipt = (
          fault === "writer-scoped"
            ? db
                .prepare(
                  "SELECT body, author FROM receipts WHERE request = ? AND author = ?",
                )
                .get(request, writer)
            : db
                .prepare("SELECT body, author FROM receipts WHERE request = ?")
                .get(request)
        ) as Receipt | undefined;
        if (receipt) {
          return receipt.body === body || fault === "key-only"
            ? {
                tag: "Stored",
                value: { body: receipt.body, author: receipt.author },
              }
            : { tag: "Conflict" };
        }
        if (owner(db) !== writer) return { tag: "Fenced" };
        db.prepare("INSERT INTO published(body) VALUES (?)").run(body);
        db.prepare("INSERT INTO receipts VALUES (?, ?, ?)").run(
          request,
          body,
          writer,
        );
        return { tag: "Stored", value: { body, author: writer } };
      }
      const resolved = resolve();
      db.exec("COMMIT");
      return resolved;
    } catch (error) {
      db.exec("ROLLBACK");
      throw error;
    }
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "promote":
          replacement.exec("UPDATE authority SET owner = 'Replacement'");
          break;
        case "loseReply":
          reply = { tag: "Silent" };
          break;
        case "submit":
          reply = submit(command);
          break;
      }
    },
    async observe() {
      const receipts = replacement.prepare(
        "SELECT body, author FROM receipts WHERE request = ? ORDER BY rowid DESC LIMIT 1",
      );
      function read(request: Request) {
        const receipt = receipts.get(request);
        return receipt ? { body: receipt.body, author: receipt.author } : null;
      }
      return {
        owner: owner(replacement),
        visible:
          replacement
            .prepare("SELECT body FROM published ORDER BY seq DESC LIMIT 1")
            .get()?.body ?? null,
        writes: replacement
          .prepare("SELECT COUNT(*) AS count FROM published")
          .get()?.count,
        receipts: { Primary: read("Primary"), Fresh: read("Fresh") },
        reply,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "a retry after takeover reuses a durable reply without reopening the write",
  { timeout: 30_000 },
  async (t) => {
    for (const [writer, body, fault] of [
      ["Original", "A", "owner-first"],
      ["Replacement", "A", "writer-scoped"],
      ["Replacement", "B", "key-only"],
    ] as const) {
      const scenario = await transitionScenario<Command>(
        checker,
        model,
        "submit",
        `writer = ${writer} and request = Primary and body = ${body} and
       before(owner) = Replacement and before(writes) = 1 and
       before(Primary.receipts) = Receipt { body: A, author: Original } and
       unchanged(writes)`,
      );
      assert.deepEqual(
        scenario.commands.map((command) => command.kind),
        ["submit", "loseReply", "promote", "submit"],
      );

      await t.test(`replay ${writer} ${body}`, async (t) => {
        const run = await recordReplay(
          checker,
          scenario,
          await openRecovery(t),
        );
        assert.equal(run.verdict, "matched");
        assert.deepEqual(run.recording.observations.at(-1), {
          owner: "Replacement",
          visible: "A",
          writes: 1,
          receipts: { Primary: { body: "A", author: "Original" }, Fresh: null },
          reply:
            body === "A"
              ? { tag: "Stored", value: { body: "A", author: "Original" } }
              : { tag: "Conflict" },
        });
      });

      await t.test(fault, async (t) => {
        const run = await recordReplay(
          checker,
          scenario,
          await openRecovery(t, fault),
        );
        assert.equal(run.verdict, "mismatch");
        assert.equal(run.difference.step, 4);
        const observed = run.difference.observed as {
          writes: number;
          reply: Reply;
        };
        assert.equal(
          observed.reply.tag,
          fault === "owner-first" ? "Fenced" : "Stored",
        );
        assert.equal(observed.writes, fault === "writer-scoped" ? 2 : 1);
      });
    }

    await t.test(
      "a historical reply survives a later publication",
      async (t) => {
        const scenario = await transitionScenario<Command>(
          checker,
          model,
          "submit",
          `writer = Replacement and request = Primary and body = A and
         before(Primary.receipts) = Receipt { body: A, author: Original } and
         before(Fresh.receipts) = Receipt { body: B, author: Replacement } and
         before(visible) = B and unchanged(writes)`,
        );
        const run = await recordReplay(
          checker,
          scenario,
          await openRecovery(t),
        );
        assert.equal(run.verdict, "matched");
        assert.deepEqual(run.recording.observations.at(-1), {
          owner: "Replacement",
          visible: "B",
          writes: 2,
          receipts: {
            Primary: { body: "A", author: "Original" },
            Fresh: { body: "B", author: "Replacement" },
          },
          reply: { tag: "Stored", value: { body: "A", author: "Original" } },
        });
      },
    );

    await t.test(
      "a stale writer still cannot create a new result",
      async (t) => {
        const scenario = await transitionScenario<Command>(
          checker,
          model,
          "submit",
          "writer = Original and request = Fresh and after(reply) = Fenced()",
        );
        const run = await recordReplay(
          checker,
          {
            ...scenario,
            commands: [
              ...scenario.commands,
              { kind: "loseReply" },
              {
                kind: "submit",
                writer: "Replacement",
                request: "Fresh",
                body: "B",
              },
            ],
          },
          await openRecovery(t),
        );
        assert.equal(run.verdict, "matched");
        assert.deepEqual(run.recording.observations.at(-1), {
          owner: "Replacement",
          visible: "B",
          writes: 1,
          receipts: {
            Primary: null,
            Fresh: { body: "B", author: "Replacement" },
          },
          reply: {
            tag: "Stored",
            value: { body: "B", author: "Replacement" },
          },
        });
      },
    );
  },
);
