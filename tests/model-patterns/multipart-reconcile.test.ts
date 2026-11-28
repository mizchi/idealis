import assert from "node:assert/strict";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  observationPairScenarios,
  recordReplay,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";

type Command =
  | { kind: "finish"; committed: boolean }
  | { kind: "overwrite" | "readCurrent" | "scanHistory" };
type Fault = "match-body" | "reject-other-version";
type Result = "Unknown" | "Confirmed" | "Rejected";
interface Version {
  id: string;
  operation: string;
  body: string;
}

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/object-store/multipart-reconcile.ideal";
const ids = {
  Original: "v-original",
  Mine: "v-completion",
  Other: "v-overwrite",
};
const expectedBody = "assembled payload";
const operations = {
  Original: "original",
  Mine: "complete-57",
  Other: "different-operation",
};

async function reconciliation(
  t: TestContext,
  fault?: Fault,
): Promise<ObservationAdapter<Command>> {
  let current: Version = {
    id: ids.Original,
    operation: operations.Original,
    body: "previous payload",
  };
  const versions = new Map([[current.id, current]]);
  let lastRead: Version | null = null;
  let result: Result = "Unknown";
  let proof: string | null = null;

  function write(version: Version) {
    versions.set(version.id, version);
    current = version;
  }

  const server = createServer((request, reply) => {
    const url = new URL(request.url ?? "/", "http://localhost");
    if (url.pathname === "/complete" && request.method === "POST") {
      if (url.searchParams.get("committed") === "true")
        write({
          id: ids.Mine,
          operation: operations.Mine,
          body: expectedBody,
        });
      reply.writeHead(200, { "Content-Type": "application/xml" });
      reply.flushHeaders();
      reply.write("<CompleteMultipartUploadResult>", () => reply.destroy());
    } else if (url.pathname === "/object" && request.method === "PUT") {
      write({
        id: ids.Other,
        operation: operations.Other,
        body: expectedBody,
      });
      reply.writeHead(204).end();
    } else if (url.pathname === "/versions" && request.method === "GET") {
      reply.setHeader("Content-Type", "application/json");
      reply.end(JSON.stringify([...versions.keys()].reverse()));
    } else if (url.pathname === "/object" && request.method === "GET") {
      const id = url.searchParams.get("versionId");
      const version = id === null ? current : versions.get(id);
      if (version === undefined) {
        reply.writeHead(404).end();
        return;
      }
      reply.writeHead(200, {
        "x-amz-version-id": version.id,
        "x-amz-meta-operation": version.operation,
      });
      reply.end(version.body);
    } else reply.writeHead(404).end();
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  t.after(async () => {
    await new Promise<void>((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
      server.closeAllConnections();
    });
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  const origin = `http://127.0.0.1:${address.port}`;

  async function readObject(id?: string): Promise<Version> {
    const url = new URL("/object", origin);
    if (id !== undefined) url.searchParams.set("versionId", id);
    const response = await fetch(url);
    assert.equal(response.status, 200);
    const version = response.headers.get("x-amz-version-id");
    const operation = response.headers.get("x-amz-meta-operation");
    assert.ok(version && operation);
    if (id !== undefined) assert.equal(version, id);
    return { id: version, operation, body: await response.text() };
  }

  function confirm(id: string) {
    result = "Confirmed";
    proof = id;
  }

  return {
    async apply(command) {
      switch (command.kind) {
        case "finish": {
          const response = await fetch(
            `${origin}/complete?committed=${command.committed}`,
            { method: "POST" },
          );
          assert.equal(response.status, 200);
          await assert.rejects(response.text(), TypeError);
          break;
        }
        case "overwrite":
          assert.equal(
            (await fetch(`${origin}/object`, { method: "PUT" })).status,
            204,
          );
          break;
        case "readCurrent": {
          const version = await readObject();
          lastRead = version;
          if (
            version.operation === operations.Mine ||
            (fault === "match-body" && version.body === expectedBody)
          )
            confirm(version.id);
          else if (fault === "reject-other-version") result = "Rejected";
          break;
        }
        case "scanHistory": {
          const listing = await fetch(`${origin}/versions`);
          assert.equal(listing.status, 200);
          for (const id of (await listing.json()) as string[]) {
            const version = await readObject(id);
            if (version.operation === operations.Mine) {
              confirm(version.id);
              break;
            }
          }
          break;
        }
      }
    },
    async observe() {
      return { lastRead, result, proof };
    },
    project: (state) => ({
      lastRead:
        state.observed === null
          ? null
          : {
              id: ids[state.observed as keyof typeof ids],
              operation: operations[state.observed as keyof typeof operations],
              body:
                state.observed === "Original"
                  ? "previous payload"
                  : expectedBody,
            },
      result: state.result,
      proof: state.proof === null ? null : ids[state.proof as keyof typeof ids],
    }),
  };
}

test("a matching current object is ambiguous after a lost multipart completion", async (t) => {
  const sameCurrent = {
    lastRead: {
      id: ids.Other,
      operation: operations.Other,
      body: expectedBody,
    },
    result: "Unknown",
    proof: null,
  };
  for (const [question, committed, fault] of [
    ["OverwrittenAfterCommit", true, "reject-other-version"],
    ["OverwrittenWithoutCommit", false, "match-body"],
  ] as const) {
    const base = await reachableScenario<Command>(checker, model, question);
    const scenario = {
      ...base,
      commands: [...base.commands, { kind: "scanHistory" }],
    } satisfies typeof base;
    const matched = await recordReplay(
      checker,
      scenario,
      await reconciliation(t),
    );
    assert.equal(matched.verdict, "matched");
    assert.deepEqual(matched.recording.observations[3], sameCurrent);
    assert.deepEqual(
      matched.recording.observations[4],
      committed
        ? { ...sameCurrent, result: "Confirmed", proof: ids.Mine }
        : sameCurrent,
    );

    const wrong = await recordReplay(
      checker,
      scenario,
      await reconciliation(t, fault),
    );
    assert.equal(wrong.verdict, "mismatch");
    assert.equal(wrong.difference.step, 3);
    assert.deepEqual(wrong.difference.expected, sameCurrent);
    assert.deepEqual(
      wrong.difference.observed,
      committed
        ? { ...sameCurrent, result: "Rejected" }
        : { ...sameCurrent, result: "Confirmed", proof: ids.Other },
    );
  }
});

test("one paired witness yields two independent HTTP executions with the same latest observation", async (t) => {
  type Run = "Left" | "Right";
  const pairedModel =
    "examples/model-patterns/object-store/multipart-observation-pair.ideal";
  await assert.rejects(
    reachableScenario(checker, pairedModel, "EqualHistoryWithDifferentCommit"),
    /EqualHistoryWithDifferentCommit: unreachable/,
  );
  for (const [question, depth] of [
    ["LatestIsAmbiguous", 6],
    ["HistoryDistinguishes", 8],
  ] as const) {
    const witness = await reachableScenario<Command & { run: Run }>(
      checker,
      pairedModel,
      question,
    );
    assert.equal(witness.commands.length, depth);
    const histories = await Promise.all(
      (["Left", "Right"] as const).map(async (run) => {
        const commands = witness.commands
          .filter((command) => command.run === run)
          .map((command): Command =>
            command.kind === "finish"
              ? { kind: "finish", committed: command.committed }
              : { kind: command.kind },
          );
        assert.deepEqual(commands[0], {
          kind: "finish",
          committed: run === "Left",
        });
        const result = await recordReplay(
          checker,
          {
            model,
            config: {},
            initial: {
              finished: false,
              current: "Original",
              versions: ["Original"],
              observed: null,
              result: "Unknown",
              proof: null,
              scanned: false,
            },
            commands,
          },
          await reconciliation(t),
        );
        assert.equal(result.verdict, "matched");
        return result.recording.observations;
      }),
    );
    const [left, right] = histories;
    assert.ok(left && right);
    const shared = {
      lastRead: {
        id: ids.Other,
        operation: operations.Other,
        body: expectedBody,
      },
      result: "Unknown",
      proof: null,
    };
    assert.deepEqual(left[3], shared);
    assert.deepEqual(left.slice(0, 4), right.slice(0, 4));
    if (question === "HistoryDistinguishes") {
      assert.deepEqual(left.at(-1), {
        ...shared,
        result: "Confirmed",
        proof: ids.Mine,
      });
      assert.deepEqual(right.at(-1), shared);
    }
  }
});

test("grouping snapshots of the single model yields the same HTTP ambiguity", async (t) => {
  const [first, second] = await observationPairScenarios<Command>(
    checker,
    model,
    {
      at: "LatestObservation",
      observe: ["observed", "result", "proof"],
      distinguish: ["versions"],
    },
  );
  const [left, right] = await Promise.all([
    recordReplay(checker, first, await reconciliation(t)),
    recordReplay(checker, second, await reconciliation(t)),
  ]);
  assert.equal(left.verdict, "matched");
  assert.equal(right.verdict, "matched");
  assert.equal(first.commands.length + second.commands.length, 6);
  assert.notDeepEqual(first.commands[0], second.commands[0]);
  assert.deepEqual(left.recording.observations, right.recording.observations);
});
