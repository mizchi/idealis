import assert from "node:assert/strict";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Notice = "First" | "Second";
type Registration = "Old" | "New";
type Delivery = { notice: Notice; registration: Registration };
type Command =
  | { kind: "subscribe"; registration: Registration }
  | { kind: "unsubscribe" }
  | { kind: "announce"; notice: Notice }
  | ({ kind: "dispatch" } & Delivery);

const rcheck = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/queued.model";
const dispatch = (notice: Notice, registration: Registration): Command => ({
  kind: "dispatch",
  notice,
  registration,
});

function rows(deliveries: readonly Delivery[]) {
  const values: Record<Notice, Registration[]> = { First: [], Second: [] };
  for (const { notice, registration } of deliveries) {
    values[notice].push(registration);
  }
  return values;
}

test("each queued notification retains the registration it captured", async () => {
  const shared = await reachableScenario<Command>(
    rcheck,
    model,
    "BothQueuedForOld",
  );
  const mixed = await reachableScenario<Command>(
    rcheck,
    model,
    "QueuedAcrossGenerations",
  );
  const replaced = await reachableScenario<Command>(
    rcheck,
    model,
    "BothQueuedAcrossReregistration",
  );
  for (const [base, suffix, fault, expected] of [
    [
      shared,
      [dispatch("First", "Old"), dispatch("Second", "Old")],
      "none",
      [
        { notice: "First", registration: "Old" },
        { notice: "Second", registration: "Old" },
      ],
    ],
    [
      mixed,
      [dispatch("First", "Old"), dispatch("Second", "New")],
      "none",
      [{ notice: "Second", registration: "New" }],
    ],
    [
      replaced,
      [dispatch("First", "Old"), dispatch("Second", "Old")],
      "none",
      [],
    ],
    [replaced, [dispatch("First", "Old")], "current-registration", []],
    [shared, [dispatch("First", "Old")], "collapse-queue", []],
  ] as const) {
    const bus = new EventTarget();
    let active: {
      registration: Registration;
      controller: AbortController;
    } | null = null;
    const pending = new Map<Notice, Delivery>();
    const delivered: Delivery[] = [];
    const dropped: Delivery[] = [];
    const result = await recordReplay<Command>(
      rcheck,
      {
        ...base,
        commands: [...base.commands, ...suffix],
      },
      {
        async apply(command) {
          switch (command.kind) {
            case "subscribe": {
              assert.equal(active, null);
              const { registration } = command;
              const controller = new AbortController();
              active = { registration, controller };
              bus.addEventListener(
                registration,
                (event) => {
                  const notice = (event as CustomEvent<Notice>).detail;
                  delivered.push({ notice, registration });
                },
                { signal: controller.signal },
              );
              break;
            }
            case "unsubscribe":
              assert.ok(active);
              active.controller.abort();
              active = null;
              break;
            case "announce":
              if (active) {
                pending.set(command.notice, {
                  notice: command.notice,
                  registration: active.registration,
                });
              }
              break;
            case "dispatch": {
              const delivery = pending.get(command.notice);
              assert.ok(delivery);
              assert.equal(command.registration, delivery.registration);
              pending.delete(command.notice);
              if (fault === "collapse-queue") {
                for (const [notice, work] of pending) {
                  if (work.registration === delivery.registration)
                    pending.delete(notice);
                }
              }
              const recipient =
                fault === "current-registration"
                  ? active?.registration
                  : delivery.registration;
              const count = delivered.length;
              if (recipient)
                bus.dispatchEvent(
                  new CustomEvent(recipient, { detail: delivery.notice }),
                );
              if (count === delivered.length) dropped.push(delivery);
              break;
            }
          }
        },
        async observe() {
          return {
            pending: rows([...pending.values()]),
            delivered: rows(delivered),
            dropped: rows(dropped),
          };
        },
        project: ({ pending, delivered, dropped }) => ({
          pending,
          delivered,
          dropped,
        }),
      },
    );
    if (fault === "none") {
      assert.equal(result.verdict, "matched");
      assert.deepEqual(delivered, expected);
    } else {
      assert.equal(result.verdict, "mismatch");
      assert.equal(result.difference.step, base.commands.length + 1);
      assert.deepEqual(
        result.difference.observed,
        fault === "current-registration"
          ? {
              pending: { First: [], Second: ["Old"] },
              delivered: { First: ["New"], Second: [] },
              dropped: { First: [], Second: [] },
            }
          : {
              pending: { First: [], Second: [] },
              delivered: { First: ["Old"], Second: [] },
              dropped: { First: [], Second: [] },
            },
      );
    }
  }
});
