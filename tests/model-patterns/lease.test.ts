import assert from "node:assert/strict";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Client = "Left" | "Right";
type Stage = "Waiting" | "Offered" | "Held" | "Cancelled" | "Returned";
type Command = {
  kind: "offer" | "timeout" | "accept" | "release";
  client: Client;
};
const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/lease/handoff.ideal";

test("a timed-out acquisition occupied its reservation before another offer", async () => {
  const offered = await reachableScenario<Command>(
    checker,
    model,
    "LeftOffered",
  );
  const held = await reachableScenario<Command>(checker, model, "LeftHeld");
  for (const [base, fault] of [
    [offered, "none"],
    [offered, "leaked"],
    [held, "none"],
    [held, "double"],
  ] as const) {
    const occupied = new Set<Client>();
    function acquisition(client: Client) {
      const controller = new AbortController();
      let grant!: (result: "Held" | "Cancelled") => void;
      const result = new Promise<"Held" | "Cancelled">((resolve) => {
        grant = resolve;
      });
      const state: { stage: Stage } = { stage: "Waiting" };
      controller.signal.addEventListener(
        "abort",
        () => {
          if (state.stage === "Held") return;
          state.stage = "Cancelled";
          if (fault !== "leaked") occupied.delete(client);
          grant("Cancelled");
        },
        { once: true },
      );
      return { controller, state, grant, result };
    }
    const clients = { Left: acquisition("Left"), Right: acquisition("Right") };
    const commands: Command[] = [
      ...base.commands,
      { kind: "timeout", client: "Left" },
    ];
    if (base === offered) commands.push({ kind: "accept", client: "Left" });
    commands.push({ kind: "offer", client: "Right" });
    if (base === held)
      commands.push(
        { kind: "release", client: "Left" },
        { kind: "offer", client: "Right" },
      );
    commands.push({ kind: "accept", client: "Right" });

    const result = await recordReplay<Command>(
      checker,
      { ...base, commands },
      {
        async apply({ kind, client }) {
          const { controller, state, grant, result } = clients[client];
          switch (kind) {
            case "offer":
              if (occupied.size === 0 || fault === "double") {
                occupied.add(client);
                state.stage = "Offered";
              }
              break;
            case "timeout":
              controller.abort(new Error("deadline"));
              if (state.stage !== "Held")
                assert.equal(await result, "Cancelled");
              break;
            case "accept":
              if (!controller.signal.aborted) {
                assert.equal(state.stage, "Offered");
                grant("Held");
              }
              state.stage = await result;
              break;
            case "release":
              assert.ok(occupied.delete(client));
              state.stage = "Returned";
              break;
          }
        },
        async observe() {
          return {
            stage: {
              Left: clients.Left.state.stage,
              Right: clients.Right.state.stage,
            },
            reserved: [...occupied].sort(),
          };
        },
        project: ({ stage, reserved }) => ({
          stage,
          reserved: [...(reserved as Client[])].sort(),
        }),
      },
    );
    if (fault === "none") {
      assert.equal(result.verdict, "matched");
      assert.deepEqual(result.recording.observations.at(-1), {
        stage: {
          Left: base === offered ? "Cancelled" : "Returned",
          Right: "Held",
        },
        reserved: ["Right"],
      });
    } else {
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, fault === "leaked" ? 2 : 4);
      assert.deepEqual(
        result.difference.observed,
        fault === "leaked"
          ? {
              stage: { Left: "Cancelled", Right: "Waiting" },
              reserved: ["Left"],
            }
          : {
              stage: { Left: "Held", Right: "Offered" },
              reserved: ["Left", "Right"],
            },
      );
    }
  }
});
