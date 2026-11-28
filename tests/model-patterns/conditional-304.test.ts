import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Tag = "Old" | "New";
type Policy = "Initial" | "Refreshed" | "Replacement";
type Entry = { tag: Tag; body: "BodyA" | "BodyB"; policy: Policy };
type Command = { kind: "start" | "replace" | "evict" | "finish" };
type Fault = "update-unmatched" | "resurrect";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/cache-refresh/conditional-304.model";
const original: Entry = { tag: "Old", body: "BodyA", policy: "Initial" };
const replacement: Entry = { tag: "New", body: "BodyB", policy: "Replacement" };
const tags = { Old: '"old"', New: '"new"' };
const policies = {
  Initial: "max-age=0",
  Refreshed: "max-age=60",
  Replacement: "max-age=10",
};

function readMetadata(response: Response) {
  const tag = (Object.keys(tags) as Tag[]).find(
    (name) => response.headers.get("etag") === tags[name],
  );
  const policy = (Object.keys(policies) as Policy[]).find(
    (name) => response.headers.get("cache-control") === policies[name],
  );
  assert.ok(tag && policy);
  return { tag, policy };
}

async function readEntry(response: Response): Promise<Entry> {
  assert.equal(response.status, 200);
  const body = await response.text();
  assert.ok(body === "BodyA" || body === "BodyB");
  return { ...readMetadata(response), body };
}

function send(response: ServerResponse, status: 200 | 304, entry: Entry) {
  response.writeHead(status, {
    ETag: tags[entry.tag],
    "Cache-Control": policies[entry.policy],
  });
  response.end(status === 200 ? entry.body : undefined);
}

async function openCache(t: TestContext, fault?: Fault) {
  let origin = original;
  let deliver: (() => void) | undefined;
  const arrivals = new EventEmitter();
  const server = createServer((request, response) => {
    if (request.headers["if-none-match"] === tags[origin.tag]) {
      const pending = { ...origin, policy: "Refreshed" } as const;
      deliver = () => send(response, 304, pending);
      arrivals.emit("conditional");
    } else {
      send(response, 200, origin);
    }
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
  let cached: Entry | null = await readEntry(await fetch(url));
  let inFlight: { base: Entry; response: Promise<Response> } | null = null;
  const replies: Tag[] = [];

  return {
    replies,
    async apply({ kind }: Command) {
      switch (kind) {
        case "start": {
          assert.ok(cached);
          const arrived = once(arrivals, "conditional", {
            signal: AbortSignal.timeout(5_000),
          });
          const response = fetch(url, {
            headers: { "If-None-Match": tags[cached.tag] },
          });
          void response.catch(() => undefined);
          inFlight = { base: cached, response };
          await arrived;
          break;
        }
        case "replace":
          origin = replacement;
          cached = await readEntry(await fetch(url));
          break;
        case "evict":
          cached = null;
          break;
        case "finish": {
          assert.ok(deliver && inFlight);
          deliver();
          deliver = undefined;
          const response = await inFlight.response;
          assert.equal(response.status, 304);
          assert.equal(await response.text(), "");
          const { tag, policy } = readMetadata(response);
          assert.equal(tag, inFlight.base.tag);
          replies.push(tag);
          if (cached && (cached.tag === tag || fault === "update-unmatched")) {
            cached = { ...cached, policy };
          } else if (!cached && fault === "resurrect") {
            cached = { ...inFlight.base, policy };
          }
          inFlight = null;
          break;
        }
      }
    },
    async observe() {
      return { cached, requested: inFlight?.base.tag ?? null };
    },
    project: (observation: Readonly<Record<string, unknown>>) => observation,
  };
}

test(
  "a delayed HTTP 304 refreshes only the cached representation named by its validator",
  { timeout: 20_000 },
  async (t) => {
    for (const [name, expected] of [
      ["PendingOldTag", { ...original, policy: "Refreshed" }],
      ["ReplacedWhileRequested", replacement],
      ["EvictedWhileRequested", null],
    ] as const) {
      const prefix = await reachableScenario<Command>(checker, model, name, {
        checker: "rprob",
      });
      const scenario = {
        ...prefix,
        commands: [...prefix.commands, { kind: "finish" } as const],
      };
      await t.test(name, async (t) => {
        const driver = await openCache(t);
        const result = await recordReplay(checker, scenario, driver);
        assert.equal(result.verdict, "matched");
        assert.deepEqual(driver.replies, ["Old"]);
        assert.deepEqual(result.recording.observations.at(-1), {
          cached: expected,
          requested: null,
        });
      });

      const fault =
        name === "ReplacedWhileRequested"
          ? "update-unmatched"
          : name === "EvictedWhileRequested"
            ? "resurrect"
            : undefined;
      if (fault) {
        await t.test(`${name} / ${fault}`, async (t) => {
          const driver = await openCache(t, fault);
          const result = await recordReplay(checker, scenario, driver);
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(driver.replies, ["Old"]);
          assert.equal(result.difference.step, scenario.commands.length);
          assert.deepEqual(result.difference.expected, {
            cached: expected,
            requested: null,
          });
          const incorrectlyRefreshed =
            fault === "resurrect" ? original : replacement;
          assert.deepEqual(result.difference.observed, {
            cached: { ...incorrectlyRefreshed, policy: "Refreshed" },
            requested: null,
          });
        });
      }
    }
  },
);
