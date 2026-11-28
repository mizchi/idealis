import assert from "node:assert/strict";
import {
  TransformStream,
  type TransformStreamDefaultController,
} from "node:stream/web";
import test from "node:test";
import { replay, type Scenario } from "../../examples/model-patterns/replay.js";

const chunks = ["First", "Second"] as const;
type Chunk = (typeof chunks)[number];
type Command =
  | { readonly kind: "write" | "read"; readonly chunk: Chunk }
  | { readonly kind: "close" | "fail" | "readTerminal" };

const drain: Command[] = [
  { kind: "write", chunk: "First" },
  { kind: "write", chunk: "Second" },
  { kind: "close" },
  { kind: "read", chunk: "First" },
  { kind: "read", chunk: "Second" },
  { kind: "readTerminal" },
];
const failAfterRead: Command[] = [
  { kind: "write", chunk: "First" },
  { kind: "read", chunk: "First" },
  { kind: "write", chunk: "Second" },
  { kind: "close" },
  { kind: "fail" },
  { kind: "readTerminal" },
];
const failWithTwoWaiting: Command[] = [
  { kind: "write", chunk: "First" },
  { kind: "write", chunk: "Second" },
  { kind: "close" },
  { kind: "fail" },
  { kind: "readTerminal" },
];

test(
  "a TransformStream drains on close and reports an error to both sides",
  { timeout: 30_000 },
  async () => {
    for (const [fault, commands] of [
      ["none", drain],
      ["none", failAfterRead],
      ["none", failWithTwoWaiting],
      ["drop-on-close", drain],
      ["fail-as-end", failAfterRead],
    ] as const) {
      let controller!: TransformStreamDefaultController<Chunk>;
      let closing = false;
      const stream = new TransformStream<Chunk, Chunk>(
        {
          start(opened) {
            controller = opened;
          },
          transform(chunk, output) {
            if (fault === "drop-on-close" && closing) return;
            output.enqueue(chunk);
          },
        },
        { highWaterMark: 2 },
        { highWaterMark: 0 },
      );
      const writer = stream.writable.getWriter();
      const reader = stream.readable.getReader();
      void reader.closed.catch(() => {});
      let writerPhase: "Closed" | "Failed" | null = null;
      const writerFinished = writer.closed.then(
        () => {
          writerPhase = "Closed";
        },
        () => {
          writerPhase = "Failed";
        },
      );
      const outcome = new Map<Chunk, "Waiting" | "Delivered" | "WriteError">();
      const writes = new Map<Chunk, Promise<void>>();
      const received: Chunk[] = [];
      let terminal: "End" | "ReadError" | null = null;
      const scenario: Scenario<Command> = {
        model: "examples/model-patterns/stream-terminal/stream.ideal",
        config: {},
        initial: {
          phase: "Open",
          pending: { $sequence: [] },
          received: { $sequence: [] },
          outcome: { First: null, Second: null },
          terminal: null,
        },
        commands,
      };
      const run = replay(
        "crates/idealis-check/target/debug/idealis-check",
        scenario,
        {
          async apply(command) {
            switch (command.kind) {
              case "write": {
                const { chunk } = command;
                outcome.set(chunk, "Waiting");
                const wrote = writer.write(chunk).then(
                  () => {
                    outcome.set(chunk, "Delivered");
                  },
                  () => {
                    outcome.set(chunk, "WriteError");
                  },
                );
                writes.set(chunk, wrote);
                break;
              }
              case "close":
                closing = true;
                void writer.close().catch(() => {});
                if (![...outcome.values()].includes("Waiting"))
                  await writerFinished;
                break;
              case "fail":
                if (fault === "fail-as-end") controller.terminate();
                else controller.error(new Error("transform failed"));
                await Promise.all([writerFinished, ...writes.values()]);
                break;
              case "read": {
                const read = await reader.read();
                if (read.done) {
                  terminal = "End";
                  await Promise.all([writerFinished, ...writes.values()]);
                } else {
                  received.push(read.value);
                  await writes.get(read.value);
                  if (closing && ![...outcome.values()].includes("Waiting"))
                    await writerFinished;
                }
                break;
              }
              case "readTerminal":
                try {
                  const read = await reader.read();
                  assert.equal(read.done, true);
                  terminal = "End";
                } catch (error) {
                  if (error instanceof assert.AssertionError) throw error;
                  terminal = "ReadError";
                }
                break;
            }
          },
          async observe() {
            return {
              phase: writerPhase ?? (closing ? "Closing" : "Open"),
              received: { $sequence: [...received] },
              outcome: Object.fromEntries(
                chunks.map((chunk) => [chunk, outcome.get(chunk) ?? null]),
              ),
              terminal,
            };
          },
          project(state) {
            return {
              phase: state.phase,
              received: state.received,
              outcome: state.outcome,
              terminal: state.terminal,
            };
          },
        },
      );
      try {
        if (fault === "drop-on-close") {
          await assert.rejects(
            run,
            /"step":4.*"observed":\{"phase":"Closed","received":\{"\$sequence":\[\]/,
          );
        } else if (fault === "fail-as-end") {
          await assert.rejects(run, /"step":6.*"observed":.*"terminal":"End"/);
        } else {
          await run;
        }
      } finally {
        if (writerPhase === null) controller.error(new Error("test finished"));
        await writerFinished;
      }
    }
  },
);
