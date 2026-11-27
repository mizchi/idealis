import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

type Part = "Head" | "Tail";
type Tag = "Old" | "New";
type Status = "Partial" | "Full";
type Command =
  | { kind: "replace" }
  | { kind: "request"; part: Part; mode: "Plain" | "Resume" }
  | { kind: "accept"; part: Part };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/cache-refresh/range-assembly.model";
const tags = { Old: '"old"', New: '"new"' };
const bodies = { Old: "A0A1", New: "B0B1" };
const parts = ["Head", "Tail"] as const;
const ranges = {
  Head: { start: 0, end: 1, spec: "0-1" },
  Tail: { start: 2, end: 3, spec: "2-3" },
};

function slice(body: string, part: Part) {
  const { start, end } = ranges[part];
  return body.slice(start, end + 1);
}

function tagForPart(body: string | null, part: Part): Tag | null {
  if (body === null) return null;
  if (body === slice(bodies.Old, part)) return "Old";
  assert.equal(body, slice(bodies.New, part));
  return "New";
}

async function openDownload(t: TestContext, acceptMismatchedPartial = false) {
  let origin: Tag = "Old";
  let cachedTag: Tag | null = null;
  const chunks: Record<Part, string | null> = { Head: null, Tail: null };
  const incoming = new Map<
    Part,
    { tag: Tag; status: Status; send: () => void }
  >();
  const requests = new Map<Part, Promise<Response>>();
  const arrivals = new EventEmitter();
  const replies: number[] = [];
  const server = createServer((request, response) => {
    const range = request.headers.range;
    const part = parts.find((part) => range === `bytes=${ranges[part].spec}`);
    assert.ok(part);
    const tag = origin;
    const validator = request.headers["if-range"];
    const partial = validator === undefined || validator === tags[tag];
    incoming.set(part, {
      tag,
      status: partial ? "Partial" : "Full",
      send() {
        response.setHeader("ETag", tags[tag]);
        if (partial) {
          response.writeHead(206, {
            "Content-Range": `bytes ${ranges[part].spec}/4`,
          });
          response.end(slice(bodies[tag], part));
        } else {
          response.writeHead(200);
          response.end(bodies[tag]);
        }
      },
    });
    arrivals.emit(part);
  });
  server.listen(0, "127.0.0.1");
  t.after(async () => {
    const closed = once(server, "close");
    server.close();
    server.closeAllConnections();
    await closed;
  });
  await once(server, "listening");
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  const url = `http://127.0.0.1:${address.port}/asset`;

  function describeReply(part: Part) {
    const reply = incoming.get(part);
    return reply ? { tag: reply.tag, status: reply.status } : null;
  }

  return {
    replies,
    async apply(command: Command) {
      switch (command.kind) {
        case "replace":
          origin = "New";
          break;
        case "request": {
          const { part, mode } = command;
          const headers = new Headers({ Range: `bytes=${ranges[part].spec}` });
          if (mode === "Resume") {
            assert.ok(cachedTag);
            headers.set("If-Range", tags[cachedTag]);
          }
          const arrived = once(arrivals, part, {
            signal: AbortSignal.timeout(5_000),
          });
          const response = fetch(url, { headers });
          void response.catch(() => undefined);
          requests.set(part, response);
          await arrived;
          break;
        }
        case "accept": {
          const { part } = command;
          const incomingReply = incoming.get(part);
          const request = requests.get(part);
          assert.ok(incomingReply && request);
          incomingReply.send();
          incoming.delete(part);
          requests.delete(part);
          const response = await request;
          replies.push(response.status);
          const etag = response.headers.get("etag");
          const tag = etag === tags.Old ? "Old" : "New";
          assert.equal(etag, tags[tag]);
          const contentRange = response.headers.get("content-range");
          const body = await response.text();
          if (response.status === 200) {
            assert.equal(contentRange, null);
            assert.equal(Buffer.byteLength(body), 4);
            cachedTag = tag;
            for (const part of parts) chunks[part] = slice(body, part);
          } else {
            assert.equal(response.status, 206);
            assert.equal(contentRange, `bytes ${ranges[part].spec}/4`);
            assert.equal(Buffer.byteLength(body), 2);
            if (
              cachedTag === null ||
              cachedTag === tag ||
              acceptMismatchedPartial
            ) {
              cachedTag = tag;
              chunks[part] = body;
            }
          }
          break;
        }
      }
    },
    async observe() {
      return {
        origin,
        cachedTag,
        pending: { Head: describeReply("Head"), Tail: describeReply("Tail") },
        stored: {
          Head: tagForPart(chunks.Head, "Head"),
          Tail: tagForPart(chunks.Tail, "Tail"),
        },
      };
    },
    project: (observation: Readonly<Record<string, unknown>>) => observation,
  };
}

test(
  "HTTP range reassembly keeps each stored piece from the same representation",
  { timeout: 20_000 },
  async (t) => {
    const cases = [
      {
        name: "resume against the same tag completes with two 206 responses",
        question: "OldHeadStored",
        suffix: [
          { kind: "request", part: "Tail", mode: "Resume" },
          { kind: "accept", part: "Tail" },
        ],
        statuses: [206, 206],
        stored: { Head: "Old", Tail: "Old" },
        cachedTag: "Old",
      },
      {
        name: "a different 206 cannot complete the old body",
        question: "NewPartialWaiting",
        suffix: [{ kind: "accept", part: "Tail" }],
        statuses: [206, 206],
        stored: { Head: "Old", Tail: null },
        cachedTag: "Old",
        mixed: { Head: "Old", Tail: "New" },
      },
      {
        name: "If-Range falls back to a full replacement before a delayed old 206",
        question: "NewFullAndOldPartialWaiting",
        suffix: [
          { kind: "accept", part: "Tail" },
          { kind: "accept", part: "Head" },
        ],
        statuses: [206, 200, 206],
        stored: { Head: "New", Tail: "New" },
        cachedTag: "New",
        mixed: { Head: "Old", Tail: "New" },
      },
    ] as const satisfies readonly {
      name: string;
      question: string;
      suffix: readonly Command[];
      statuses: readonly number[];
      stored: Record<Part, Tag | null>;
      cachedTag: Tag;
      mixed?: Record<Part, Tag>;
    }[];

    for (const scenario of cases) {
      const prefix = await reachableScenario<Command>(
        checker,
        model,
        scenario.question,
        {
          checker: "rprob",
        },
      );
      const replay = {
        ...prefix,
        commands: [...prefix.commands, ...scenario.suffix],
      };
      await t.test(scenario.name, async (t) => {
        const driver = await openDownload(t);
        const result = await recordReplay(checker, replay, driver);
        assert.equal(result.verdict, "matched");
        assert.deepEqual(driver.replies, scenario.statuses);
        assert.partialDeepStrictEqual(result.recording.observations.at(-1), {
          stored: scenario.stored,
          cachedTag: scenario.cachedTag,
        });
      });
      if ("mixed" in scenario) {
        await t.test(
          `${scenario.name} / accepting a mismatched 206 mixes bytes`,
          async (t) => {
            const driver = await openDownload(t, true);
            const result = await recordReplay(checker, replay, driver);
            assert.equal(result.verdict, "mismatch");
            assert.equal(result.difference.step, replay.commands.length);
            assert.deepEqual(driver.replies, scenario.statuses);
            assert.partialDeepStrictEqual(result.difference.expected, {
              stored: scenario.stored,
            });
            assert.partialDeepStrictEqual(result.difference.observed, {
              stored: scenario.mixed,
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
              { verdict: "mismatch", difference: result.difference },
            );
          },
        );
      }
    }
  },
);
