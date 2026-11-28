import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test from "node:test";
import { chromium, type Route } from "@playwright/test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type RequestId = "Old" | "New";
type Command = {
  kind: "choose" | "accept" | "persist" | "deliver";
  request: RequestId;
};

const checkerPath = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/upload/upload.ideal";
const bodies = { Old: "first file", New: "replacement file" };

const document = `<!doctype html>
<input type="file"><output></output>
<script>
let selected = 0;
const input = document.querySelector("input");
const output = document.querySelector("output");
input.onchange = async () => {
  const file = input.files[0];
  const number = ++selected;
  const reply = await fetch("/upload/" + file.name, {
    method: "POST", body: file, headers: { "x-upload-generation": String(number) }
  });
  if (!reply.ok) throw new Error("upload failed");
  const result = await reply.json();
  if (location.search === "?stale-screen" || number === selected) {
    output.textContent = result.id;
  }
  document.body.dataset.lastResponse = result.id;
};
</script>`;

test(
  "upload replay finds independent regressions in the published file and the screen",
  { timeout: 30_000 },
  async (t) => {
    const browser = await chromium.launch();
    t.after(() => browser.close());
    const awaitingSave = await reachableScenario<Command>(
      checkerPath,
      model,
      "NewVisibleWhileOldWriting",
    );
    const awaitingReply = await reachableScenario<Command>(
      checkerPath,
      model,
      "NewVisibleWhileOldReplyPending",
    );
    const awaitingArrival = await reachableScenario<Command>(
      checkerPath,
      model,
      "NewVisibleWhileOldInTransit",
    );
    assert.equal(awaitingArrival.commands.length, 5);
    assert.equal(awaitingSave.commands.length, 6);
    assert.equal(awaitingReply.commands.length, 7);

    for (const [base, fault] of [
      [awaitingArrival, "none"],
      [awaitingSave, "none"],
      [awaitingReply, "none"],
      [awaitingSave, "stale-publish"],
      [awaitingReply, "stale-screen"],
      [awaitingArrival, "arrival-order"],
    ] as const) {
      const scenario = {
        ...base,
        commands: [
          ...base.commands,
          ...(base === awaitingArrival
            ? [{ kind: "accept", request: "Old" } as const]
            : []),
          ...(base !== awaitingReply
            ? [{ kind: "persist", request: "Old" } as const]
            : []),
          { kind: "deliver", request: "Old" } as const,
        ],
      };
      const waiting = new Map<
        string,
        { body: string; response: ServerResponse }
      >();
      const uploads = new Map<string, Route>();
      const sent = new EventEmitter();
      const arrivals = new EventEmitter();
      const files = new Map<string, string>();
      let selected: string | null = null;
      let highestGeneration = 0;
      let publicKey: string | null = null;
      const server = createServer((request, response) => {
        const url = request.url ?? "/";
        if (request.method === "POST" && url.startsWith("/upload/")) {
          const id = url.slice("/upload/".length);
          let body = "";
          request.setEncoding("utf8");
          request.on("data", (chunk: string) => {
            body += chunk;
          });
          request.on("end", () => {
            const generation = Number(request.headers["x-upload-generation"]);
            if (fault === "arrival-order" || generation > highestGeneration) {
              highestGeneration = generation;
              selected = id;
            }
            waiting.set(id, { body, response });
            arrivals.emit(id);
          });
          return;
        }
        if (url === "/published" || url === "/files") {
          response.setHeader("Content-Type", "application/json");
          response.end(
            JSON.stringify(
              url === "/files"
                ? [...files.keys()].sort()
                : publicKey === null
                  ? null
                  : files.get(publicKey),
            ),
          );
          return;
        }
        response.setHeader("Content-Type", "text/html");
        response.end(document);
      });
      const page = await browser.newPage();
      try {
        server.listen(0, "127.0.0.1");
        await once(server, "listening");
        const address = server.address();
        assert.ok(address && typeof address !== "string");
        const origin = `http://127.0.0.1:${address.port}`;
        await page.goto(
          origin + (fault === "stale-screen" ? "/?stale-screen" : "/"),
        );
        await page.route("**/upload/*", (route) => {
          const id = new URL(route.request().url()).pathname.slice(
            "/upload/".length,
          );
          uploads.set(id, route);
          sent.emit(id);
        });
        const result = await recordReplay(checkerPath, scenario, {
          async apply({ kind, request }) {
            if (kind === "choose") {
              await Promise.all([
                once(sent, request, { signal: AbortSignal.timeout(5_000) }),
                page.locator("input").setInputFiles({
                  name: request,
                  mimeType: "text/plain",
                  buffer: Buffer.from(bodies[request]),
                }),
              ]);
              return;
            }
            if (kind === "accept") {
              const route = uploads.get(request);
              assert.ok(route);
              await Promise.all([
                once(arrivals, request, { signal: AbortSignal.timeout(5_000) }),
                route.continue(),
              ]);
              uploads.delete(request);
              return;
            }
            const upload = waiting.get(request);
            assert.ok(upload);
            if (kind === "persist") {
              files.set(request, upload.body);
              if (fault === "stale-publish" || selected === request)
                publicKey = request;
              return;
            }
            upload.response.setHeader("Content-Type", "application/json");
            upload.response.end(JSON.stringify({ id: request }));
            await page.waitForFunction(
              (id) => window.document.body.dataset.lastResponse === id,
              request,
            );
            waiting.delete(request);
          },
          async observe() {
            const [visible, published, stored] = await Promise.all([
              page.locator("output").textContent(),
              fetch(origin + "/published").then((reply) => reply.json()),
              fetch(origin + "/files").then((reply) => reply.json()),
            ]);
            return { visible: visible || null, published, stored };
          },
          project(state) {
            return {
              visible: state.shown,
              published:
                state.published === null
                  ? null
                  : bodies[state.published as RequestId],
              stored: [...(state.stored as RequestId[])].sort(),
            };
          },
        });
        const expected = {
          visible: "New",
          published: bodies.New,
          stored: ["New", "Old"],
        };
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), expected);
        } else {
          assert.equal(result.verdict, "mismatch");
          const stalePublish = fault !== "stale-screen";
          assert.deepEqual(result.difference, {
            step: stalePublish ? 7 : 8,
            command: {
              kind: stalePublish ? "persist" : "deliver",
              request: "Old",
            },
            expected,
            observed: stalePublish
              ? { ...expected, published: bodies.Old }
              : { ...expected, visible: "Old" },
          });
        }
      } finally {
        server.closeAllConnections();
        await Promise.all([
          page.close(),
          new Promise<void>((resolve) => server.close(() => resolve())),
        ]);
      }
    }
  },
);
