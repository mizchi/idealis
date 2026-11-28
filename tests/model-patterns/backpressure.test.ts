import assert from "node:assert/strict";
import test from "node:test";
import { replay, type Scenario } from "../../examples/model-patterns/replay.js";

const requests = ["Second", "Third", "Fourth"] as const;
type Item = "First" | (typeof requests)[number];
type Result = "Accepted" | "Cancelled";
type Command =
  | { readonly kind: "write" | "cancel"; readonly item: Item }
  | { readonly kind: "take" };

class AsyncSlot {
  next: Item | null = null;
  private readonly waiting: Array<{
    item: Item;
    accept: () => void;
  }> = [];

  constructor(private readonly keepCancelled: boolean) {}

  write(item: Item, signal: AbortSignal): Promise<Result> {
    if (signal.aborted) return Promise.resolve("Cancelled");
    if (this.next === null) {
      this.next = item;
      return Promise.resolve("Accepted");
    }
    return new Promise((resolve) => {
      const waiter = {
        item,
        accept: () => {
          signal.removeEventListener("abort", cancel);
          resolve("Accepted");
        },
      };
      const cancel = () => {
        if (!this.keepCancelled) {
          const index = this.waiting.indexOf(waiter);
          assert.ok(index >= 0);
          this.waiting.splice(index, 1);
        }
        resolve("Cancelled");
      };
      this.waiting.push(waiter);
      signal.addEventListener("abort", cancel, { once: true });
    });
  }

  take(): Item {
    const first = this.next;
    assert.ok(first);
    const queued = this.waiting.shift();
    this.next = queued?.item ?? null;
    queued?.accept();
    return first;
  }
}

test("backpressure keeps order when a blocked write is cancelled", async () => {
  for (const [position, cancelled] of requests.entries()) {
    const survivors = requests.filter((item) => item !== cancelled);
    const first = survivors[0];
    assert.ok(first);
    const scenario: Scenario<Command> = {
      model: "examples/model-patterns/backpressure/channel.ideal",
      config: {},
      initial: {
        buffer: "First",
        waiting: { $sequence: [] },
        outcome: { Second: null, Third: null, Fourth: null },
        delivered: [],
      },
      commands: [
        ...requests.map((item) => ({ kind: "write" as const, item })),
        { kind: "cancel", item: cancelled },
        { kind: "take" },
        { kind: "cancel", item: first },
        { kind: "take" },
        { kind: "take" },
      ],
    };
    for (const keepCancelled of [false, true]) {
      const slot = new AsyncSlot(keepCancelled);
      assert.equal(
        await slot.write("First", new AbortController().signal),
        "Accepted",
      );
      const controllers = new Map<Item, AbortController>();
      const results = new Map<Item, Result | "Waiting">();
      const delivered: Item[] = [];
      const run = replay(
        "crates/idealis-check/target/debug/idealis-check",
        scenario,
        {
          async apply(command) {
            if (command.kind === "write") {
              const controller = new AbortController();
              controllers.set(command.item, controller);
              results.set(command.item, "Waiting");
              void slot
                .write(command.item, controller.signal)
                .then((result) => {
                  results.set(command.item, result);
                });
            } else if (command.kind === "cancel") {
              controllers.get(command.item)?.abort();
            } else {
              delivered.push(slot.take());
            }
          },
          async observe() {
            return {
              buffer: slot.next,
              outcome: Object.fromEntries(
                requests.map((item) => [item, results.get(item) ?? null]),
              ),
              delivered: [...delivered].sort(),
            };
          },
          project(state) {
            return {
              buffer: state.buffer,
              outcome: state.outcome,
              delivered: [...(state.delivered as Item[])].sort(),
            };
          },
        },
      );
      if (keepCancelled) {
        const step = position === 0 ? 5 : 6 + position;
        await assert.rejects(
          run,
          new RegExp(`"step":${step}.*"observed":\\{"buffer":"${cancelled}"`),
        );
        assert.equal(slot.take(), cancelled);
      } else {
        await run;
        assert.deepEqual(delivered, ["First", ...survivors]);
        assert.equal(results.get(cancelled), "Cancelled");
        for (const item of survivors)
          assert.equal(results.get(item), "Accepted");
      }
    }
  }
});
