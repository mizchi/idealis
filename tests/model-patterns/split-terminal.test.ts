import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { MessageChannel } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

const chunks = ["First", "Second"] as const;
const boundaries = ["Empty", "One", "Two"] as const;
type Chunk = (typeof chunks)[number];
type Boundary = (typeof boundaries)[number];
type Command =
  | { kind: "send" | "receiveData" | "applyChunk"; chunk: Chunk }
  | { kind: "close" | "receiveClose" | "finish" };
type Fault = "end-on-tail" | "skip-prefix" | null;

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/stream-terminal/split-terminal.ideal";

function openReceiver(t: TestContext, fault: Fault) {
  const channel = () => {
    const ports = new MessageChannel();
    t.after(() => {
      ports.port1.close();
      ports.port2.close();
    });
    return ports;
  };
  const control = channel();
  const data = { First: channel(), Second: channel() };
  const sent: Chunk[] = [];
  const arrived = new Set<Chunk>();
  const applied: Chunk[] = [];
  let sealed: Boundary | null = null;
  let announced: Boundary | null = null;
  let ended = false;
  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "send":
          data[command.chunk].port1.postMessage(command.chunk);
          sent.push(command.chunk);
          break;
        case "close":
          sealed = boundaries[sent.length]!;
          control.port1.postMessage(sealed);
          break;
        case "receiveData": {
          const [chunk] = await once(data[command.chunk].port2, "message");
          assert.equal(chunk, command.chunk);
          arrived.add(chunk);
          break;
        }
        case "receiveClose": {
          const [boundary] = await once(control.port2, "message");
          assert.ok(boundaries.includes(boundary));
          announced = boundary;
          break;
        }
        case "applyChunk":
          if (
            fault === "skip-prefix" ||
            chunks[applied.length] === command.chunk
          )
            applied.push(command.chunk);
          break;
        case "finish": {
          assert.ok(announced);
          const count = boundaries.indexOf(announced);
          ended =
            fault === "end-on-tail"
              ? count === 0 || arrived.has(chunks[count - 1]!)
              : applied.length === count;
          break;
        }
      }
    },
    async observe() {
      return {
        sent: [...sent],
        arrived: chunks.filter((chunk) => arrived.has(chunk)),
        applied: [...applied],
        sealed,
        announced,
        ended,
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

test(
  "a terminal message can arrive before the first chunk on a separate port",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "ControlAndSecondOvertakeFirst",
    );
    assert.equal(base.commands.length, 5);
    await t.test("an empty stream can end without data", async (t) => {
      const empty = await reachableScenario<Command>(
        checker,
        model,
        "EmptyEnds",
      );
      assert.deepEqual(
        empty.commands.map(({ kind }) => kind),
        ["close", "receiveClose", "finish"],
      );
      const result = await recordReplay(checker, empty, openReceiver(t, null));
      assert.equal(result.verdict, "matched");
      assert.deepEqual(result.recording.observations.at(-1), {
        sent: [],
        arrived: [],
        applied: [],
        sealed: "Empty",
        announced: "Empty",
        ended: true,
      });
    });
    const commands: Command[] = [
      ...base.commands,
      { kind: "applyChunk", chunk: "Second" },
      { kind: "finish" },
      { kind: "receiveData", chunk: "First" },
      { kind: "applyChunk", chunk: "First" },
      { kind: "applyChunk", chunk: "Second" },
      { kind: "finish" },
    ];
    for (const fault of [null, "end-on-tail", "skip-prefix"] as const) {
      await t.test(fault ?? "drain all chunks in order", async (t) => {
        const result = await recordReplay(
          checker,
          { ...base, commands },
          openReceiver(t, fault),
        );
        const waiting = {
          sent: ["First", "Second"],
          arrived: ["Second"],
          applied: [],
          sealed: "Two",
          announced: "Two",
          ended: false,
        };
        if (fault === "end-on-tail") {
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(result.difference, {
            step: 7,
            command: { kind: "finish" },
            expected: waiting,
            observed: { ...waiting, ended: true },
          });
        } else if (fault === "skip-prefix") {
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(result.difference, {
            step: 6,
            command: { kind: "applyChunk", chunk: "Second" },
            expected: waiting,
            observed: { ...waiting, applied: ["Second"] },
          });
        } else {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[7], waiting);
          assert.deepEqual(result.recording.observations.at(-1), {
            ...waiting,
            arrived: ["First", "Second"],
            applied: ["First", "Second"],
            ended: true,
          });
        }
      });
    }
  },
);
