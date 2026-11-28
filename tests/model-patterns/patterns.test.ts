import assert from "node:assert/strict";
import { readFile } from "node:fs";
import { createServer, type RequestListener } from "node:http";
import { nextTick } from "node:process";
import test, { type TestContext } from "node:test";
import { chromium, type Page } from "@playwright/test";
import type { Navigation } from "../../examples/model-patterns/browser/adapter.js";
import {
  browserHistoryPattern,
  browserScreenProjection,
  ioBatchProjection,
  nextTickProjection,
  nodeIoBatchPattern,
  nodeNextTickPattern,
  objectStorePattern,
} from "../../examples/model-patterns/library.js";
import type {
  IoCallback,
  NextTickCallback,
} from "../../examples/model-patterns/node-event-loop/adapter.js";
import {
  nodeFileIoBatch,
  nodeFileNextTickBatch,
} from "../../examples/model-patterns/node-event-loop/file-batch.js";
import type { ObjectStore } from "../../examples/model-patterns/object-store/adapter.js";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  replay,
  recordReplay,
  replayRecording,
  type ObservationProjection,
  type Scenario,
} from "../../examples/model-patterns/replay.js";

const checkerPath = "crates/idealis-check/target/debug/idealis-check";
const models = "examples/model-patterns";

async function serve(
  t: TestContext,
  handler: RequestListener,
): Promise<string> {
  const server = createServer(handler);
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  t.after(async () => {
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  return `http://127.0.0.1:${address.port}`;
}

async function traverse(page: Page, direction: "back" | "forward") {
  await page.evaluate(
    (direction) =>
      new Promise<void>((resolve, reject) => {
        const timeout = window.setTimeout(
          () => reject(new Error("missing popstate")),
          5_000,
        );
        window.addEventListener(
          "popstate",
          () => {
            window.clearTimeout(timeout);
            resolve();
          },
          { once: true },
        );
        if (direction === "back") window.history.back();
        else window.history.forward();
      }),
    direction,
  );
}

const routes = {
  Home: { path: "/home", screen: "home" },
  Catalog: { path: "/catalog", screen: "catalog" },
  Detail: { path: "/product", screen: "product" },
};

test("a rejected history command never reaches the adapter", async () => {
  let applied = 0;
  const { scenario, adapter } = browserHistoryPattern(
    {
      async push() {},
      async goBack() {
        applied += 1;
      },
      async goForward() {},
      async observe() {
        return routes.Home;
      },
    },
    routes,
  );
  await assert.rejects(
    replay(
      checkerPath,
      { ...scenario, commands: [{ kind: "goBack" }] },
      adapter,
    ),
    /model rejected step 1/,
  );
  assert.equal(applied, 0);
});

test(
  "Chromium history agrees with the visible screen, including back and forward",
  { timeout: 30_000 },
  async (t) => {
    const origin = await serve(t, (_request, response) => {
      response.setHeader("Content-Type", "text/html");
      response.end(`<!doctype html><main></main><script>
      const render = () => { document.querySelector("main").textContent = location.pathname.slice(1); };
      render();
      document.addEventListener("navigate", render);
      if (!location.search.includes("skip-popstate")) addEventListener("popstate", render);
    </script>`);
    });
    const browser = await chromium.launch();
    t.after(() => browser.close());
    for (const faulty of [false, true]) {
      const page = await browser.newPage();
      const { scenario, adapter } = browserHistoryPattern(
        {
          async push(path) {
            await page.evaluate((path) => {
              window.history.pushState(null, "", path);
              document.dispatchEvent(new Event("navigate"));
            }, path);
          },
          goBack: () => traverse(page, "back"),
          goForward: () => traverse(page, "forward"),
          observe: () =>
            page.evaluate(() => ({
              path: location.pathname,
              screen: document.querySelector("main")?.textContent ?? null,
            })),
        },
        routes,
      );
      let result;
      try {
        await page.goto(`${origin}/home${faulty ? "?skip-popstate" : ""}`);
        result = await recordReplay(checkerPath, scenario, adapter);
      } finally {
        await page.close();
      }
      if (faulty) {
        assert.equal(result.verdict, "mismatch");
        assert.deepEqual(result.difference, {
          step: 2,
          command: { kind: "goBack" },
          expected: routes.Home,
          observed: { path: "/home", screen: "catalog" },
        });
        const saved = JSON.parse(
          JSON.stringify(result.recording),
        ) as typeof result.recording;
        assert.deepEqual(
          await replayRecording(
            checkerPath,
            saved,
            browserScreenProjection(routes),
          ),
          { verdict: "mismatch", difference: result.difference },
        );
      } else {
        assert.equal(result.verdict, "matched");
        assert.equal(
          result.recording.observations.length,
          scenario.commands.length + 1,
        );
      }
    }
  },
);

test("the built-in Node file batch records and rechecks observations without I/O", async () => {
  const { scenario, adapter } = nodeIoBatchPattern(
    nodeFileIoBatch(new URL(import.meta.url)),
  );
  const result = await recordReplay(checkerPath, scenario, adapter);
  assert.equal(result.verdict, "matched");
  assert.deepEqual(result.recording.observations, [
    { $sequence: [] },
    { $sequence: ["Microtask", "Immediate", "Timer"] },
  ]);
  assert.deepEqual(
    await replayRecording(checkerPath, result.recording, ioBatchProjection),
    {
      verdict: "matched",
    },
  );
  await assert.rejects(
    replayRecording(
      checkerPath,
      {
        ...result.recording,
        observations: result.recording.observations.slice(0, 1),
      },
      ioBatchProjection,
    ),
    /missing observation at step 1/,
  );
  const missing = nodeIoBatchPattern(
    nodeFileIoBatch(new URL("missing-io-batch-input", import.meta.url)),
  );
  await assert.rejects(
    recordReplay(checkerPath, missing.scenario, missing.adapter),
    { code: "ENOENT" },
  );
});

test("Node runs the observed microtask and immediate before the timer queued in I/O", async () => {
  for (const faulty of [false, true]) {
    const calls: IoCallback[] = [];
    const { scenario, adapter } = nodeIoBatchPattern({
      async drain() {
        await new Promise<void>((resolve, reject) => {
          readFile(new URL(import.meta.url), (error) => {
            if (error) {
              reject(error);
              return;
            }
            const record = (name: IoCallback) => () => {
              calls.push(name);
              if (calls.length === 3) resolve();
            };
            queueMicrotask(record("Microtask"));
            setImmediate(record("Immediate"));
            if (faulty) queueMicrotask(record("Timer"));
            else setTimeout(record("Timer"), 0);
          });
        });
      },
      observe: () => calls,
    });
    const run = replay(checkerPath, scenario, adapter);
    if (faulty)
      await assert.rejects(
        run,
        /"step":1.*"observed":\{"\$sequence":\["Microtask","Timer","Immediate"\]/,
      );
    else await run;
  }
});

test("a next tick queued inside a microtask waits for the microtask queue to drain", async () => {
  const file = new URL(import.meta.url);
  const { scenario, adapter } = nodeNextTickPattern(
    nodeFileNextTickBatch(file),
  );
  const normal = await recordReplay(checkerPath, scenario, adapter);
  const expected = {
    $sequence: [
      "IoNextTick",
      "FirstMicrotask",
      "SecondMicrotask",
      "NestedNextTick",
    ],
  };
  assert.equal(normal.verdict, "matched");
  assert.deepEqual(normal.recording.observations.at(-1), expected);

  const calls: NextTickCallback[] = [];
  const faulty = nodeNextTickPattern({
    async drain() {
      await new Promise<void>((resolve, reject) => {
        readFile(file, (error) => {
          if (error) return reject(error);
          nextTick(() => calls.push("IoNextTick"));
          queueMicrotask(() => {
            calls.push("FirstMicrotask");
            calls.push("NestedNextTick");
          });
          queueMicrotask(() => {
            calls.push("SecondMicrotask");
            resolve();
          });
        });
      });
    },
    observe: () => calls,
  });
  const result = await recordReplay(
    checkerPath,
    faulty.scenario,
    faulty.adapter,
  );
  assert.equal(result.verdict, "mismatch");
  assert.deepEqual(result.difference, {
    step: 1,
    command: { kind: "drainIoBatch" },
    expected,
    observed: {
      $sequence: [
        "IoNextTick",
        "FirstMicrotask",
        "NestedNextTick",
        "SecondMicrotask",
      ],
    },
  });
  assert.deepEqual(
    await replayRecording(checkerPath, result.recording, nextTickProjection),
    {
      verdict: "mismatch",
      difference: result.difference,
    },
  );
});

test("an existing object stops the pattern before the first write", async () => {
  let writes = 0;
  const { scenario, adapter } = objectStorePattern(
    {
      async put() {
        writes += 1;
      },
      async remove() {
        writes += 1;
      },
      async get() {
        return "existing";
      },
      async list() {
        return ["test"];
      },
    },
    "test",
  );
  const result = await recordReplay(checkerPath, scenario, adapter);
  assert.equal(result.verdict, "mismatch");
  assert.equal(result.difference.step, 0);
  assert.equal(writes, 0);
});

function httpObjectStore(origin: string): ObjectStore {
  const url = (key: string) => `${origin}/objects/${encodeURIComponent(key)}`;
  async function change(key: string, init: RequestInit) {
    const reply = await fetch(url(key), init);
    assert.ok(reply.ok, `write returned ${reply.status}`);
  }
  return {
    put: (key, body) => change(key, { method: "PUT", body }),
    remove: (key) => change(key, { method: "DELETE" }),
    async get(key) {
      const reply = await fetch(url(key));
      if (reply.status === 404) return null;
      assert.ok(reply.ok, `read returned ${reply.status}`);
      return reply.text();
    },
    async list(prefix) {
      const reply = await fetch(
        `${origin}/objects?prefix=${encodeURIComponent(prefix)}`,
      );
      assert.ok(reply.ok, `list returned ${reply.status}`);
      return reply.json() as Promise<string[]>;
    },
  };
}

test("read and list observe writes and deletes against a local object endpoint", async (t) => {
  for (const fault of ["none", "stale-get", "hidden-list"]) {
    const objects = new Map<string, string>();
    const stale = new Map<string, string>();
    const origin = await serve(t, (request, response) => {
      const url = new URL(request.url ?? "/", "http://localhost");
      const key = decodeURIComponent(url.pathname.slice("/objects/".length));
      if (request.method === "PUT") {
        let body = "";
        request.setEncoding("utf8");
        request.on("data", (chunk: string) => {
          body += chunk;
        });
        request.on("end", () => {
          const previous = objects.get(key);
          if (fault === "stale-get" && previous !== undefined)
            stale.set(key, previous);
          objects.set(key, body);
          response.end();
        });
      } else if (request.method === "DELETE") {
        objects.delete(key);
        stale.delete(key);
        response.end();
      } else if (url.pathname === "/objects") {
        const prefix = url.searchParams.get("prefix") ?? "";
        response.end(
          JSON.stringify(
            fault === "hidden-list"
              ? []
              : [...objects.keys()].filter((key) => key.startsWith(prefix)),
          ),
        );
      } else {
        const body = stale.get(key) ?? objects.get(key);
        response.statusCode = body === undefined ? 404 : 200;
        response.end(body);
      }
    });
    const { scenario, adapter } = objectStorePattern(
      httpObjectStore(origin),
      "test",
    );
    const run = replay(checkerPath, scenario, adapter);
    if (fault === "stale-get")
      await assert.rejects(
        run,
        /"step":2.*"expected":\{"body":"V2","listed":true\},"observed":\{"body":"V1","listed":true\}/,
      );
    else if (fault === "hidden-list")
      await assert.rejects(
        run,
        /"step":1.*"observed":\{"body":"V1","listed":false\}/,
      );
    else await run;
  }
});

test(
  "Chromium history keeps the protected screen hidden after logout and discards forward entries",
  { timeout: 30_000 },
  async (t) => {
    type Command =
      | Navigation<"Public" | "Account">
      | { readonly kind: "signIn" | "signOut" };
    const paths = { Home: "/home", Public: "/public", Account: "/account" };
    const screens = { ...paths, SignIn: "/sign-in" };
    const scenario: Scenario<Command> = {
      model: `${models}/auth-history/history.ideal`,
      config: {},
      derive: ["screen"],
      initial: {
        history: { $sequence: ["Home"] },
        cursor: 0,
        current: "Home",
        signedIn: false,
      },
      commands: [
        { kind: "signIn" },
        { kind: "push", destination: "Account" },
        { kind: "push", destination: "Public" },
        { kind: "signOut" },
        { kind: "goBack" },
        { kind: "goBack" },
        { kind: "goForward" },
        { kind: "goForward" },
        { kind: "goBack" },
        { kind: "goBack" },
        { kind: "push", destination: "Public" },
        { kind: "goBack" },
        { kind: "goForward" },
      ],
    };
    const project: ObservationProjection = (state, derived) => {
      const screen = derived?.screen as [[keyof typeof screens]];
      return {
        path: paths[state.current as keyof typeof paths],
        screen: screens[screen[0][0]],
        signedIn: state.signedIn,
        entries: (state.history as { $sequence: readonly unknown[] }).$sequence
          .length,
      };
    };
    const origin = await serve(t, (request, response) => {
      const faulty = request.url?.includes("?unguarded-history");
      response.setHeader("Content-Type", "text/html");
      response.end(`<!doctype html><main></main><script>
        let signedIn = false;
        function render(unguarded = false) {
          const path = location.pathname;
          document.querySelector("main").textContent =
            path === "/account" && !signedIn && !unguarded ? "/sign-in" : path;
          document.documentElement.dataset.signedIn = String(signedIn);
        }
        for (const action of ["signIn", "signOut"]) {
          addEventListener(action, () => {
            signedIn = action === "signIn";
            render();
          });
        }
        addEventListener("pushed", () => render());
        addEventListener("popstate", () => render(${faulty === true}));
        render();
      </script>`);
    });
    const browser = await chromium.launch();
    t.after(() => browser.close());
    for (const faulty of [false, true]) {
      const page = await browser.newPage();
      try {
        await page.goto(`${origin}/home${faulty ? "?unguarded-history" : ""}`);
        const initialLength = await page.evaluate(() => window.history.length);
        const result = await recordReplay(checkerPath, scenario, {
          async apply(command) {
            if (command.kind === "push") {
              await page.evaluate((path) => {
                window.history.pushState(null, "", path);
                window.dispatchEvent(new Event("pushed"));
              }, paths[command.destination]);
            } else if (
              command.kind === "goBack" ||
              command.kind === "goForward"
            ) {
              await traverse(
                page,
                command.kind === "goBack" ? "back" : "forward",
              );
            } else {
              await page.evaluate((kind) => {
                window.dispatchEvent(new Event(kind));
              }, command.kind);
            }
          },
          observe: () =>
            page.evaluate(
              (initialLength) => ({
                path: location.pathname,
                screen: document.querySelector("main")?.textContent,
                signedIn: document.documentElement.dataset.signedIn === "true",
                entries: window.history.length - initialLength + 1,
              }),
              initialLength,
            ),
          project,
        });
        if (faulty) {
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(result.difference, {
            step: 5,
            command: { kind: "goBack" },
            expected: {
              path: "/account",
              screen: "/sign-in",
              signedIn: false,
              entries: 3,
            },
            observed: {
              path: "/account",
              screen: "/account",
              signedIn: false,
              entries: 3,
            },
          });
        } else {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[7], {
            path: "/account",
            screen: "/sign-in",
            signedIn: false,
            entries: 3,
          });
        }
        const { recording, ...verdict } = result;
        const itf = toReplayItf(recording);
        const restored = fromReplayItf(JSON.parse(JSON.stringify(itf)));
        assert.deepEqual(restored.scenario.derive, ["screen"]);
        assert.deepEqual(
          await replayRecording(checkerPath, restored, project),
          verdict,
        );
      } finally {
        await page.close();
      }
    }
  },
);
