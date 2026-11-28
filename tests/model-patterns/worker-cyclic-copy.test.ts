import assert from "node:assert/strict";
import { once } from "node:events";
import test, { type TestContext } from "node:test";
import { Worker } from "node:worker_threads";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Value = "Original" | "Edited";
interface Link {
  value: Value;
  next: Link | null;
}
type Command = { kind: "send" | "editThroughCycle" };
type Fault = "none" | "cut-cycle" | "reuse-source";

const checker = "crates/idealis-check/target/debug/idealis-check";
const model = "examples/model-patterns/worker-transfer/cyclic-references.ideal";

function snapshot(root: Link) {
  const back = root.next?.next;
  return {
    root: root.value,
    neighbor: root.next?.value ?? null,
    back: back?.value ?? null,
    returnsToRoot: back === root,
  };
}

function openGraph(t: TestContext, fault: Fault) {
  const source: Link = { value: "Original", next: null };
  source.next = { value: "Original", next: source };
  let target: Link | undefined;
  let received: ReturnType<typeof snapshot> | null = null;
  // The reuse fault is a same-realm clone wrapper. A worker cannot hold this
  // ordinary source object directly across the transport.
  const worker =
    fault === "reuse-source"
      ? null
      : new Worker(
          `
    const { parentPort, workerData } = require("node:worker_threads");
    let root;
    parentPort.on("message", (command) => {
      if (command.kind === "send") {
        root = command.source;
        if (workerData === "cut-cycle") root.next.next = null;
      } else if (root.next?.next) {
        root.next.next.value = "Edited";
      }
      const back = root.next?.next;
      parentPort.postMessage({
        root: root.value,
        neighbor: root.next?.value ?? null,
        back: back?.value ?? null,
        returnsToRoot: back === root,
      });
    });
  `,
          { eval: true, workerData: fault },
        );
  if (worker) t.after(() => worker.terminate());
  return {
    async apply({ kind }: Command) {
      if (worker) {
        const response = once(worker, "message");
        worker.postMessage(kind === "send" ? { kind, source } : { kind });
        [received] = await response;
      } else if (kind === "send") {
        target = structuredClone(source);
        assert.ok(target.next);
        target.next.next = source;
        received = snapshot(target);
      } else {
        assert.ok(target?.next?.next);
        target.next.next.value = "Edited";
        received = snapshot(target);
      }
    },
    async observe() {
      return { sender: snapshot(source), received };
    },
  };
}

test(
  "a structured copy preserves a cycle inside the clone",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "MessageCopied",
    );
    assert.deepEqual(base.commands, [{ kind: "send" }]);
    const edit = { kind: "editThroughCycle" } as const;
    for (const fault of ["none", "cut-cycle", "reuse-source"] as const) {
      await t.test(fault, async (t) => {
        const graph = openGraph(t, fault);
        const result = await recordReplay(
          checker,
          { ...base, commands: [...base.commands, edit] },
          {
            ...graph,
            project({ target, edges, values }) {
              const next = edges as Record<string, string | null>;
              const bytes = values as Record<string, Value>;
              const view = (root: string) => {
                const neighbor = next[root];
                const back = neighbor == null ? null : next[neighbor];
                return {
                  root: bytes[root],
                  neighbor: neighbor == null ? null : bytes[neighbor],
                  back: back == null ? null : bytes[back],
                  returnsToRoot: back === root,
                };
              };
              return {
                sender: view("SourceA"),
                received: typeof target === "string" ? view(target) : null,
              };
            },
          },
        );
        if (fault === "none") assert.equal(result.verdict, "matched");
        else {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, 1);
          await graph.apply(edit);
        }
        const senderValue = fault === "reuse-source" ? "Edited" : "Original";
        assert.deepEqual(await graph.observe(), {
          sender: {
            root: senderValue,
            neighbor: "Original",
            back: senderValue,
            returnsToRoot: true,
          },
          received: {
            root: fault === "none" ? "Edited" : "Original",
            neighbor: "Original",
            back: fault === "cut-cycle" ? null : "Edited",
            returnsToRoot: fault === "none",
          },
        });
      });
    }
  },
);
