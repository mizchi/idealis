import assert from "node:assert/strict";
import test from "node:test";
import { chromium } from "@playwright/test";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
  type ObservationProjection,
} from "../../examples/model-patterns/replay.js";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/browser/bootstrap.ideal";
const project: ObservationProjection = (_state, derived) => {
  const screen = derived?.screen as [[string]];
  return { screen: screen[0][0] };
};

test(
  "the first rendered screen can disagree before any operation runs",
  { timeout: 30_000 },
  async (t) => {
    const witness = await reachableScenario<never>(
      checker,
      model,
      "SignedOutAccountBoot",
    );
    assert.deepEqual(witness.commands, []);
    const scenario = { ...witness, derive: ["screen"] };
    await assert.rejects(
      recordReplay(
        checker,
        { ...scenario, initial: { current: "Home" } },
        {
          async apply() {
            assert.fail("an invalid initial snapshot must not run operations");
          },
          async observe() {
            assert.fail("an invalid initial snapshot must not be observed");
          },
          project,
        },
      ),
      /model rejected initial snapshot.*does not satisfy init/s,
    );
    const browser = await chromium.launch();
    t.after(() => browser.close());
    for (const faulty of [false, true]) {
      await t.test(
        faulty
          ? "account content appears at startup"
          : "startup requires sign-in",
        async (t) => {
          const page = await browser.newPage();
          t.after(() => page.close());
          const address = "http://bootstrap.invalid/account";
          await page.route(address, async (route) => {
            await route.fulfill({
              contentType: "text/html",
              body: faulty ? "<main>Account</main>" : "<main>SignIn</main>",
            });
          });
          await page.goto(address);
          const result = await recordReplay(checker, scenario, {
            async apply() {
              assert.fail(
                "an initial observation must not execute an operation",
              );
            },
            async observe() {
              return { screen: await page.locator("main").textContent() };
            },
            project,
          });
          const { recording, ...verdict } = result;
          if (faulty) {
            assert.deepEqual(verdict, {
              verdict: "mismatch",
              difference: {
                step: 0,
                command: null,
                expected: { screen: "SignIn" },
                observed: { screen: "Account" },
              },
            });
          } else {
            assert.deepEqual(verdict, { verdict: "matched" });
          }
          const itf = toReplayItf(recording);
          assert.equal(itf.states.length, 1);
          const saved = fromReplayItf(JSON.parse(JSON.stringify(itf)));
          assert.deepEqual(
            await replayRecording(checker, saved, project),
            verdict,
          );
          await assert.rejects(
            replayRecording(checker, { ...saved, observations: [] }, project),
            /missing observation at step 0/,
          );
        },
      );
    }
  },
);
