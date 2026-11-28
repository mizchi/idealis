import assert from "node:assert/strict";
import { createServer } from "node:http";
import test from "node:test";
import { chromium } from "@playwright/test";
import { replay, type Scenario } from "../../examples/model-patterns/replay.js";

type Tab = "Left" | "Right";
type Setting = "Default" | "Light" | "Dark";
type Command =
  | { readonly kind: "load"; readonly tab: Tab }
  | { readonly kind: "save"; readonly tab: Tab; readonly setting: Setting };
const scenario: Scenario<Command> = {
  model: "examples/model-patterns/tab-settings/settings.ideal",
  config: {},
  initial: {
    version: 0,
    value: "Default",
    loaded: { Left: null, Right: null },
    responses: { Left: null, Right: null },
  },
  commands: [
    { kind: "load", tab: "Left" },
    { kind: "load", tab: "Right" },
    { kind: "save", tab: "Left", setting: "Light" },
    { kind: "save", tab: "Right", setting: "Dark" },
    { kind: "load", tab: "Right" },
    { kind: "save", tab: "Right", setting: "Dark" },
  ],
};

test(
  "two Chromium tabs reject a stale HTTP write and recover by reloading",
  { timeout: 30_000 },
  async (t) => {
    const browser = await chromium.launch();
    t.after(() => browser.close());
    for (const bypass of [false, true]) {
      let version = 0;
      let value: Setting = "Default";
      const etag = () => `"${version}"`;
      const server = createServer(async (request, response) => {
        if (request.url !== "/settings") {
          response.setHeader("Content-Type", "text/html");
          response.end(`<!doctype html><main></main><script>
          const screen = document.querySelector("main");
          window.loadSettings = async () => {
            const reply = await fetch("/settings");
            if (!reply.ok) throw new Error("could not load settings");
            await reply.json();
            screen.dataset.etag = reply.headers.get("ETag");
            screen.textContent = "Ready";
          };
          window.saveSettings = async (setting) => {
            const reply = await fetch("/settings", {
              method: "PUT",
              headers: { "If-Match": screen.dataset.etag },
              body: setting,
            });
            if (reply.status === 412) {
              screen.textContent = "Conflict";
              return;
            }
            if (!reply.ok) throw new Error("could not save settings");
            await reply.json();
            screen.dataset.etag = reply.headers.get("ETag");
            screen.textContent = "Saved";
          };
        </script>`);
          return;
        }
        if (request.method === "PUT") {
          request.setEncoding("utf8");
          let body = "";
          for await (const chunk of request) body += chunk;
          if (!bypass && request.headers["if-match"] !== etag()) {
            response.writeHead(412).end();
            return;
          }
          if (body !== "Default" && body !== "Light" && body !== "Dark") {
            response.writeHead(400).end();
            return;
          }
          value = body;
          version += 1;
        }
        response.setHeader("Content-Type", "application/json");
        response.setHeader("Cache-Control", "no-store");
        response.setHeader("ETag", etag());
        response.end(JSON.stringify({ value }));
      });
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
      const origin = `http://127.0.0.1:${address.port}`;
      const context = await browser.newContext();
      try {
        const pages = {
          Left: await context.newPage(),
          Right: await context.newPage(),
        };
        await Promise.all(
          Object.values(pages).map((page) => page.goto(origin)),
        );
        const run = replay(
          "crates/idealis-check/target/debug/idealis-check",
          scenario,
          {
            async apply(command) {
              await pages[command.tab].evaluate((command) => {
                const app = window as unknown as {
                  loadSettings(): Promise<void>;
                  saveSettings(setting: Setting): Promise<void>;
                };
                return command.kind === "load"
                  ? app.loadSettings()
                  : app.saveSettings(command.setting);
              }, command);
            },
            async observe() {
              const snapshot = await fetch(`${origin}/settings`, {
                signal: AbortSignal.timeout(5_000),
              });
              const current = (await snapshot.json()) as { value: Setting };
              assert.ok(snapshot.headers.get("etag"));
              const tabs = await Promise.all(
                Object.values(pages).map((page) =>
                  page.evaluate(() => {
                    const screen = document.querySelector("main");
                    const tag = screen?.dataset.etag;
                    return {
                      loaded: tag ? Number(tag.slice(1, -1)) : null,
                      response: screen?.textContent || null,
                    };
                  }),
                ),
              );
              const [left, right] = tabs;
              assert.ok(left && right);
              return {
                version: Number(snapshot.headers.get("etag")?.slice(1, -1)),
                value: current.value,
                loaded: { Left: left.loaded, Right: right.loaded },
                responses: { Left: left.response, Right: right.response },
              };
            },
            project(state) {
              return state;
            },
          },
        );
        if (bypass) {
          await assert.rejects(
            run,
            /"step":4.*"expected":.*"Conflict".*"observed":\{"version":2,"value":"Dark"/,
          );
        } else {
          await run;
        }
      } finally {
        await context.close();
      }
    }
  },
);
