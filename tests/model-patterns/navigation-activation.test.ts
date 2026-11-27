import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import test from "node:test";
import { chromium, type Route } from "@playwright/test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Command =
  | { kind: "loadDetail" | "openCatalog" | "back" }
  | { kind: "finish"; visitId: number };
const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/browser/navigation-activation.model";

const document = `<!doctype html><main></main><script>
  const routeOnly = location.search.includes("route-only");
  const main = document.querySelector("main");
  let visit = 0;
  const pending = new Set();
  const completions = new Map();
  function activate() {
    const id = ++visit;
    const path = location.pathname;
    main.textContent = "";
    if (path === "/catalog") return;
    pending.add(id);
    const completion = fetch("/data/" + id).then(async response => {
      const data = await response.json();
      pending.delete(id);
      if (routeOnly ? path === location.pathname : visit === id) {
        main.textContent = String(data.version);
      }
    });
    completions.set(id, completion);
  }
  addEventListener("popstate", activate);
  window.demo = {
    loadDetail: activate,
    openCatalog() { history.pushState(null, "", "/catalog"); activate(); },
    back() {
      return new Promise(resolve => {
        addEventListener("popstate", resolve, { once: true });
        history.back();
      });
    },
    finish: id => completions.get(id),
    snapshot() {
      return {
        current: location.pathname === "/detail" ? "Detail" : "Catalog",
        visit,
        pending: [...pending].sort((left, right) => left - right),
        shown: main.textContent === "" ? null : Number(main.textContent),
      };
    },
  };
</script>`;

declare global {
  interface Window {
    demo: {
      loadDetail(): void;
      openCatalog(): void;
      back(): Promise<void>;
      finish(visit: number): Promise<void>;
      snapshot(): unknown;
    };
  }
}

test(
  "returning to the same URL creates a fresh activation for pending responses",
  { timeout: 30_000 },
  async (t) => {
    const fresh = await reachableScenario<Command>(
      checker,
      model,
      "FreshDetailWhileOldPending",
      { checker: "rprob" },
    );
    const scenario = {
      ...fresh,
      commands: [...fresh.commands, { kind: "finish", visitId: 1 }] as const,
    };
    assert.deepEqual(
      scenario.commands.map((command) => command.kind),
      ["loadDetail", "openCatalog", "back", "finish", "finish"],
    );
    const browser = await chromium.launch();
    t.after(() => browser.close());
    for (const routeOnly of [false, true]) {
      await t.test(
        routeOnly
          ? "a URL match accepts the old response"
          : "the current activation owns the screen",
        async (t) => {
          const page = await browser.newPage();
          t.after(() => page.close());
          const requests = new Map<number, Route>();
          const sent = new EventEmitter();
          await page.route("http://visit.test/**", async (route) => {
            const path = new URL(route.request().url()).pathname;
            if (path.startsWith("/data/")) {
              const id = Number(path.slice("/data/".length));
              requests.set(id, route);
              sent.emit(String(id));
            } else {
              await route.fulfill({ contentType: "text/html", body: document });
            }
          });
          await page.goto(
            `http://visit.test/detail${routeOnly ? "?route-only" : ""}`,
          );
          const result = await recordReplay(checker, scenario, {
            async apply(command) {
              if (command.kind === "finish") {
                const route = requests.get(command.visitId);
                assert.ok(route);
                await route.fulfill({ json: { version: command.visitId } });
                await page.evaluate(
                  (visit) => window.demo.finish(visit),
                  command.visitId,
                );
                return;
              }
              if (command.kind === "openCatalog") {
                await page.evaluate(() => window.demo.openCatalog());
                return;
              }
              const id = command.kind === "loadDetail" ? "1" : "3";
              await Promise.all([
                once(sent, id, { signal: AbortSignal.timeout(5_000) }),
                page.evaluate((kind) => window.demo[kind](), command.kind),
              ]);
            },
            observe: () => page.evaluate(() => window.demo.snapshot()),
            project: (state) => state,
          });
          if (!routeOnly) {
            assert.equal(result.verdict, "matched");
            return;
          }
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, 5);
          assert.deepEqual(result.difference.command, {
            kind: "finish",
            visitId: 1,
          });
          assert.equal(
            (result.difference.expected as { shown: number }).shown,
            3,
          );
          assert.equal(
            (result.difference.observed as { shown: number }).shown,
            1,
          );
        },
      );
    }
  },
);
