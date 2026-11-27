import assert from "node:assert/strict";
import test from "node:test";
import {
  recordReplay,
  transitionScenario,
  type ObservationAdapter,
} from "../../examples/model-patterns/replay.js";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/semaphore/cancel-reservation.model";

type Waiter = "First" | "Second";
type Client = "Owner" | Waiter;
type Command =
  | { readonly kind: "enqueue" | "cancel" | "deliver"; readonly client: Waiter }
  | { readonly kind: "release"; readonly client: Client }
  | { readonly kind: "reserve" };
type Fault = "leak" | "stale-grant";
type Result = "acquired" | "cancelled";

interface Wait {
  readonly client: Waiter;
  readonly settle: (result: Result) => void;
}

class Semaphore {
  private readonly queue: Wait[] = [];
  private readonly deliveries = new Map<Waiter, () => void>();
  private reserved: Wait | undefined;
  private held: Client | null = "Owner";

  constructor(private readonly fault?: Fault) {}

  acquire(client: Waiter, signal: AbortSignal): Promise<Result> {
    return new Promise((resolve) => {
      const wait: Wait = {
        client,
        settle(result) {
          signal.removeEventListener("abort", cancel);
          resolve(result);
        },
      };
      const cancel = () => {
        const at = this.queue.indexOf(wait);
        if (at >= 0) this.queue.splice(at, 1);
        if (this.reserved === wait && this.fault !== "leak") {
          this.reserved = undefined;
        }
        wait.settle("cancelled");
      };
      this.queue.push(wait);
      signal.addEventListener("abort", cancel, { once: true });
    });
  }

  reserve() {
    assert.equal(this.held, null);
    assert.equal(this.reserved, undefined);
    const next = this.queue.shift();
    assert.ok(next);
    this.reserved = next;
    this.deliveries.set(next.client, () => {
      if (this.fault !== "stale-grant" && this.reserved !== next) return;
      this.reserved = undefined;
      this.held = next.client;
      next.settle("acquired");
    });
  }

  deliver(client: Waiter) {
    const delivery = this.deliveries.get(client);
    assert.ok(delivery);
    this.deliveries.delete(client);
    queueMicrotask(delivery);
  }

  release(client: Client) {
    assert.equal(this.held, client);
    this.held = null;
  }

  snapshot() {
    return {
      waiting: { $sequence: this.queue.map(({ client }) => client) },
      reserved: this.reserved?.client ?? null,
      held: this.held,
      callbacks: [...this.deliveries.keys()].sort(),
    };
  }
}

function adapter(fault?: Fault) {
  const semaphore = new Semaphore(fault);
  const controllers = new Map<Waiter, AbortController>();
  const outcomes = new Map<Waiter, Result>();
  return {
    outcomes,
    async apply(command) {
      switch (command.kind) {
        case "enqueue": {
          const controller = new AbortController();
          controllers.set(command.client, controller);
          void semaphore
            .acquire(command.client, controller.signal)
            .then((result) => outcomes.set(command.client, result));
          break;
        }
        case "cancel": {
          const controller = controllers.get(command.client);
          assert.ok(controller);
          controller.abort();
          break;
        }
        case "reserve":
          semaphore.reserve();
          break;
        case "deliver":
          semaphore.deliver(command.client);
          break;
        case "release":
          semaphore.release(command.client);
          break;
      }
    },
    async observe() {
      return {
        ...semaphore.snapshot(),
        cancelled: [...outcomes]
          .filter(([, result]) => result === "cancelled")
          .map(([client]) => client)
          .sort(),
      };
    },
    project: (state) => ({
      ...state,
      callbacks: [...(state.callbacks as Waiter[])].sort(),
      cancelled: [...(state.cancelled as Waiter[])].sort(),
    }),
  } satisfies ObservationAdapter<Command> & { outcomes: Map<Waiter, Result> };
}

test("cancelling a reserved permit preserves the next waiter through a stale notification", async () => {
  const witness = await transitionScenario<Command>(
    checker,
    model,
    "deliver",
    "client in before(cancelled) and some before(reserved)",
  );
  assert.equal(witness.commands.length, 7);
  const stale = witness.commands.at(-1);
  assert.ok(stale?.kind === "deliver");
  const next = stale.client === "First" ? "Second" : "First";
  const scenario = {
    ...witness,
    commands: [...witness.commands, { kind: "deliver", client: next }] as const,
  };

  const healthy = adapter();
  const matched = await recordReplay(checker, scenario, healthy);
  assert.equal(matched.verdict, "matched");
  assert.equal(healthy.outcomes.get(stale.client), "cancelled");
  assert.equal(healthy.outcomes.get(next), "acquired");

  const leaked = await recordReplay(checker, scenario, adapter("leak"));
  assert.equal(leaked.verdict, "mismatch");
  assert.equal(leaked.difference.command?.kind, "cancel");
  assert.equal(
    (leaked.difference.expected as { reserved: unknown }).reserved,
    null,
  );
  assert.equal(
    (leaked.difference.observed as { reserved: unknown }).reserved,
    stale.client,
  );

  const overwritten = await recordReplay(
    checker,
    scenario,
    adapter("stale-grant"),
  );
  assert.equal(overwritten.verdict, "mismatch");
  assert.equal(overwritten.difference.step, 7);
  assert.deepEqual(overwritten.difference.command, stale);
  assert.equal(
    (overwritten.difference.expected as { reserved: unknown }).reserved,
    next,
  );
  assert.equal(
    (overwritten.difference.observed as { held: unknown }).held,
    stale.client,
  );
});
