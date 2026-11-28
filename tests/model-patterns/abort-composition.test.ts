import assert from "node:assert/strict";
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

type Source = "Left" | "Right";
type Command =
  | { kind: "compose" | "abortWithReentry"; first: Source }
  | { kind: "abort"; source: Source };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/cancellation/abort-composition.ideal";
const sources = ["Left", "Right"] as const;
const other = { Left: "Right", Right: "Left" } as const;
const event = { Left: "LeftEvent", Right: "RightEvent" } as const;

function composeByListening(signals: readonly AbortSignal[]): AbortSignal {
  const prior = signals.find((signal) => signal.aborted);
  if (prior) return AbortSignal.abort(prior.reason);
  const result = new AbortController();
  for (const signal of signals) {
    signal.addEventListener("abort", () => result.abort(signal.reason), {
      once: true,
    });
  }
  return result.signal;
}

function openSignals(
  combine: (signals: AbortSignal[]) => AbortSignal = (signals) =>
    AbortSignal.any(signals),
) {
  const controllers = {
    Left: new AbortController(),
    Right: new AbortController(),
  };
  const errors = {
    Left: new Error("left cancelled"),
    Right: new Error("right cancelled"),
  };
  const history: string[] = [];
  const seen: Record<Source, Source | null> = { Left: null, Right: null };
  let composed: AbortSignal | null = null;
  let reenterFrom: Source | null = null;

  function reason(): Source | null {
    if (!composed?.aborted) return null;
    const identity = composed.reason;
    const source = sources.find((source) => errors[source] === identity);
    assert.ok(source, "the composed signal carries the original reason object");
    return source;
  }

  for (const source of sources) {
    controllers[source].signal.addEventListener(
      "abort",
      () => {
        history.push(event[source]);
        seen[source] = reason();
        if (reenterFrom === source) {
          const nested = other[source];
          controllers[nested].abort(errors[nested]);
        }
      },
      { once: true },
    );
  }
  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "compose": {
          const { first } = command;
          composed = combine([
            controllers[first].signal,
            controllers[other[first]].signal,
          ]);
          composed.addEventListener("abort", () => history.push("JoinedEvent"));
          break;
        }
        case "abort":
          controllers[command.source].abort(errors[command.source]);
          break;
        case "abortWithReentry":
          reenterFrom = command.first;
          controllers[command.first].abort(errors[command.first]);
          reenterFrom = null;
          break;
      }
    },
    async observe() {
      return {
        stopped: sources.filter((source) => controllers[source].signal.aborted),
        composed: composed !== null,
        joined: reason(),
        seen: { ...seen },
        history: { $sequence: [...history] },
      };
    },
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test("AbortSignal.any fixes the reason before source listeners and delivers a single dependent event", async (t) => {
  const live = await reachableScenario<Command>(
    checker,
    model,
    "LiveComposition",
    { checker: "rprob" },
  );
  await t.test(
    "the first of two sequential aborts supplies the unchanged reason",
    async () => {
      const driver = openSignals();
      const result = await recordReplay(
        checker,
        {
          ...live,
          commands: [
            ...live.commands,
            { kind: "abort", source: "Right" },
            { kind: "abort", source: "Left" },
          ],
        },
        driver,
      );
      assert.equal(result.verdict, "matched");
      assert.deepEqual(await driver.observe(), {
        stopped: ["Left", "Right"],
        composed: true,
        joined: "Right",
        seen: { Left: "Right", Right: "Right" },
        history: { $sequence: ["RightEvent", "JoinedEvent", "LeftEvent"] },
      });
    },
  );

  for (const first of sources) {
    await t.test(`nested abort from the ${first} callback`, async () => {
      const driver = openSignals();
      const result = await recordReplay(
        checker,
        {
          ...live,
          commands: [...live.commands, { kind: "abortWithReentry", first }],
        },
        driver,
      );
      assert.equal(result.verdict, "matched");
      assert.deepEqual(await driver.observe(), {
        stopped: ["Left", "Right"],
        composed: true,
        joined: first,
        seen: { Left: first, Right: first },
        history: {
          $sequence: [event[first], event[other[first]], "JoinedEvent"],
        },
      });
    });
  }

  await t.test(
    "a listener-forwarding shim loses the outer abort reason",
    async () => {
      const driver = openSignals(composeByListening);
      const result = await recordReplay(
        checker,
        {
          ...live,
          commands: [
            ...live.commands,
            { kind: "abortWithReentry", first: "Left" },
          ],
        },
        driver,
      );
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, live.commands.length + 1);
      assert.partialDeepStrictEqual(result.difference.expected, {
        joined: "Left",
        seen: { Left: "Left", Right: "Left" },
        history: { $sequence: ["LeftEvent", "RightEvent", "JoinedEvent"] },
      });
      assert.partialDeepStrictEqual(result.difference.observed, {
        joined: "Right",
        seen: { Left: null, Right: null },
        history: { $sequence: ["LeftEvent", "RightEvent", "JoinedEvent"] },
      });
      const itf = JSON.parse(JSON.stringify(toReplayItf(result.recording)));
      assert.deepEqual(
        await replayRecording(checker, fromReplayItf(itf), driver.project),
        {
          verdict: "mismatch",
          difference: result.difference,
        },
      );
    },
  );

  await t.test(
    "with two already-aborted inputs the list order wins without a new event",
    async () => {
      const prior = await reachableScenario<Command>(
        checker,
        model,
        "BothStoppedLeftFirst",
        { checker: "rprob" },
      );
      for (const first of sources) {
        const driver = openSignals();
        const result = await recordReplay(
          checker,
          {
            ...prior,
            commands: [...prior.commands, { kind: "compose", first }],
          },
          driver,
        );
        assert.equal(result.verdict, "matched");
        assert.deepEqual(await driver.observe(), {
          stopped: ["Left", "Right"],
          composed: true,
          joined: first,
          seen: { Left: null, Right: null },
          history: { $sequence: ["LeftEvent", "RightEvent"] },
        });
      }
    },
  );
});
