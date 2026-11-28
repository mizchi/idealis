import assert from "node:assert/strict";
import test from "node:test";
import {
  reachableScenario,
  replay,
} from "../../examples/model-patterns/replay.js";

type Subscriber = "Left" | "Right";
type Ticket = "L0" | "L1" | "R0";
type Command =
  | {
      readonly kind: "subscribe";
      readonly subscriber: Subscriber;
      readonly ticket: Ticket;
    }
  | { readonly kind: "unsubscribe"; readonly subscriber: Subscriber }
  | { readonly kind: "announce" }
  | { readonly kind: "dispatch"; readonly ticket: Ticket };

test("a deferred notification follows the registration captured at announcement", async () => {
  const checkerPath = "crates/idealis-check/target/debug/idealis-check";
  const model = "examples/model-patterns/subscriptions/notification.ideal";
  const replaced = await reachableScenario<Command>(
    checkerPath,
    model,
    "ReplacedWhileQueued",
  );
  const late = await reachableScenario<Command>(
    checkerPath,
    model,
    "RightJoinsAfterAnnouncement",
  );
  const afterDelivery = await reachableScenario<Command>(
    checkerPath,
    model,
    "LeftDeliveredWithRightPending",
  );
  const dispatch = (ticket: Ticket): Command => ({ kind: "dispatch", ticket });
  for (const [trace, suffix, expected, fault] of [
    [replaced, [dispatch("L0"), dispatch("R0")], ["R0"], "none"],
    [replaced, [dispatch("L0"), dispatch("R0")], [], "current-registration"],
    [late, [dispatch("L0")], ["L0"], "none"],
    [late, [dispatch("L0")], [], "add-late-registration"],
    [
      afterDelivery,
      [{ kind: "unsubscribe", subscriber: "Left" }, dispatch("R0")],
      ["L0", "R0"],
      "none",
    ],
  ] as const) {
    const target = new EventTarget();
    const active = new Map<
      Subscriber,
      { ticket: Ticket; controller: AbortController }
    >();
    const pending = new Map<Ticket, Subscriber>();
    const used = new Set<Ticket>();
    const delivered: Ticket[] = [];
    let started = false;
    const run = replay<Command>(
      checkerPath,
      {
        ...trace,
        commands: [...trace.commands, ...suffix],
      },
      {
        async apply(command) {
          switch (command.kind) {
            case "subscribe": {
              const { subscriber, ticket } = command;
              const controller = new AbortController();
              active.set(subscriber, { ticket, controller });
              used.add(ticket);
              target.addEventListener(
                ticket,
                () => {
                  delivered.push(ticket);
                },
                { signal: controller.signal },
              );
              if (started && fault === "add-late-registration")
                pending.set(ticket, subscriber);
              break;
            }
            case "unsubscribe":
              active.get(command.subscriber)?.controller.abort();
              active.delete(command.subscriber);
              break;
            case "announce":
              started = true;
              for (const [subscriber, { ticket }] of active)
                pending.set(ticket, subscriber);
              break;
            case "dispatch": {
              const subscriber = pending.get(command.ticket);
              assert.ok(subscriber);
              pending.delete(command.ticket);
              const ticket =
                fault === "current-registration"
                  ? active.get(subscriber)?.ticket
                  : command.ticket;
              if (ticket) target.dispatchEvent(new Event(ticket));
              break;
            }
          }
        },
        async observe() {
          return {
            active: {
              Left: active.get("Left")?.ticket ?? null,
              Right: active.get("Right")?.ticket ?? null,
            },
            used: [...used].sort(),
            pending: [...pending.keys()].sort(),
            delivered: [...delivered].sort(),
            started,
          };
        },
        project(state) {
          const sorted = (value: unknown) => [...(value as Ticket[])].sort();
          return {
            active: state.active,
            used: sorted(state.used),
            pending: sorted(state.pending),
            delivered: sorted(state.delivered),
            started: state.started,
          };
        },
      },
    );
    if (fault === "current-registration") {
      await assert.rejects(
        run,
        new RegExp(
          `"step":${trace.commands.length + 1}.*"observed":.*"delivered":\\["L1"\\]`,
        ),
      );
    } else if (fault === "add-late-registration") {
      await assert.rejects(
        run,
        new RegExp(
          `"step":${trace.commands.length}.*"observed":.*"pending":\\["L0","R0"\\]`,
        ),
      );
    } else {
      await run;
      assert.deepEqual(delivered, expected);
    }
  }
});

test("an unreachable state cannot become a replay scenario", async () => {
  await assert.rejects(
    reachableScenario(
      "crates/idealis-check/target/debug/idealis-check",
      "spec/models/rmodel-behavior.model",
      "BothOn",
    ),
    /no complete reachability witness for BothOn: unreachable/,
  );
});
