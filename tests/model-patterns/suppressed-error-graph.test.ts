/// <reference lib="esnext.disposable" />
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

type Identity = "Reuse" | "Copy";
type NodeLabel = "Earlier" | "Recent" | "Original" | "Clone" | "Wrapped";
type Wrapper = "Original" | "Clone" | "Wrapped";
type Command = { kind: "unwind"; identity: Identity };

const checker = "crates/idealis-check/target/debug/idealis-check";
const model =
  "examples/model-patterns/node-event-loop/suppressed-error-graph.ideal";

// Exercise native await using even when the test bundle targets ES2023.
const unwind = new Function(
  "resource",
  "bodyError",
  `
  return (async () => {
    await using owned = resource;
    throw bodyError;
  })();
`,
) as (resource: AsyncDisposable, bodyError: Error) => Promise<void>;

function openScope(copyWhenReused = false) {
  const earlier = new Error("earlier");
  const recent = new Error("recent");
  const original = new SuppressedError(recent, earlier, "same message");
  const clone = new SuppressedError(recent, earlier, "same message");
  const identities = new Map<unknown, NodeLabel>([
    [earlier, "Earlier"],
    [recent, "Recent"],
    [original, "Original"],
    [clone, "Clone"],
  ]);
  let terminal: SuppressedError | null = null;

  function label(node: unknown): NodeLabel {
    const identity = identities.get(node);
    assert.ok(identity);
    return identity;
  }

  return {
    async apply(command: Command) {
      const disposalError =
        command.identity === "Copy" || copyWhenReused ? clone : original;
      await assert.rejects(
        unwind(
          {
            async [Symbol.asyncDispose]() {
              throw disposalError;
            },
          },
          original,
        ),
        (error: unknown) => {
          assert.ok(error instanceof SuppressedError);
          assert.ok(!identities.has(error));
          identities.set(error, "Wrapped");
          terminal = error;
          return true;
        },
      );
    },
    async observe() {
      const primary: Record<Wrapper, NodeLabel | null> = {
        Original: null,
        Clone: null,
        Wrapped: null,
      };
      const suppressed = { ...primary };
      const visited = new Set<unknown>();
      function visit(node: unknown) {
        if (!(node instanceof SuppressedError) || visited.has(node)) return;
        visited.add(node);
        const key = label(node);
        assert.ok(key === "Original" || key === "Clone" || key === "Wrapped");
        primary[key] = label(node.error);
        suppressed[key] = label(node.suppressed);
        visit(node.error);
        visit(node.suppressed);
      }
      visit(terminal);
      return { root: terminal ? label(terminal) : null, primary, suppressed };
    },
    project: (snapshot: Readonly<Record<string, unknown>>) => snapshot,
  };
}

test("SuppressedError preserves shared identities and distinguishes copies with identical branches", async (t) => {
  const base = await reachableScenario<Command>(
    checker,
    model,
    "InitialErrorScope",
    { checker: "rprob" },
  );
  for (const identity of ["Reuse", "Copy"] as const) {
    await t.test(identity, async () => {
      const driver = openScope();
      const result = await recordReplay(
        checker,
        {
          ...base,
          commands: [...base.commands, { kind: "unwind", identity }],
        },
        driver,
      );
      assert.equal(result.verdict, "matched");
      const graph = await driver.observe();
      assert.equal(
        graph.primary.Wrapped,
        identity === "Reuse" ? "Original" : "Clone",
      );
      assert.equal(graph.suppressed.Wrapped, "Original");
      assert.equal(graph.primary.Clone, identity === "Reuse" ? null : "Recent");
    });
  }
  await t.test(
    "copying the thrown wrapper loses its sharing with the body error",
    async () => {
      const driver = openScope(true);
      const result = await recordReplay(
        checker,
        {
          ...base,
          commands: [...base.commands, { kind: "unwind", identity: "Reuse" }],
        },
        driver,
      );
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, 1);
      assert.partialDeepStrictEqual(result.difference.expected, {
        primary: { Wrapped: "Original", Clone: null },
        suppressed: { Wrapped: "Original" },
      });
      assert.partialDeepStrictEqual(result.difference.observed, {
        primary: { Wrapped: "Clone", Clone: "Recent" },
        suppressed: { Wrapped: "Original" },
      });
      const trace = JSON.parse(JSON.stringify(toReplayItf(result.recording)));
      assert.deepEqual(
        await replayRecording(checker, fromReplayItf(trace), driver.project),
        {
          verdict: "mismatch",
          difference: result.difference,
        },
      );
    },
  );
});
