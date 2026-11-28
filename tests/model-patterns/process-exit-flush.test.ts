import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import test from "node:test";
import {
  fromReplayItf,
  toReplayItf,
} from "../../examples/model-patterns/replay-itf.js";
import {
  reachableScenario,
  recordReplay,
  replayRecording,
} from "../../examples/model-patterns/replay.js";

type Schedule = "Before" | "Unref" | "AtExit";
type Path = "Natural" | "Explicit";
type Hook = "BeforeHook" | "ExitHook";
type Location = "BetweenBeforeHooks" | "OutsideBeforeHooks";
type Command = { kind: "run"; schedule: Schedule; path: Path };

const checker = "crates/rcheck/target/debug/rcheck";
const model =
  "examples/model-patterns/node-event-loop/process-exit-flush.model";

const program = `
const { writeSync } = require("node:fs");
const [schedule, path] = process.argv.slice(1);
const emit = (event, code) => writeSync(1, JSON.stringify({ event, code }) + "\\n");
let beforeCalls = 0;
function flush() { emit("Flush"); }
process.on("beforeExit", (code) => {
  emit("BeforeHook", code);
  beforeCalls++;
  if (beforeCalls !== 1) return;
  if (schedule === "Before") setImmediate(flush);
  if (schedule === "Unref") setImmediate(flush).unref();
});
process.on("exit", (code) => {
  emit("ExitHook", code);
  if (schedule === "AtExit") setImmediate(flush);
});
if (path === "Explicit") process.exit(7);
`;

async function runChild(schedule: Schedule, path: Path) {
  return new Promise<{ stdout: string; code: number }>((resolve, reject) => {
    execFile(
      process.execPath,
      ["--eval", program, schedule, path],
      { timeout: 5_000 },
      (error, stdout, stderr) => {
        if (stderr || (error && typeof error.code !== "number")) {
          reject(new Error(stderr || "could not run child", { cause: error }));
          return;
        }
        resolve({
          stdout,
          code: typeof error?.code === "number" ? error.code : 0,
        });
      },
    );
  });
}

function openProcess(flushFromExit = false) {
  const hooks: Hook[] = [];
  let flushAt: Location | null = null;
  let exited: Path | null = null;
  return {
    async apply(command: Command) {
      const schedule =
        flushFromExit && command.schedule === "Before"
          ? "AtExit"
          : command.schedule;
      const { stdout, code } = await runChild(schedule, command.path);
      assert.ok(code === 0 || code === 7);
      exited = code === 7 ? "Explicit" : "Natural";
      for (const line of stdout.trimEnd().split("\n")) {
        const observation: { event: unknown; code?: unknown } =
          JSON.parse(line);
        if (observation.event === "Flush") {
          assert.equal(flushAt, null);
          flushAt =
            hooks.length === 1 && hooks[0] === "BeforeHook"
              ? "BetweenBeforeHooks"
              : "OutsideBeforeHooks";
        } else {
          assert.ok(
            observation.event === "BeforeHook" ||
              observation.event === "ExitHook",
          );
          assert.equal(observation.code, code);
          hooks.push(observation.event);
        }
      }
    },
    async observe() {
      return { exited, hooks: { $sequence: [...hooks] }, flushAt };
    },
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test("only referenced work scheduled at beforeExit can extend the child process", async (t) => {
  const base = await reachableScenario<Command>(
    checker,
    model,
    "InitialProcess",
    { checker: "rprob" },
  );
  for (const schedule of ["Before", "Unref", "AtExit"] as const) {
    for (const path of ["Natural", "Explicit"] as const) {
      await t.test(`${schedule} / ${path}`, async () => {
        const driver = openProcess();
        const replay = await recordReplay(
          checker,
          {
            ...base,
            commands: [{ kind: "run", schedule, path }],
          },
          driver,
        );
        assert.equal(replay.verdict, "matched");
      });
    }
  }
  await t.test(
    "moving a flush to exit loses it despite a successful exit status",
    async () => {
      const driver = openProcess(true);
      const replay = await recordReplay(
        checker,
        {
          ...base,
          commands: [{ kind: "run", schedule: "Before", path: "Natural" }],
        },
        driver,
      );
      assert.equal(replay.verdict, "mismatch");
      assert.equal(replay.difference.step, 1);
      assert.deepEqual(replay.difference.expected, {
        exited: "Natural",
        hooks: { $sequence: ["BeforeHook", "BeforeHook", "ExitHook"] },
        flushAt: "BetweenBeforeHooks",
      });
      assert.deepEqual(replay.difference.observed, {
        exited: "Natural",
        hooks: { $sequence: ["BeforeHook", "ExitHook"] },
        flushAt: null,
      });
      const itf = JSON.parse(JSON.stringify(toReplayItf(replay.recording)));
      assert.deepEqual(
        await replayRecording(checker, fromReplayItf(itf), driver.project),
        {
          verdict: "mismatch",
          difference: replay.difference,
        },
      );
    },
  );
});
