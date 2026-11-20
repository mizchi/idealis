import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { test } from "node:test";
import { checkTrace, type Driver } from "../../packages/idealis/src/index.js";
import { choiceTrace } from "./choice-trace.js";

const dump = JSON.parse(
  await readFile("fixtures/connect/typed-choice.itf.json", "utf8"),
);
const trace = choiceTrace(dump);

// Project-owned adapter. The store uses its own keys and native bigint values.
function mapDriver(dropWrites: boolean): Driver<string, string> {
  const store = new Map<string, bigint>();
  const read = () => store.get("slot")!.toString();
  return {
    reset() {
      store.set("slot", 0n);
      return read();
    },
    step(decimal) {
      if (!dropWrites) store.set("slot", BigInt(decimal));
      return read();
    },
  };
}

// The process receives only operations. Its stdout contains fresh observations.
function processDriver(
  binary: string,
  dropWrites: boolean,
): Driver<string, string> {
  const child = spawn(binary, dropWrites ? ["drop-writes"] : [], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  const closed = once(child, "close");
  const lines = createInterface({ input: child.stdout })[
    Symbol.asyncIterator
  ]();
  async function send(operation: string) {
    child.stdin.write(operation + "\n");
    const line = await lines.next();
    assert.ok(!line.done, "store exited without an observation");
    return line.value;
  }
  return {
    reset: () => send("clear"),
    step: (decimal) => send("set " + decimal),
    async dispose() {
      child.stdin.end();
      const [code] = await closed;
      assert.equal(code, 0);
    },
  };
}

async function checkStore(
  open: (dropWrites: boolean) => Driver<string, string>,
) {
  const correct = await checkTrace(trace, open(false));
  assert.equal(correct.status, "matched");
  assert.equal(correct.checkedSteps, 6);
  const faulty = await checkTrace(trace, open(true));
  assert.equal(faulty.status, "mismatch");
  assert.equal(faulty.checkedSteps, 1); // The first Put was executed and failed.
  assert.deepEqual(faulty.expected, ["1"]);
  assert.equal(faulty.actual, "0");
}

test("a handwritten TS adapter satisfies storedSelection on the saved trace", async () => {
  await checkStore(mapDriver);
});

test("a Rust store satisfies the same property without generated bindings", async () => {
  const dir = await mkdtemp(join(tmpdir(), "idealis-plain-store-"));
  try {
    const binary = join(dir, "store");
    execFileSync("rustc", [
      "--edition=2024",
      "fixtures/connect/consumer/plain-store.rs",
      "-o",
      binary,
    ]);
    await checkStore((dropWrites) => processDriver(binary, dropWrites));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("empty traces, unknown operations and malformed model values fail conversion", () => {
  assert.throws(() => choiceTrace({ ...dump, states: [] }), /empty/);
  for (const command of [{ tag: "Delete" }, { tag: "Put", value: {} }]) {
    assert.throws(() =>
      choiceTrace({
        ...dump,
        states: [dump.states[0], { ...dump.states[1], command }],
      }),
    );
  }
  assert.throws(() =>
    choiceTrace({
      ...dump,
      states: [{ ...dump.states[0], observation: { value: 0 } }],
    }),
  );
});

test("a convertible oracle that violates storedSelection is rejected", () => {
  const state = {
    ...dump.states[1],
    observation: { value: { "#bigint": "0" } },
  };
  assert.throws(
    () => choiceTrace({ ...dump, states: [dump.states[0], state] }),
    /storedSelection/,
  );
  assert.throws(
    () =>
      choiceTrace({
        ...dump,
        states: [
          { ...dump.states[0], observation: { value: { "#bigint": "1" } } },
        ],
      }),
    /storedSelection/,
  );
});
