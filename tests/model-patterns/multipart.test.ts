import assert from "node:assert/strict";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";
import { taskGate } from "./helpers/task-gate.js";

type Upload = "First" | "Next";
type Part = "Head" | "Tail";
type Command =
  | { kind: "begin" | "requestComplete" | "abort"; upload: Upload }
  | { kind: "uploadPart"; upload: Upload; part: Part }
  | { kind: "commit" };
type Fault = "partial" | "publish-after-abort";
type Complete = "Committed" | "NoSuchUpload";
const checkerPath = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/object-store/multipart.ideal";
const completeBodies = {
  First: "First-Head;First-Tail",
  Next: "Next-Head;Next-Tail",
};

async function multipart(
  t: TestContext,
  fault?: Fault,
): Promise<ObservationAdapter<Command>> {
  const uploads = new Map<string, Map<string, string>>();
  let body: string | null = null;
  let result: Complete | null = null;
  let queued: ReturnType<typeof taskGate>;
  let proceed: ReturnType<typeof taskGate> | undefined;
  let response: Promise<Response> | undefined;

  const server = createServer(async (request, reply) => {
    const [, upload, part] = (request.url ?? "").split("/");
    const send = (status: number, data?: string) => {
      reply.writeHead(status).end(data);
    };
    if (request.method === "GET" && upload === "object")
      return send(body === null ? 404 : 200, body ?? undefined);
    if (!upload) return send(400);
    if (request.method === "POST" && part === undefined) {
      uploads.set(upload, new Map());
      return send(201);
    }
    const parts = uploads.get(upload);
    if (!parts) return send(404, "NoSuchUpload");
    if (request.method === "DELETE") {
      uploads.delete(upload);
      return send(204);
    }
    if (request.method === "PUT" && part) {
      const chunks: Buffer[] = [];
      for await (const chunk of request) chunks.push(chunk);
      const data = Buffer.concat(chunks).toString();
      parts.set(part, data);
      if (fault === "partial") body = data;
      return send(200);
    }
    const head = parts.get("Head");
    const tail = parts.get("Tail");
    if (
      request.method !== "POST" ||
      part !== "complete" ||
      head === undefined ||
      tail === undefined
    )
      return send(400);
    const inFlight = proceed?.promise;
    queued.open();
    await inFlight;
    if (uploads.get(upload) !== parts && fault !== "publish-after-abort")
      return send(404, "NoSuchUpload");
    body = [head, tail].join(";");
    uploads.delete(upload);
    return send(200);
  });

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  t.after(async () => {
    proceed?.open();
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  const origin = `http://127.0.0.1:${address.port}`;
  return {
    async apply(command) {
      if (command.kind === "commit") {
        assert.ok(response && proceed);
        proceed.open();
        const settled = await response;
        if (settled.status === 404)
          assert.equal(await settled.text(), "NoSuchUpload");
        else assert.equal(settled.status, 200);
        result = settled.status === 200 ? "Committed" : "NoSuchUpload";
        return;
      }
      const path = `${origin}/${command.upload}`;
      if (command.kind === "requestComplete") {
        result = null;
        queued = taskGate();
        proceed = taskGate();
        response = fetch(`${path}/complete`, { method: "POST" });
        const early = await Promise.race([queued.promise, response]);
        assert.equal(early, undefined, "completion ended before commit");
        return;
      }
      const settled =
        command.kind === "uploadPart"
          ? await fetch(`${path}/${command.part}`, {
              method: "PUT",
              body: `${command.upload}-${command.part}`,
            })
          : await fetch(path, {
              method: command.kind === "begin" ? "POST" : "DELETE",
            });
      assert.equal(
        settled.status,
        command.kind === "begin" ? 201 : command.kind === "abort" ? 204 : 200,
      );
    },
    async observe() {
      const received = await fetch(`${origin}/object`);
      assert.ok(received.status === 200 || received.status === 404);
      return { body: received.ok ? await received.text() : null, result };
    },
    project: (state) => ({
      body:
        state.visible === null ? null : completeBodies[state.visible as Upload],
      result: state.result,
    }),
  };
}

test("multipart completion publishes only assembled data for a live upload", async (t) => {
  const pending = await reachableScenario<Command>(
    checkerPath,
    model,
    "AbortedWithCompletionWaiting",
  );
  assert.equal(pending.commands.length, 5);
  const withCommit = {
    ...pending,
    commands: [...pending.commands, { kind: "commit" }],
  } satisfies typeof pending;
  for (const fault of [undefined, "partial", "publish-after-abort"] as const) {
    const run = await recordReplay(
      checkerPath,
      withCommit,
      await multipart(t, fault),
    );
    if (fault) {
      assert.equal(run.verdict, "mismatch");
      assert.equal(run.difference.step, fault === "partial" ? 2 : 6);
      if (fault === "publish-after-abort") {
        assert.deepEqual(run.difference.observed, {
          body: completeBodies.First,
          result: "Committed",
        });
      }
    } else {
      assert.equal(run.verdict, "matched");
      assert.deepEqual(run.recording.observations.at(-1), {
        body: null,
        result: "NoSuchUpload",
      });
    }
  }

  const reopen = {
    ...pending,
    commands: [
      ...pending.commands,
      { kind: "begin", upload: "Next" },
      { kind: "uploadPart", upload: "Next", part: "Tail" },
      { kind: "uploadPart", upload: "Next", part: "Head" },
      { kind: "commit" },
      { kind: "requestComplete", upload: "Next" },
      { kind: "commit" },
    ],
  } satisfies typeof pending;
  const reopened = await recordReplay(checkerPath, reopen, await multipart(t));
  assert.equal(reopened.verdict, "matched");
  assert.deepEqual(reopened.recording.observations[9], {
    body: null,
    result: "NoSuchUpload",
  });
  assert.deepEqual(reopened.recording.observations.at(-1), {
    body: completeBodies.Next,
    result: "Committed",
  });

  const replacement = await reachableScenario<Command>(
    checkerPath,
    model,
    "ReplacementPartsArePrivate",
  );
  const publish = {
    ...replacement,
    commands: [
      ...replacement.commands,
      { kind: "requestComplete", upload: "Next" },
      { kind: "commit" },
    ],
  } satisfies typeof replacement;
  const run = await recordReplay(checkerPath, publish, await multipart(t));
  assert.equal(run.verdict, "matched");
  assert.deepEqual(run.recording.observations.at(-3), {
    body: completeBodies.First,
    result: "Committed",
  });
  assert.deepEqual(run.recording.observations.at(-1), {
    body: completeBodies.Next,
    result: "Committed",
  });
});
